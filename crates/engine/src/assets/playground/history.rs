use super::settings::Settings;

const MAX_STEPS: usize = 100;

/// The playground's own undo stack, apart from the document's: at most 100 steps.
#[derive(Clone, Debug, Default)]
pub struct History {
    past: Vec<Settings>,
    present: Settings,
    future: Vec<Settings>,
    preview: Option<Settings>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> &Settings {
        self.preview.as_ref().unwrap_or(&self.present)
    }

    /// Shows `settings` without making a step: a slider still being dragged.
    pub fn preview(&mut self, settings: Settings) {
        self.preview = Some(settings);
    }

    pub fn commit(&mut self, settings: Settings) {
        self.preview = None;
        self.past.push(std::mem::replace(&mut self.present, settings));
        self.future.clear();
        if self.past.len() > MAX_STEPS {
            self.past.remove(0);
        }
    }

    /// Back to a step; a preview in progress is dropped first and counts as that move.
    pub fn undo(&mut self) -> bool {
        if self.preview.take().is_some() {
            return true;
        }
        match self.past.pop() {
            Some(before) => {
                self.future.push(std::mem::replace(&mut self.present, before));
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        self.preview = None;
        match self.future.pop() {
            Some(after) => {
                self.past.push(std::mem::replace(&mut self.present, after));
                true
            }
            None => false,
        }
    }

    /// Empty settings, as one step that undo walks back.
    pub fn revert(&mut self) {
        self.commit(Settings::default());
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}
