use super::{GEOMETRY_TOLERANCE_PX, LibText, RefText, TextReport, Verdict};

const EPSILON: f64 = 1e-9;
const REGULAR: u16 = 400;

const WEIGHT_NAMES: [(&str, u16); 12] = [
    ("thin", 100),
    ("extralight", 200),
    ("ultralight", 200),
    ("light", 300),
    ("medium", 500),
    ("semibold", 600),
    ("demibold", 600),
    ("extrabold", 800),
    ("ultrabold", 800),
    ("black", 900),
    ("bold", 700),
    ("heavy", 900),
];

const STYLE_WORDS: [&str; 14] =
    ["thin", "extra", "ultra", "semi", "demi", "light", "regular", "normal", "medium", "bold", "black", "heavy", "italic", "oblique"];

struct Pair<'a> {
    family_ok: bool,
    size_ok: bool,
    weight_ok: bool,
    installed: bool,
    measured: bool,
    library_family: &'a str,
    deltas: [f64; 3],
}

fn squash(name: &str) -> String {
    name.chars().filter(|character| character.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn is_style_suffix(mut rest: &str) -> bool {
    while !rest.is_empty() {
        match STYLE_WORDS.iter().find_map(|word| rest.strip_prefix(word)) {
            Some(after) => rest = after,
            None => return false,
        }
    }
    true
}

fn family_matches(reference: &str, library: &str) -> bool {
    let actual = squash(library);
    if actual.is_empty() {
        return false;
    }
    let expected = squash(reference);
    expected == actual || expected.strip_prefix(actual.as_str()).is_some_and(is_style_suffix)
}

pub fn weight_of_font_style(style: &str) -> u16 {
    let squashed = squash(style);
    WEIGHT_NAMES.iter().find(|(word, _)| squashed.contains(word)).map_or(REGULAR, |(_, weight)| *weight)
}

impl<'a> Pair<'a> {
    fn of(reference: &RefText, library: &'a LibText) -> Pair<'a> {
        let expected = reference.line.unwrap_or(reference.box_);
        Pair {
            family_ok: family_matches(&reference.family, &library.family),
            size_ok: (reference.size - library.size).abs() < EPSILON,
            weight_ok: reference.weight == library.weight,
            installed: library.installed,
            measured: library.width_measured,
            library_family: &library.family,
            deltas: [library.x - expected[0], library.y - expected[1], library.width - expected[2]],
        }
    }

    fn within_tolerance(&self) -> bool {
        self.deltas.iter().all(|delta| delta.is_finite() && delta.abs() <= GEOMETRY_TOLERANCE_PX + EPSILON)
    }
}

fn worst(values: impl Iterator<Item = f64>) -> f64 {
    values.filter(|value| value.is_finite()).fold(0.0, |worst, value| if value.abs() > worst.abs() { value } else { worst })
}

fn missing_families<'a>(pairs: &[Pair<'a>]) -> Vec<&'a str> {
    let mut families: Vec<&str> = Vec::new();
    for pair in pairs.iter().filter(|pair| !pair.installed) {
        if !families.contains(&pair.library_family) {
            families.push(pair.library_family);
        }
    }
    families
}

fn failure_reasons(pairs: &[Pair]) -> Vec<String> {
    let missing = missing_families(pairs);
    let checks = [
        (!missing.is_empty(), format!("police absente : {}", missing.join(", "))),
        (pairs.iter().any(|pair| !pair.measured), "largeur non mesurée (le calque texte n'est pas à taille automatique)".to_owned()),
        (pairs.iter().any(|pair| !pair.family_ok), "police différente".to_owned()),
        (pairs.iter().any(|pair| !pair.size_ok), "taille différente".to_owned()),
        (pairs.iter().any(|pair| !pair.weight_ok), "graisse différente".to_owned()),
        (pairs.iter().any(|pair| !pair.within_tolerance()), format!("écart de position ou de largeur au-delà de {GEOMETRY_TOLERANCE_PX} px")),
    ];
    checks.into_iter().filter_map(|(failed, reason)| failed.then_some(reason)).collect()
}

fn mismatch(reason: String) -> TextReport {
    TextReport { verdict: Verdict::Ko, reason, max_dx: 0.0, max_dy: 0.0, max_dwidth: 0.0 }
}

pub fn text_verdict(reference: &[RefText], library: &[LibText]) -> TextReport {
    if reference.len() != library.len() {
        return mismatch(format!("{} textes capturés, {} calque(s) texte", reference.len(), library.len()));
    }
    let pairs: Vec<Pair> = reference.iter().zip(library).map(|(text, layer)| Pair::of(text, layer)).collect();
    let reasons = failure_reasons(&pairs);
    TextReport {
        verdict: if reasons.is_empty() { Verdict::Ok } else { Verdict::Ko },
        reason: reasons.join(" ; "),
        max_dx: worst(pairs.iter().map(|pair| pair.deltas[0])),
        max_dy: worst(pairs.iter().map(|pair| pair.deltas[1])),
        max_dwidth: worst(pairs.iter().map(|pair| pair.deltas[2])),
    }
}
