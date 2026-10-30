//! The Playground's "Save theme" window: names the theme the settings are written into (a new
//! collection, or the chosen theme rewritten), and calls `playground.saveTheme` only on "Save".

use egui::{Context, Id, RichText};
use figcraft_engine::EngineError;
use serde_json::json;

use super::window;
use crate::FigApp;
use crate::a11y;
use crate::i18n::{tr, trf};
use crate::theme::Tokens;
use crate::widgets;

const SAVE: &str = "playground.saveTheme";
const WIDTH: f32 = 320.0;

#[derive(Clone, Default)]
struct Draft {
    name: String,
    overwrite: bool,
    apply: bool,
    problem: Option<String>,
    themes: Vec<String>,
    chosen: String,
    focused: bool,
}

impl Draft {
    fn open(app: &mut FigApp) -> Self {
        let (themes, chosen) = crate::playground_bars::theme_choices(app);
        Self { themes, chosen, ..Self::default() }
    }
}

fn data_id() -> Id {
    Id::unique("playground_save_draft")
}

fn reason(error: EngineError) -> String {
    match error {
        EngineError::BadParams { msg, .. } => msg,
        other => other.to_string(),
    }
}

fn commit(app: &mut FigApp, draft: &mut Draft) -> bool {
    let name = draft.name.trim().to_string();
    let mut params = if draft.overwrite {
        json!({"name": draft.chosen, "update": true})
    } else if name.is_empty() {
        draft.problem = Some(tr("Give the theme a name").to_string());
        return false;
    } else if draft.themes.contains(&name) {
        draft.problem = Some(tr("A theme with this name already exists").to_string());
        return false;
    } else {
        json!({"name": name})
    };
    if draft.apply {
        params["apply"] = json!(true);
    }
    match app.session.execute(SAVE, &params) {
        Ok(_) => {
            let saved = params["name"].as_str().unwrap_or_default();
            app.toast(trf("Theme {name} saved", &[("name", &saved)]));
            true
        }
        Err(error) => {
            draft.problem = Some(reason(error));
            false
        }
    }
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    if !app.ui.playground.save_open {
        ctx.data_mut(|data| data.remove::<Draft>(data_id()));
        return;
    }
    let stored = ctx.data_mut(|data| data.get_temp::<Draft>(data_id()));
    let mut draft = stored.unwrap_or_else(|| Draft::open(app));
    let t = Tokens::get(ctx);
    let stale = app.session.playground_stale();
    let (mut open, mut save, mut cancel) = (true, false, false);
    window(tr("Save theme"), ctx).id(Id::unique("playground_save_window")).open(&mut open).show(ctx, |ui| {
        ui.set_width(WIDTH);
        let field =
            ui.add_enabled(!draft.overwrite, egui::TextEdit::singleline(&mut draft.name).hint_text(tr("Theme name")).desired_width(f32::INFINITY));
        a11y::text(&field, tr("Theme name"), &draft.name);
        if !draft.focused {
            field.request_focus();
            draft.focused = true;
        }
        if field.changed() {
            draft.problem = None;
        }
        save = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if !draft.chosen.is_empty() {
            ui.horizontal(|ui| {
                if widgets::checkbox(ui, draft.overwrite, tr("Overwrite the chosen theme")) {
                    draft.overwrite = !draft.overwrite;
                    draft.problem = None;
                }
                widgets::label(ui, &draft.chosen, t.text_secondary);
            });
        }
        if widgets::checkbox(ui, draft.apply, tr("Set as the applied theme")) {
            draft.apply = !draft.apply;
        }
        if stale {
            ui.add(
                egui::Label::new(
                    RichText::new(tr("The file changed since the page was exported: the saved theme may differ from the page.")).color(t.warning),
                )
                .wrap(),
            );
        }
        if let Some(problem) = &draft.problem {
            ui.label(RichText::new(problem).color(t.danger));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            cancel = widgets::button(ui, tr("Cancel"), false, true).clicked();
            save |= widgets::button(ui, tr("Save"), true, true).clicked();
        });
    });
    if (save && commit(app, &mut draft)) || cancel || !open {
        app.ui.playground.save_open = false;
        ctx.data_mut(|data| data.remove::<Draft>(data_id()));
    } else {
        ctx.data_mut(|data| data.insert_temp(data_id(), draft));
    }
}
