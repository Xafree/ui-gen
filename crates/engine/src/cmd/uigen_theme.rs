//! `uigen.themes`: the token themes of the file. `uigen.applyTheme`: the layers of the library follow
//! another collection, and `uigen.json` remembers which one.

use figcraft_doc::Document;
use serde_json::{Value, json};

use super::CommandSpec;
use super::project::remember_theme;
use super::project_sources::binding;
use crate::library::{is_remote_collection, local_collections};
use crate::uigen_theme::{Swatch, ThemeInfo, most_followed, rebind, rebinding, themes};
use crate::{Result, Session, bad, spec};

const LIST: &str = "uigen.themes";
const APPLY: &str = "uigen.applyTheme";
const MAX_THEME_CHARS: usize = 200;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            LIST,
            "List token themes",
            "{}  the variable collections of the file (library copies left out), in file order: {applied (the appliedTheme of uigen.json, or null), inSync (true when the applied theme exists and no binding of the library is left to move to it; with no applied theme, when no layer follows a collection), themes: [{collection (id), name, origin ({package, version} for a collection an import made, else null), modes, variables, boundLayers (layers of the library components that follow it; import pages and notes do not count), applied, swatches: [{name, light, dark}] (colour variables, 8 at most, \"#RRGGBB\")}]}. Refused (`bad`) outside a library project",
            false,
            list
        ),
        spec!(
            APPLY,
            "Apply token theme",
            "{theme (the name or id of a local variable collection)}  in one undo step, every variable binding of the library components whose variable is not in that collection moves to the variable of the same name (trimmed, case apart) and type in it; no value of any layer changes. Then `appliedTheme` of uigen.json is written. Returns {theme, previous (the former appliedTheme or null), layers (layers with a changed binding), rebound (changed bindings), unmatched (names of variables with no equivalent, which keep their binding; 50 at most), unchanged}. Applying the theme already applied with no binding left to move returns `unchanged: true` and writes nothing, even when other bindings have no equivalent. Refused (`Disabled`, uigen.json untouched) while a version is viewed or a live session is read-only. Refused (`bad`, document and uigen.json untouched) outside a library project, for an unknown collection or a library copy, and when no binding has an equivalent",
            true,
            apply
        ),
    ]
}

fn swatch_json(swatch: &Swatch) -> Value {
    json!({"name": swatch.name, "light": swatch.light, "dark": swatch.dark})
}

fn theme_json(theme: &ThemeInfo, applied: Option<&str>) -> Value {
    let origin = theme.origin.as_ref().map(|(package, version)| json!({"package": package, "version": version}));
    let swatches: Vec<Value> = theme.swatches.iter().map(swatch_json).collect();
    json!({
        "collection": theme.collection,
        "name": theme.name,
        "origin": origin,
        "modes": theme.modes,
        "variables": theme.variables,
        "boundLayers": theme.bound_layers,
        "applied": applied == Some(theme.name.as_str()),
        "swatches": swatches,
    })
}

fn in_sync(doc: &Document, applied: Option<&str>) -> bool {
    let Some(name) = applied else { return most_followed(doc).is_none() };
    local_collections(doc).find(|collection| collection.name == name).is_some_and(|collection| rebinding(doc, &collection.id).rebound() == 0)
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    let binding = binding(s, LIST)?;
    let doc = s.doc()?;
    let applied = binding.config.applied_theme.as_deref();
    let in_sync = in_sync(doc, applied);
    let rows: Vec<Value> = themes(doc, &binding.config).iter().map(|theme| theme_json(theme, applied)).collect();
    Ok(json!({"applied": applied, "inSync": in_sync, "themes": rows}))
}

fn theme_param(p: &Value) -> Result<&str> {
    let theme = p.get("theme").ok_or_else(|| bad(APPLY, "theme is required"))?.as_str().ok_or_else(|| bad(APPLY, "theme must be a string"))?.trim();
    if theme.is_empty() {
        return Err(bad(APPLY, "theme must not be empty"));
    }
    if theme.chars().count() > MAX_THEME_CHARS {
        return Err(bad(APPLY, format!("theme is longer than {MAX_THEME_CHARS} characters")));
    }
    if theme.chars().any(char::is_control) {
        return Err(bad(APPLY, "theme must not contain control characters"));
    }
    Ok(theme)
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, APPLY)?;
    let wanted = theme_param(p)?;
    let doc = s.doc()?;
    let collections = &doc.variables.collections;
    let target = collections
        .iter()
        .find(|collection| collection.id == wanted)
        .or_else(|| collections.iter().find(|collection| collection.name == wanted))
        .ok_or_else(|| bad(APPLY, format!("no variable collection named `{wanted}`")))?;
    if is_remote_collection(doc, &target.id) {
        let themes: Vec<&str> = local_collections(doc).map(|collection| collection.name.as_str()).collect();
        return Err(bad(APPLY, format!("`{}` is a library copy, not a token theme of this file (themes: {})", target.name, themes.join(", "))));
    }
    let (id, name) = (target.id.clone(), target.name.clone());
    let plan = rebinding(doc, &id);
    let previous = binding.config.applied_theme.clone();
    let unchanged = plan.rebound() == 0 && previous.as_deref() == Some(name.as_str());
    if !unchanged && plan.rebound() == 0 && !plan.unmatched.is_empty() {
        return Err(bad(APPLY, format!("no variable bound in the library has an equivalent in `{name}`: nothing to apply")));
    }
    if !unchanged {
        s.require_editable(APPLY)?;
        if plan.rebound() > 0 {
            let baseline = s.edit_baseline();
            rebind(s.doc_mut()?, &plan)?;
            s.check_instance_structure(APPLY, &baseline)?;
        }
        remember_theme(s, APPLY, &name)?;
    }
    Ok(json!({
        "theme": name,
        "previous": previous,
        "layers": plan.layers,
        "rebound": plan.rebound(),
        "unmatched": plan.unmatched,
        "unchanged": unchanged,
    }))
}
