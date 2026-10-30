//! The Playground's "Show code" window: the style sheet pushed to the page and the HTML of the
//! model, read only and copyable. It shows what `playground.code` answered when it opened.

use egui::{Context, Id, RichText};
use serde_json::json;

use super::window;
use crate::FigApp;
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::widgets;

const CODE: &str = "playground.code";
const WIDTH: f32 = 640.0;
const OVERRIDES_ROWS: usize = 8;
const TEMPLATE_ROWS: usize = 14;
const MAX_SHOWN_CHARS: usize = 50_000;

#[derive(Clone, Default)]
struct Code {
    overrides: String,
    template: String,
    problem: Option<String>,
}

impl Code {
    fn read(app: &mut FigApp) -> Self {
        match app.session.execute(CODE, &json!({})) {
            Ok(answer) => {
                let text = |key: &str| answer[key].as_str().unwrap_or_default().to_string();
                Self { overrides: text("overrides"), template: text("template"), problem: None }
            }
            Err(error) => Self { problem: Some(error.to_string()), ..Self::default() },
        }
    }

    fn truncated(&self) -> bool {
        [&self.overrides, &self.template].iter().any(|text| text.chars().nth(MAX_SHOWN_CHARS).is_some())
    }
}

fn data_id() -> Id {
    Id::unique("playground_code_text")
}

fn section(ui: &mut egui::Ui, t: &Tokens, title: &str, text: &str, rows: usize, copy_label: &str) {
    widgets::label(ui, title, t.text_secondary);
    let shown: String = text.chars().take(MAX_SHOWN_CHARS).collect();
    let mut view = shown.as_str();
    egui::ScrollArea::vertical().id_salt(title).max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
        ui.add(egui::TextEdit::multiline(&mut view).code_editor().desired_width(f32::INFINITY).desired_rows(rows));
    });
    if widgets::button(ui, copy_label, false, true).clicked() {
        crate::egui_copy_text(ui.ctx(), text);
    }
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    if !app.ui.playground.code_open {
        ctx.data_mut(|data| data.remove::<Code>(data_id()));
        return;
    }
    let stored = ctx.data_mut(|data| data.get_temp::<Code>(data_id()));
    let code = stored.unwrap_or_else(|| Code::read(app));
    let t = Tokens::get(ctx);
    let mut open = true;
    window(tr("Show code"), ctx).id(Id::unique("playground_code_window")).open(&mut open).show(ctx, |ui| {
        ui.set_width(WIDTH);
        if let Some(problem) = &code.problem {
            ui.label(RichText::new(problem).color(t.danger));
            return;
        }
        section(ui, &t, tr("Overrides"), &code.overrides, OVERRIDES_ROWS, tr("Copy overrides"));
        ui.add_space(8.0);
        section(ui, &t, tr("Template"), &code.template, TEMPLATE_ROWS, tr("Copy template"));
        if code.truncated() {
            ui.add_space(4.0);
            widgets::label(ui, tr("Only the beginning is shown here: the copy buttons take everything."), t.text_secondary);
        }
    });
    if open {
        ctx.data_mut(|data| data.insert_temp(data_id(), code));
    } else {
        app.ui.playground.code_open = false;
        ctx.data_mut(|data| data.remove::<Code>(data_id()));
    }
}
