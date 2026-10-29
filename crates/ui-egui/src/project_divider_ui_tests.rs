//! Deleting a divider page of a library project says "Page deleted" with no Undo button: only a
//! component is announced as one.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use figcraft_doc::NodeId;
use figcraft_engine::Session;
use serde_json::json;

use crate::i18n;
use crate::layers_drag_tests::Ui;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn frames(u: &mut Ui, n: usize) {
    for _ in 0..n {
        u.frame(vec![]);
    }
}

fn project_ui(tag: &str) -> (Ui, Dir) {
    figcraft_text::scan_system_fonts();
    let dir = std::env::temp_dir().join(format!("figcraft-project-divider-ui-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut u = Ui::new(Session::empty());
    frames(&mut u, 2);
    assert!(u.app.open_project_at(dir.clone(), true));
    frames(&mut u, 3);
    assert!(u.app.session.in_lib_project());
    (u, Dir(dir))
}

fn hyphen_page_with_a_layer(u: &mut Ui) -> NodeId {
    let page = NodeId(u.app.session.execute("page.add", &json!({"name": "Temp"})).unwrap()["id"].as_u64().unwrap());
    u.app.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 40, "height": 40})).unwrap();
    u.app.session.execute("page.rename", &json!({"id": page, "name": "-- Forms"})).unwrap();
    frames(u, 2);
    page
}

// covers: L1-U2
#[test]
fn deleting_a_divider_page_in_a_project_shows_the_plain_page_toast_without_undo() {
    let (mut u, _dir) = project_ui("divider");
    let page = hyphen_page_with_a_layer(&mut u);
    crate::layers::delete_page(&mut u.app, page);
    let toast = u.app.ui.toast.as_ref().map(|(message, _)| message.as_str());
    assert_eq!(toast, Some(i18n::tr("Page deleted")));
    assert!(u.app.ui.toast_action.is_none());
}

// covers: L1-U2
#[test]
fn deleting_a_component_still_offers_undo() {
    let (mut u, _dir) = project_ui("component");
    let page = NodeId(u.app.session.execute("page.add", &json!({"name": "Alpha"})).unwrap()["id"].as_u64().unwrap());
    crate::layers::delete_page(&mut u.app, page);
    let toast = u.app.ui.toast.as_ref().map(|(message, _)| message.as_str());
    assert_eq!(toast, Some(i18n::tr("Component deleted")));
    assert!(u.app.ui.toast_action.is_some());
}
