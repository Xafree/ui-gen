//! `uigen.compare`: a variant of an imported component measured against the original the import kept.

use serde_json::{Value, json};

use super::CommandSpec;
use super::project_sources::binding;
use crate::params::{get, opt_id};
use crate::uigen_compare::measure::{self, MAX_VARIANTS, Row, Status};
use crate::uigen_import::Theme;
use crate::{EngineError, Result, Session, bad, spec};

const COMPARE: &str = "uigen.compare";
const PANEL: &str = "panel.compare";

/// The commands that only read the document: they leave its revision and the undo stack alone, so a panel
/// showing their result is not recomputed by asking for it.
pub(crate) fn reads_document_only(id: &str) -> bool {
    matches!(id, COMPARE | "uigen.sourceDiff" | "uigen.themes")
}

pub(crate) fn is_journaled(id: &str) -> bool {
    id == COMPARE
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            PANEL,
            "Compare",
            "{}  asks the interface to open the Compare tab of the right panel (no panel from a programmatic call); refused outside a library project",
            false,
            open_panel
        ),
        spec!(
            COMPARE,
            "Compare with the original",
            "{set? (a component set with uigen.source, an import or its copy in the library; default the one of the current page), theme? (\"light\" | \"dark\", default \"light\"), variants? (names of variants of the set; default all, 128 at most)}  measures each variant against the image its import kept in .uigen/captures and returns {set, name, source: {package, version, component}, theme, rows: [{variant, style, size, state, status (\"ok\" | \"ko\" | \"missingReference\" | \"notComparable\" | \"sizeMismatch\"), reason? (why the row could not be measured), original: {width, height}, library: {width, height}, shapes: {differing, total, percent, verdict} | null (pixels outside the text boxes; under 1 % is ok), text: {verdict, reason, maxDx, maxDy, maxDwidth, pixelPercent} | null (family, size, weight, position and measured width; the pixels of text are information only)}], summary: {compared, ok, ko, missing, notComparable}}. A missing, unreadable or oversized (8 MiB, 4096 × 4096) image is a `missingReference` row, never an error; a dark render that equals the light one (the library does not follow the system colour scheme) is `notComparable`. Writes nothing: the document is rendered on a copy. Refused (`bad`) outside a library project, for a set that is missing, not a component set, without uigen.source or of more than 128 variants, an unknown theme and unknown variants",
            false,
            compare
        ),
    ]
}

fn open_panel(s: &mut Session, _: &Value) -> Result<Value> {
    binding(s, PANEL)?;
    s.request_ui(PANEL, json!({}));
    Ok(json!({}))
}

fn theme_param(p: &Value) -> Result<Theme> {
    match get(p, "theme") {
        None => Ok(Theme::Light),
        Some(value) => value.as_str().and_then(Theme::parse).ok_or_else(|| bad(COMPARE, "`theme` must be \"light\" or \"dark\"")),
    }
}

fn variants_param(p: &Value) -> Result<Option<Vec<String>>> {
    let Some(value) = get(p, "variants") else { return Ok(None) };
    let items = value.as_array().ok_or_else(|| bad(COMPARE, "`variants` must be a list of variant names"))?;
    if items.len() > MAX_VARIANTS {
        return Err(bad(COMPARE, format!("`variants` names {} variants: at most {MAX_VARIANTS} can be compared", items.len())));
    }
    let names = items.iter().map(|item| item.as_str().map(str::to_owned));
    names.collect::<Option<Vec<String>>>().map(Some).ok_or_else(|| bad(COMPARE, "`variants` must be a list of variant names"))
}

fn count(rows: &[Row], statuses: &[Status]) -> usize {
    rows.iter().filter(|row| statuses.contains(&row.status)).count()
}

fn summary(rows: &[Row]) -> Value {
    json!({
        "compared": count(rows, &[Status::Ok, Status::Ko, Status::SizeMismatch]),
        "ok": count(rows, &[Status::Ok]),
        "ko": count(rows, &[Status::Ko, Status::SizeMismatch]),
        "missing": count(rows, &[Status::MissingReference]),
        "notComparable": count(rows, &[Status::NotComparable]),
    })
}

fn compare(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, COMPARE)?;
    let theme = theme_param(p)?;
    let wanted = variants_param(p)?;
    let set = opt_id(COMPARE, p, "set")?;
    let doc = s.doc()?;
    let subject = measure::subject(doc, set, s.page()?, wanted.as_deref()).map_err(|why| bad(COMPARE, why))?;
    let rows = measure::measure(doc, &binding.root, theme, &subject.variants);
    let listed = serde_json::to_value(&rows).map_err(|error| EngineError::Other(error.to_string()))?;
    let source = &subject.source;
    Ok(json!({
        "set": subject.set,
        "name": subject.name,
        "source": {"package": source.package, "version": source.version, "component": source.component},
        "theme": theme.render(),
        "rows": listed,
        "summary": summary(&rows),
    }))
}
