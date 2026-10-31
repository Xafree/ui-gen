//! npm package names and versions: only registry names and exact versions reach `uigen.json` and the
//! capture service. The same rules live in `sidecar/npm-name.mjs`.

use crate::{ProjectError, invalid};

const MAX_NAME_BYTES: usize = 214;
const MAX_VERSION_BYTES: usize = 64;
const LATEST: &str = "latest";

pub fn validate_package_name(name: &str) -> Result<(), ProjectError> {
    let refuse = |reason: String| Err(invalid("package", reason));
    if name.is_empty() {
        return refuse("package name is empty".to_string());
    }
    if name.len() > MAX_NAME_BYTES {
        return refuse(format!("package name is longer than {MAX_NAME_BYTES} bytes"));
    }
    if name.starts_with('-') {
        return refuse("package name must not start with \"-\"".to_string());
    }
    if name.ends_with(".tgz") {
        return refuse("package name must be a registry name, not an archive".to_string());
    }
    if !is_registry_name(name) {
        return refuse("package name must be a lowercase registry name: \"name\" or \"@scope/name\"".to_string());
    }
    Ok(())
}

pub fn validate_version(version: &str, allow_latest: bool) -> Result<(), ProjectError> {
    let refuse = |reason: String| Err(invalid("version", reason));
    if version.len() > MAX_VERSION_BYTES {
        return refuse(format!("version is longer than {MAX_VERSION_BYTES} bytes"));
    }
    if version == LATEST {
        return if allow_latest { Ok(()) } else { refuse("version \"latest\" is not allowed here, give an exact version".to_string()) };
    }
    if !is_exact_semver(version) {
        return refuse("version must be an exact semver MAJOR.MINOR.PATCH, no range or tag".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackageSpec {
    pub name: String,
    pub version: String,
}

const MANAGERS: [&str; 4] = ["yarn", "pnpm", "npx", "bun"];
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
            Some(other) => return Err(invalid(other, "only \"npm install\" or \"npm i\" is understood here; type the package names")),
            None => return Err(invalid("npm", "type the package names after \"npm install\"")),
        }
    } else if let Some(manager) = tokens.peek().filter(|token| MANAGERS.contains(token)) {
        return Err(invalid(*manager, "only npm is understood here; type the package names"));
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
    Ok(specs)
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
