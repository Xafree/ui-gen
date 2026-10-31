//! npm package names and versions: only registry names and exact versions reach `uigen.json` and the
//! capture service. The same rules live in `sidecar/npm-name.mjs`.

use crate::{ProjectError, invalid};

const MAX_NAME_BYTES: usize = 214;
const MAX_VERSION_BYTES: usize = 64;
const LATEST: &str = "latest";
const NAME_EMPTY: &str = "package name is empty";
const NAME_DASH: &str = "package name must not start with \"-\"";
const NAME_ARCHIVE: &str = "package name must be a registry name, not an archive";
const NAME_SHAPE: &str = "package name must be a lowercase registry name: \"name\" or \"@scope/name\"";
const LATEST_REFUSED: &str = "version \"latest\" is not allowed here, give an exact version";
const VERSION_SHAPE: &str = "version must be an exact semver MAJOR.MINOR.PATCH, no range or tag";
const ONLY_NPM_INSTALL: &str = "only \"npm install\" or \"npm i\" is understood here; type the package names";
const NPM_WITHOUT_PACKAGES: &str = "type the package names after \"npm install\"";
const ONLY_NPM: &str = "only npm is understood here; type the package names";

pub fn validate_package_name(name: &str) -> Result<(), ProjectError> {
    let refuse = |reason: String| Err(invalid("package", reason));
    if name.is_empty() {
        return refuse(NAME_EMPTY.to_string());
    }
    if name.len() > MAX_NAME_BYTES {
        return refuse(format!("package name is longer than {MAX_NAME_BYTES} bytes"));
    }
    if name.starts_with('-') {
        return refuse(NAME_DASH.to_string());
    }
    if name.ends_with(".tgz") {
        return refuse(NAME_ARCHIVE.to_string());
    }
    if !is_registry_name(name) {
        return refuse(NAME_SHAPE.to_string());
    }
    Ok(())
}

pub fn validate_version(version: &str, allow_latest: bool) -> Result<(), ProjectError> {
    let refuse = |reason: String| Err(invalid("version", reason));
    if version.len() > MAX_VERSION_BYTES {
        return refuse(format!("version is longer than {MAX_VERSION_BYTES} bytes"));
    }
    if version == LATEST {
        return if allow_latest { Ok(()) } else { refuse(LATEST_REFUSED.to_string()) };
    }
    if !is_exact_semver(version) {
        return refuse(VERSION_SHAPE.to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackageSpec {
    pub name: String,
    pub version: String,
}

const MANAGERS: [&str; 4] = ["yarn", "pnpm", "npx", "bun"];
/// What the capture service takes in one install (`sidecar/install.mjs`).
pub const MAX_PACKAGES: usize = 32;
const TOO_MANY_START: &str = " packages on one line: ";
const TOO_MANY_END: &str = " at most";
const EMPTY_LINE: &str = "type at least one package, for example: @scope/name another@1.2.0";

pub fn parse_install_line(line: &str) -> Result<Vec<PackageSpec>, ProjectError> {
    let mut tokens = line.split_whitespace().peekable();
    if tokens.peek().is_none() {
        return Err(invalid("line", EMPTY_LINE));
    }
    if tokens.peek() == Some(&"npm") {
        tokens.next();
        match tokens.next() {
            Some("install" | "i") => {}
            Some(other) => return Err(invalid(other, ONLY_NPM_INSTALL)),
            None => return Err(invalid("npm", NPM_WITHOUT_PACKAGES)),
        }
    } else if let Some(manager) = tokens.peek().filter(|token| MANAGERS.contains(token)) {
        return Err(invalid(*manager, ONLY_NPM));
    }
    let mut specs: Vec<PackageSpec> = Vec::new();
    for token in tokens {
        let spec = parse_token(token)?;
        if !specs.contains(&spec) {
            specs.push(spec);
        }
    }
    if specs.is_empty() {
        return Err(invalid("line", EMPTY_LINE));
    }
    if specs.len() > MAX_PACKAGES {
        return Err(invalid("line", format!("{}{TOO_MANY_START}{MAX_PACKAGES}{TOO_MANY_END}", specs.len())));
    }
    Ok(specs)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineRefusal {
    Empty,
    TooMany { count: usize, max: usize },
    NotInstall(String),
    NoPackagesAfterNpm,
    OtherManager(String),
    Token { token: String, problem: TokenProblem },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenProblem {
    NameEmpty,
    NameTooLong(usize),
    NameStartsWithDash,
    NameIsArchive,
    NameNotRegistry,
    VersionTooLong(usize),
    LatestNotAllowed,
    VersionNotExact,
}

/// What `parse_install_line` refused, as a kind the caller can word in its own language.
pub fn line_refusal(error: &ProjectError) -> Option<LineRefusal> {
    let ProjectError::Invalid { field, reason } = error else { return None };
    let reason = reason.as_str();
    match (field.as_str(), reason) {
        ("line", EMPTY_LINE) => Some(LineRefusal::Empty),
        ("line", _) => too_many(reason),
        (_, ONLY_NPM_INSTALL) => Some(LineRefusal::NotInstall(field.clone())),
        ("npm", NPM_WITHOUT_PACKAGES) => Some(LineRefusal::NoPackagesAfterNpm),
        (_, ONLY_NPM) => Some(LineRefusal::OtherManager(field.clone())),
        _ => token_problem(reason).map(|problem| LineRefusal::Token { token: field.clone(), problem }),
    }
}

fn too_many(reason: &str) -> Option<LineRefusal> {
    let (count, rest) = reason.split_once(TOO_MANY_START)?;
    Some(LineRefusal::TooMany { count: count.parse().ok()?, max: rest.strip_suffix(TOO_MANY_END)?.parse().ok()? })
}

fn token_problem(reason: &str) -> Option<TokenProblem> {
    let length = |text: &str| reason.strip_prefix(text)?.strip_suffix(" bytes")?.parse().ok();
    match reason {
        NAME_EMPTY => Some(TokenProblem::NameEmpty),
        NAME_DASH => Some(TokenProblem::NameStartsWithDash),
        NAME_ARCHIVE => Some(TokenProblem::NameIsArchive),
        NAME_SHAPE => Some(TokenProblem::NameNotRegistry),
        LATEST_REFUSED => Some(TokenProblem::LatestNotAllowed),
        VERSION_SHAPE => Some(TokenProblem::VersionNotExact),
        _ => length("package name is longer than ")
            .map(TokenProblem::NameTooLong)
            .or_else(|| length("version is longer than ").map(TokenProblem::VersionTooLong)),
    }
}

fn parse_token(token: &str) -> Result<PackageSpec, ProjectError> {
    let (name, version) = match token.char_indices().skip(1).find(|(_, c)| *c == '@') {
        Some((at, _)) => (&token[..at], &token[at + 1..]),
        None => (token, LATEST),
    };
    let name_of_token = |error: ProjectError| match error {
        ProjectError::Invalid { reason, .. } => invalid(token, reason),
        other => other,
    };
    validate_package_name(name).map_err(name_of_token)?;
    validate_version(version, true).map_err(name_of_token)?;
    Ok(PackageSpec { name: name.to_string(), version: version.to_string() })
}

fn is_registry_name(name: &str) -> bool {
    match name.strip_prefix('@') {
        Some(scoped) => scoped.split_once('/').is_some_and(|(scope, rest)| is_name_part(scope) && is_name_part(rest)),
        None => is_name_part(name),
    }
}

fn is_name_part(part: &str) -> bool {
    let mut bytes = part.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit() || first == b'~')
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

fn is_exact_semver(version: &str) -> bool {
    let (rest, build) = match version.split_once('+') {
        Some((rest, build)) => (rest, Some(build)),
        None => (version, None),
    };
    let (core, pre_release) = match rest.split_once('-') {
        Some((core, pre_release)) => (core, Some(pre_release)),
        None => (rest, None),
    };
    is_core(core) && pre_release.is_none_or(are_identifiers) && build.is_none_or(are_identifiers)
}

fn is_core(core: &str) -> bool {
    let mut parts = core.split('.');
    let (Some(major), Some(minor), Some(patch), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    [major, minor, patch].into_iter().all(is_number)
}

fn is_number(part: &str) -> bool {
    part == "0" || (!part.starts_with('0') && !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn are_identifiers(list: &str) -> bool {
    list.split('.').all(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}
