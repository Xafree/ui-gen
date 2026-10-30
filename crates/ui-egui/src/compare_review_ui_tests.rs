//! Review fixes of the Compare tab: a set of an import page reads no difference and raises no
//! error, a drag never restarts the measure on every frame and keeps the rows it has, a detail
//! whose row is gone closes, a difference tells where it is and whether it can be taken.

use std::path::PathBuf;

use figcraft_doc::NodeId;
use serde_json::{Value, json};

use crate::compare_tab::{Settle, Stamp};
use crate::compare_updates::Entry;
use crate::i18n;
use crate::layers_drag_tests::Ui;

const PACKAGE: &str = "synthetic-ui";
const COMPARE: &str = "uigen.compare";
const FRAMES: usize = 60;

struct Lab {
    ui: Ui,
    set: NodeId,
    variants: Vec<NodeId>,
    root: PathBuf,
}

impl Drop for Lab {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

impl Lab {
    fn new(tag: &str, import_page: bool, variants: &[(&str, u32)]) -> Self {
        figcraft_text::scan_system_fonts();
        let root = std::env::temp_dir().join(format!("uigen-review-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = json!({"package": PACKAGE, "version": "1.0.0", "framework": "react", "license": "MIT", "removed": false});
        let manifest = json!({"version": 1, "name": "lib", "designFiles": ["lib.figcraft"], "sources": [source]});
        std::fs::write(root.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        let doc = figcraft_doc::Document::new("lib");
        std::fs::write(root.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
        let mut ui = Ui::new(figcraft_engine::Session::empty());
        ui.app.session.prefs.language = "en".into();
        assert!(ui.app.open_project_at(root.clone(), false), "the project opens");
        ui.app.ui.home = false;
        let mut lab = Self { ui, set: NodeId(0), variants: Vec::new(), root };
        lab.build(import_page, variants);
        lab
    }

    fn run(&mut self, id: &str, params: Value) -> Value {
        self.ui.app.session.execute(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn build(&mut self, import_page: bool, variants: &[(&str, u32)]) {
        let node = |made: Value| NodeId(made["id"].as_u64().unwrap());
        let mut mains = Vec::new();
        for (index, (name, width)) in variants.iter().enumerate() {
            let frame = json!({"x": index * 300, "y": 0, "width": width, "height": 40, "name": name});
            let frame = node(self.run("frame.create", frame));
            mains.push(node(self.run("component.create", json!({"ids": [frame.0]}))));
        }
        self.set = node(self.run("component.combineVariants", json!({"ids": mains.iter().map(|id| id.0).collect::<Vec<_>>()})));
        let doc = self.ui.app.session.doc().unwrap();
        let page = doc.pages[0];
        let origin = json!({"package": PACKAGE, "version": "1.0.0", "component": "Button"}).to_string();
        self.run("node.set", json!({"id": self.set.0, "props": {"pluginData": {"uigen.source": origin}}}));
        if import_page {
            let marker = json!({"package": PACKAGE, "version": "1.0.0"}).to_string();
            self.run("node.set", json!({"id": page.0, "props": {"pluginData": {"uigen.importPage": marker}}}));
        }
        self.variants = mains;
        self.run("select.set", json!({"ids": [self.set.0]}));
        self.frames(3);
        self.ui.app.run("panel.compare", json!({}));
        self.ui.app.ui.toast = None;
        self.ui.app.ui.last_error = None;
    }

    fn frames(&mut self, count: usize) {
        for _ in 0..count {
            self.ui.frame(vec![]);
        }
    }

    fn visible_rows(&self) -> usize {
        self.ui.app.ui.compare.run().map_or(0, |run| run.visible().len())
    }

    fn measures_since(&self, mark: usize) -> usize {
        self.ui.app.session.journal[mark..].iter().filter(|(command, _)| command == COMPARE).count()
    }

    fn widen(&mut self, variant: NodeId, width: usize) {
        self.run("node.set", json!({"id": variant.0, "props": {"width": width}}));
    }
}

// covers: E5-7.5
#[test]
fn a_set_in_an_import_page_reads_no_difference_and_raises_no_error() {
    let mut lab = Lab::new("import-page", true, &[("State=A", 100), ("State=B", 120)]);
    lab.ui.app.toast_with_undo("kept");

    lab.frames(10);
    lab.widen(lab.variants[0], 140);
    lab.frames(40);

    assert_eq!(lab.ui.app.ui.last_error, None, "reading the differences of an import page is not an error");
    assert_eq!(lab.ui.app.ui.toast.as_ref().map(|(text, _)| text.as_str()), Some("kept"), "no refusal replaces the toast");
}

// covers: E5-7.7
#[test]
fn a_drag_measures_once_and_keeps_the_rows_until_the_edit_is_over() {
    let mut lab = Lab::new("drag", false, &[("State=A", 100), ("State=B", 120), ("State=C", 140)]);
    lab.frames(FRAMES);
    assert_eq!(lab.visible_rows(), 3, "the three states are measured");
    let mark = lab.ui.app.session.journal.len();

    for step in 0..40 {
        lab.widen(lab.variants[0], 100 + step);
        lab.ui.frame(vec![]);
        assert_eq!(lab.visible_rows(), 3, "the rows stay while the document changes (step {step})");
    }
    let during = lab.measures_since(mark);
    assert!(during <= 3, "a drag restarts the measure once, not on every frame: {during} measures");

    lab.frames(FRAMES);
    let revision = lab.ui.app.session.ds().unwrap().revision;
    assert_eq!(lab.ui.app.ui.compare.run().map(|run| run.revision()), Some(revision), "measured again once the edit is over");
    assert_eq!(lab.visible_rows(), 3);
    assert!(lab.measures_since(mark) >= during + 3, "every state is measured again");
}

// covers: E5-7.7
#[test]
fn two_states_with_the_same_name_each_get_their_own_measure() {
    let mut lab = Lab::new("twins", false, &[("State=A", 100), ("State=A", 160)]);
    lab.frames(FRAMES);

    let sizes: Vec<(u32, u32)> = lab.ui.app.ui.compare.run().unwrap().visible().iter().map(|row| row.library).collect();

    assert_eq!(sizes.len(), 2);
    assert_ne!(sizes[0], sizes[1], "the second row is not the first one's measure: {sizes:?}");
}

// covers: E5-7.8
#[test]
fn a_detail_whose_row_is_gone_closes_instead_of_staying_invisible() {
    let mut lab = Lab::new("gone", false, &[("State=A", 100), ("State=B", 120)]);
    lab.frames(FRAMES);
    lab.ui.app.ui.compare.detail = Some(NodeId(9_999_999));

    lab.frames(2);

    assert_eq!(lab.ui.app.ui.compare.detail, None);
}

// covers: E5-7.7
#[test]
fn the_first_change_after_a_quiet_moment_is_taken_at_once_and_a_burst_waits_for_calm() {
    let mut settle = Settle::default();
    let stamp = |revision| Stamp { revision, finished_job: 0 };

    assert_eq!(settle.due(stamp(1), 10.0, false), Ok(()), "the first change follows a quiet moment");
    assert!(settle.due(stamp(2), 10.02, false).is_err(), "the next one, a frame later, waits");
    assert!(settle.due(stamp(3), 10.1, false).is_err(), "and waits again for each change");
    assert!(settle.due(stamp(3), 10.3, false).is_err(), "less than 300 ms after the last change");
    assert_eq!(settle.due(stamp(3), 10.45, false), Ok(()), "300 ms without change");
    assert!(settle.due(stamp(4), 20.0, true).is_err(), "a pointer held down waits even after a quiet moment");
    assert!(settle.due(stamp(4), 20.5, true).is_err(), "and keeps waiting while it is down");
    assert_eq!(settle.due(stamp(4), 20.6, false), Ok(()), "released");
}

// covers: E5-7.5
#[test]
fn a_difference_at_the_root_of_a_variant_names_the_variant_and_one_that_cannot_be_taken_says_so() {
    let root = json!({"id": "State=Hover||fill", "variant": "State=Hover", "path": "", "kind": "changed", "field": "fill", "state": "clean", "applicable": true});
    let other =
        json!({"id": "x", "variant": "State=Hover", "path": "Icon", "kind": "changed", "field": "other", "state": "modified", "applicable": false});

    let (root, other) = (Entry::of(&root).unwrap(), Entry::of(&other).unwrap());

    assert_eq!(root.place(), "State=Hover");
    assert!(root.applicable);
    assert_eq!(other.place(), "State=Hover · Icon");
    assert!(!other.applicable);
}

// covers: E5-7.8
#[test]
fn every_language_translates_the_texts_of_the_review_fixes() {
    let keys = ["Can't be taken automatically: change your copy by hand.", "Use version {version}", "Not applied: {reason}"];
    let languages = i18n::languages();
    let mut missing = Vec::new();
    for lang in languages.iter().filter(|l| l.code != "en") {
        let own: Vec<&i18n::Lang> = [Some(lang), languages.get(lang.parent)].into_iter().flatten().filter(|l| l.code != "en").collect();
        for key in keys {
            if !own.iter().any(|l| l.ui.iter().any(|(k, _)| *k == key)) {
                missing.push(format!("{}: {key:?}", lang.code));
            }
        }
    }
    assert!(missing.is_empty(), "untranslated:\n{}", missing.join("\n"));
}
