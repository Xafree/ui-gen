use std::collections::BTreeMap;

use figcraft_doc::{Collection, CollectionId, Document, Mode, ModeId, VariableId, VariableValue};

use super::overrides::Rewrite;

/// A copy of `source` named `name` with `rewrites` written in, appended to the document's
/// collections; the copy stands alone even when `source` extends another collection. None when
/// `source` is not in the document.
pub(crate) fn copy_with(doc: &mut Document, source: &CollectionId, name: &str, rewrites: &[Rewrite]) -> Option<CollectionId> {
    let origin = doc.variables.collection(source)?.clone();
    let id = doc.new_collection_id();
    let modes: BTreeMap<ModeId, ModeId> = origin.modes.iter().enumerate().map(|(i, m)| (m.id.clone(), format!("{id}/mode{}", i + 1))).collect();
    let mode_list = origin.modes.iter().filter_map(|m| Some(Mode { id: modes.get(&m.id)?.clone(), name: m.name.clone() })).collect();
    doc.variables.collections.push(Collection { id: id.clone(), name: name.to_string(), modes: mode_list, extends: None });

    let mut renamed: BTreeMap<VariableId, VariableId> = BTreeMap::new();
    let sources: Vec<_> = doc.variables.variables.iter().filter(|v| v.collection == origin.id).cloned().collect();
    for mut copy in sources {
        let new_id = doc.new_variable_id();
        let old_id = std::mem::replace(&mut copy.id, new_id.clone());
        renamed.insert(old_id, new_id);
        copy.collection = id.clone();
        copy.extends = None;
        copy.overrides.clear();
        copy.values = std::mem::take(&mut copy.values).into_iter().filter_map(|(mode, value)| Some((modes.get(&mode)?.clone(), value))).collect();
        doc.variables.variables.push(copy);
    }
    for v in doc.variables.variables.iter_mut().filter(|v| v.collection == id) {
        for value in v.values.values_mut() {
            if let VariableValue::Alias(target) = value
                && let Some(new_target) = renamed.get(target)
            {
                *target = new_target.clone();
            }
        }
    }
    for rewrite in rewrites {
        let (Some(variable), Some(mode)) = (renamed.get(&rewrite.variable), modes.get(&rewrite.mode)) else { continue };
        if let Some(slot) = doc.variables.variables.iter_mut().find(|v| v.id == *variable) {
            slot.values.insert(mode.clone(), rewrite.value.clone());
        }
    }
    Some(id)
}

/// `rewrites` written into the variables they name, in place; a variable that inherits its value
/// overrides it in the modes written.
pub(crate) fn write_into(doc: &mut Document, rewrites: &[Rewrite]) {
    for rewrite in rewrites {
        let Some(slot) = doc.variables.variables.iter_mut().find(|v| v.id == rewrite.variable) else { continue };
        if slot.extends.is_some() {
            slot.overrides.insert(rewrite.mode.clone());
        }
        slot.values.insert(rewrite.mode.clone(), rewrite.value.clone());
    }
}

#[cfg(test)]
mod tests {
    use figcraft_doc::{Variable, VariableValue};

    use super::*;
    use crate::playground_targets_tests::{DARK, LIGHT, THEME, color, fixture_with, themed_color};

    const ACCENT: &str = "couleur/primaire";

    fn linked() -> Variable {
        let mut link = themed_color("couleur/lien", color("#000000"), color("#000000"));
        link.values = [LIGHT, DARK].into_iter().map(|mode| (mode.to_string(), VariableValue::Alias(format!("v:{ACCENT}")))).collect();
        link
    }

    fn named<'a>(doc: &'a Document, collection: &str, name: &str) -> &'a Variable {
        doc.variables.variables.iter().find(|v| v.collection == collection && v.name == name).expect("the variable is there")
    }

    #[test]
    fn a_copy_keeps_the_aliases_of_its_collection_inside_the_copy() {
        let mut doc = fixture_with(vec![linked()]).doc;
        let blue = Rewrite { variable: format!("v:{ACCENT}"), mode: LIGHT.to_string(), value: VariableValue::Color(color("#2563eb")) };

        let id = copy_with(&mut doc, &THEME.to_string(), "Copy", &[blue]).expect("the theme is in the document");

        let (copied, original) = (named(&doc, &id, ACCENT), named(&doc, THEME, ACCENT));
        assert_ne!(copied.id, original.id);
        assert_eq!(named(&doc, &id, "couleur/lien").values.values().next(), Some(&VariableValue::Alias(copied.id.clone())));
        let modes = &doc.variables.collection(&id).expect("the copy exists").modes;
        let light = modes.iter().find(|m| m.name == "Light").expect("a light mode").id.clone();
        let dark = modes.iter().find(|m| m.name == "Dark").expect("a dark mode").id.clone();
        assert_eq!(copied.values.get(&light), Some(&VariableValue::Color(color("#2563eb"))));
        assert_eq!(copied.values.get(&dark), original.values.get(DARK));
        assert_eq!(original.values.get(LIGHT), Some(&VariableValue::Color(color("#E5532D"))));
    }

    #[test]
    fn a_copy_of_an_unknown_collection_is_not_made() {
        let mut doc = fixture_with(Vec::new()).doc;
        let before = doc.variables.collections.len();

        assert_eq!(copy_with(&mut doc, &"nope".to_string(), "Copy", &[]), None);

        assert_eq!(doc.variables.collections.len(), before);
    }

    #[test]
    fn writing_into_an_inheriting_variable_overrides_it_in_that_mode() {
        let mut doc = fixture_with(Vec::new()).doc;
        let id = format!("v:{ACCENT}");
        doc.variables.variables.iter_mut().filter(|v| v.id == id).for_each(|v| v.extends = Some("v:parent".to_string()));
        let green = Rewrite { variable: id.clone(), mode: DARK.to_string(), value: VariableValue::Color(color("#16a34a")) };

        write_into(&mut doc, &[green]);

        let written = named(&doc, THEME, ACCENT);
        assert_eq!(written.values.get(DARK), Some(&VariableValue::Color(color("#16a34a"))));
        assert_eq!(written.overrides.iter().collect::<Vec<_>>(), [DARK]);
        assert_eq!(written.values.get(LIGHT), Some(&VariableValue::Color(color("#E5532D"))));
    }
}
