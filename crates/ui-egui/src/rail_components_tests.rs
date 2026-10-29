//! The rail's "Assets" entry reads "Components", driven through egui headless: the rail item, the
//! left panel's header and its accessible region carry the new word in English and French, while
//! the Inspect tab's "Assets" section, the palette tab, `LeftTab::Assets` and the `view.assets` menu
//! label keep theirs; the new word is in every catalog; `ui.set {leftTab}` takes "components" as
//! well as "assets".

use egui::accesskit::{Node, Role};
use egui::epaint::ClippedShape;
use egui::{Pos2, Shape};
use figcraft_engine::Session;
use serde_json::json;

use crate::LeftTab;
use crate::i18n::{self, Lang, Text};
use crate::layers_drag_tests::Ui;

const RAIL_WIDTH: f32 = 56.0;
const HEADER_HEIGHT: f32 = 64.0;

fn assets_tab_ui(language: &str) -> Ui {
    figcraft_text::scan_system_fonts();
    let mut u = Ui::new(Session::new());
    u.app.session.prefs.language = language.into();
    u.app.ui.left_collapsed = false;
    u.app.ui.left_tab = LeftTab::Assets;
    u.frame(vec![]);
    u
}

fn painted_texts(shapes: &[ClippedShape]) -> Vec<(String, Pos2)> {
    fn walk(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos)),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

fn frame_with_tree(u: &mut Ui) -> (Vec<Node>, Vec<(String, Pos2)>) {
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    let out = u.frame_output(vec![]);
    let nodes = out.platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect();
    (nodes, painted_texts(&out.shapes))
}

fn has(nodes: &[Node], role: Role, label: &str) -> bool {
    nodes.iter().any(|n| n.role() == role && n.label() == Some(label))
}

fn in_header(texts: &[(String, Pos2)], left_width: f32, word: &str) -> bool {
    texts.iter().any(|(s, p)| s == word && p.x > RAIL_WIDTH && p.x < RAIL_WIDTH + left_width && p.y < HEADER_HEIGHT)
}

fn in_rail(texts: &[(String, Pos2)], word: &str) -> bool {
    let start = word.get(..4).unwrap_or(word);
    texts.iter().any(|(s, p)| s.starts_with(start) && p.x < RAIL_WIDTH)
}

fn assert_renamed(language: &str, new: &str, old: &str) {
    let mut u = assets_tab_ui(language);
    let left_width = u.app.ui.left_width;
    let (nodes, texts) = frame_with_tree(&mut u);
    assert!(has(&nodes, Role::Tab, new), "{language}: the rail item is a tab named {new:?}");
    assert!(has(&nodes, Role::Pane, new), "{language}: the left region is named {new:?}");
    assert!(in_rail(&texts, new), "{language}: the rail label reads {new:?}");
    assert!(in_header(&texts, left_width, new), "{language}: the left panel header reads {new:?}");
    assert!(!has(&nodes, Role::Tab, old), "{language}: no rail tab named {old:?} remains");
    assert!(!has(&nodes, Role::Pane, old), "{language}: no left region named {old:?} remains");
    assert!(!in_rail(&texts, old), "{language}: the rail label no longer reads {old:?}");
    assert!(!in_header(&texts, left_width, old), "{language}: the header no longer reads {old:?}");
}

// covers: L1-R1
#[test]
fn rail_header_and_region_read_components_in_english_and_composants_in_french() {
    assert_renamed("en", "Components", "Assets");
    assert_renamed("fr", "Composants", "Ressources");
}

fn text_of<'a>(table: &'a [(&str, Text)], key: &str) -> Option<&'a str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, t)| match t {
        Text::Plain(s) | Text::Plural { other: s, .. } => *s,
    })
}

// covers: L1-R2
#[test]
fn assets_keeps_its_word_in_the_inspector_the_palette_and_the_view_menu() {
    assert_eq!(LeftTab::Assets, LeftTab::Assets, "the Assets tab variant still exists");
    let fr = i18n::languages().iter().find(|l| l.code == "fr").expect("French is baked in");
    assert_eq!(text_of(fr.ui, "Assets"), Some("Ressources"), "tr(\"Assets\") stays Ressources, for the Inspect section and the palette tab");
    assert_eq!(text_of(fr.menu, "view.assets"), Some("Ressources"), "the view.assets menu label stays Ressources");
    assert_eq!(text_of(fr.commands, "view.assets"), Some("Panneau Ressources"), "the view.assets command label stays");
    let en = i18n::languages().first().expect("English comes first");
    assert_eq!(text_of(en.ui, "Assets"), Some("Assets"));
}

// covers: L1-R2
#[test]
fn the_inspect_tab_section_still_reads_ressources_in_french() {
    figcraft_text::scan_system_fonts();
    let mut s = Session::new();
    let r = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 40, "height": 40})).unwrap();
    s.execute("select.set", &json!({"ids": [r["id"]]})).unwrap();
    let mut u = Ui::new(s);
    u.app.session.prefs.language = "fr".into();
    u.app.ui.right_tab = crate::RightTab::Inspect;
    u.frame(vec![]);
    let (_, texts) = frame_with_tree(&mut u);
    eprintln!("{texts:?}");
    let right_edge = 1440.0 - u.app.ui.right_width;
    assert!(texts.iter().any(|(s, p)| s == "Ressources" && p.x > right_edge), "the Inspect section reads Ressources");
    assert!(!texts.iter().any(|(s, p)| s == "Composants" && p.x > right_edge), "the Inspect section is not renamed");
}

fn has_components(lang: &Lang) -> bool {
    text_of(lang.ui, "Components").is_some_and(|s| !s.trim().is_empty())
}

// covers: L1-R3
#[test]
fn components_is_translated_and_not_empty_in_every_catalog() {
    let langs = i18n::languages();
    assert_eq!(langs.len(), 22);
    for lang in langs {
        let inherited = lang.parent != 0 && langs.get(lang.parent).is_some_and(has_components);
        assert!(has_components(lang) || inherited, "{}: no non-empty \"Components\" key in locales/{}.toml", lang.code, lang.code);
    }
}

fn set_view(u: &mut Ui, left_tab: &str) {
    let ctx = u.ctx.clone();
    crate::control::handle(&mut u.app, &ctx, "ui.set", &json!({"leftTab": left_tab})).unwrap();
}

// covers: L1-R4
#[test]
fn set_view_left_tab_components_and_assets_both_select_the_assets_tab() {
    let mut u = Ui::new(Session::new());
    for alias in ["components", "assets"] {
        set_view(&mut u, "file");
        assert_eq!(u.app.ui.left_tab, LeftTab::File, "file selects the File tab");
        set_view(&mut u, alias);
        assert_eq!(u.app.ui.left_tab, LeftTab::Assets, "{alias:?} selects LeftTab::Assets");
    }
}
