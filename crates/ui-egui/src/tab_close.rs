use egui::Context;
use serde_json::json;

use crate::FigApp;
use crate::i18n::{tr, trf};
use crate::windows::CloseAnswer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Asking,
    Saving,
    SavingAs,
}

#[derive(Clone, Debug)]
pub struct TabClose {
    pub(crate) uid: u64,
    pub(crate) stage: Stage,
}

pub(crate) fn ask(app: &mut FigApp, uid: u64) {
    app.ui.tab_close = Some(TabClose { uid, stage: Stage::Asking });
}

pub(crate) fn answer(app: &mut FigApp, ctx: &Context, answer: CloseAnswer) {
    let Some(question) = app.ui.tab_close.as_mut() else { return };
    match answer {
        CloseAnswer::Cancel => app.ui.tab_close = None,
        CloseAnswer::DontSave => {
            let uid = question.uid;
            app.ui.tab_close = None;
            close(app, uid, true);
        }
        CloseAnswer::Save => {
            question.stage = Stage::Saving;
            ctx.request_repaint();
        }
    }
}

pub(crate) fn show(app: &mut FigApp, ctx: &Context) {
    let Some(TabClose { uid, stage }) = app.ui.tab_close.clone() else { return };
    match stage {
        Stage::Asking => ask_window(app, ctx, uid),
        Stage::Saving => save(app, ctx, uid),
        Stage::SavingAs => finish_save_as(app, uid),
    }
}

fn index_of(app: &FigApp, uid: u64) -> Option<usize> {
    app.session.docs.iter().position(|d| d.uid == uid)
}

fn close(app: &mut FigApp, uid: u64, force: bool) {
    if let Some(i) = index_of(app, uid) {
        app.run("file.close", json!({"index": i, "force": force}));
    }
}

fn ask_window(app: &mut FigApp, ctx: &Context, uid: u64) {
    let Some(name) = app.session.docs.iter().find(|d| d.uid == uid && d.is_dirty()).map(|d| d.doc.name.clone()) else {
        app.ui.tab_close = None;
        return;
    };
    let t = crate::theme::Tokens::get(ctx);
    let mut open = true;
    let mut answered = None;
    crate::dialogs::window(tr("Close tab"), ctx).order(egui::Order::Foreground).open(&mut open).show(ctx, |ui| {
        ui.set_width(340.0);
        crate::widgets::label(ui, &trf("{name} has unsaved changes. Save it before closing the tab?", &[("name", &name)]), t.text);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if crate::widgets::button(ui, tr("Don't Save"), false, true).clicked() {
                answered = Some(CloseAnswer::DontSave);
            }
            if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
                answered = Some(CloseAnswer::Cancel);
            }
            if crate::widgets::button(ui, tr("Save"), true, true).clicked() {
                answered = Some(CloseAnswer::Save);
            }
        });
        if answered.is_none() && ui.memory(|m| m.focused().is_none()) && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            answered = Some(CloseAnswer::Save);
        }
    });
    if !open {
        answered = Some(CloseAnswer::Cancel);
    }
    if let Some(a) = answered {
        answer(app, ctx, a);
    }
}

fn save(app: &mut FigApp, ctx: &Context, uid: u64) {
    let Some(i) = index_of(app, uid) else {
        app.ui.tab_close = None;
        return;
    };
    app.session.active = i;
    if crate::windows::saves_in_place(app.session.docs.get(i).and_then(|d| d.path.as_deref())) {
        app.ui.tab_close = None;
        if app.run("file.save", json!({})).is_some() {
            close(app, uid, false);
        }
        return;
    }
    app.ui.tab_close = Some(TabClose { uid, stage: Stage::SavingAs });
    app.save_as_dialog();
    finish_save_as(app, uid);
    ctx.request_repaint();
}

fn finish_save_as(app: &mut FigApp, uid: u64) {
    if app.pending_dialog.is_some() {
        return;
    }
    app.ui.tab_close = None;
    if index_of(app, uid).and_then(|i| app.session.docs.get(i)).is_some_and(|d| !d.is_dirty()) {
        close(app, uid, false);
    }
}
