//! Source libraries of a library project: the commands hand an npm job to the capture service and
//! `uigen.json` changes only when [`apply`] brings the success back through `Session::pump_capture`.

use std::path::{self, Path};
use std::sync::Arc;

use figcraft_project::{Project, Source, WORK_DIR, npm};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::capture::{CaptureError, CaptureErrorKind, CaptureJob, JobState};
use crate::file_access::{self, Access};
use crate::project::{self, ProjectBinding};
use crate::{EngineError, Result, Session, bad, spec};

const ADD: &str = "project.addSource";
const ADD_MANY: &str = "project.addSources";
const SET_VERSION: &str = "project.setSourceVersion";
const REMOVE: &str = "project.removeSource";
const REINSTALL: &str = "project.reinstallSources";
const LATEST: &str = "latest";
const SOURCES_DIR: &str = "sources";
const FRAMEWORKS: [&str; 5] = ["react", "vue", "svelte", "web-components", "unknown"];
const MAX_LICENSE_BYTES: usize = 128;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "project.addSource",
            "Add source library",
            "{package (an npm registry name, e.g. \"@mui/material\"), version? (exact semver or \"latest\", default \"latest\")}  asks the capture service to install the package in the project's .uigen/sources folder and returns {job} at once; uigen.json gets the source (exact version, framework, license) only when the install succeeds, through pump_capture. A source removed earlier comes back",
            false,
            add
        ),
        spec!(
            "project.addSources",
            "Add source libraries",
            "{line (a string)}  the names of one or several npm registry packages separated by spaces, each \"name\" or \"name@exact-version\", optionally after \"npm install\" or \"npm i\", e.g. \"@scope/a b@1.2.0\": asks the capture service to install them all in one go in the project's .uigen/sources folder and returns {job} at once; uigen.json gets every source only when the install succeeds, none otherwise. A package already a source of the project refuses the line",
            false,
            add_many
        ),
        spec!(
            "project.setSourceVersion",
            "Change source library version",
            "{package, version (exact semver or \"latest\")} or {packages (an array of package names), version}  asks the capture service to install that version of a source of the project, or of every listed source in one job, and returns {job}; uigen.json keeps the old versions unless the install succeeds, and then changes them all in one write",
            false,
            set_version
        ),
        spec!(
            "project.removeSource",
            "Remove source library",
            "{package}  asks the capture service to uninstall a source of the project and returns {job}; once it succeeds the source stays in uigen.json with removed: true (its version kept, .uigen/captures untouched)",
            false,
            remove
        ),
        spec!(
            "project.reinstallSources",
            "Reinstall source libraries",
            "{}  asks the capture service to install again, in one job, every active source of the project at the exact version of uigen.json, for a project whose .uigen folder is gone, and returns {job}; removed sources stay out, and framework and license stay as they are when the install reports none. A project with no active source is refused",
            false,
            reinstall
        ),
        spec!(
            "capture.status",
            "Capture jobs",
            "{job? (a whole number: only that job)}  the jobs submitted to the capture service: {jobs: [{job, command, package, root (the project folder), state: \"queued\"|\"running\"|\"done\"|\"failed\", error? (the message of a failed job)}]}, oldest first; a `job` that never ran or is forgotten is refused",
            false,
            status
        ),
    ]
}

pub(super) fn binding(s: &Session, cmd: &str) -> Result<Arc<ProjectBinding>> {
    s.project().cloned().ok_or_else(|| bad(cmd, "the current file is not in a library project"))
}

pub(super) fn package(cmd: &str, p: &Value) -> Result<String> {
    let name = p.get("package").ok_or_else(|| bad(cmd, "package is required"))?.as_str().ok_or_else(|| bad(cmd, "package must be a string"))?;
    npm::validate_package_name(name).map_err(|e| bad(cmd, e.to_string()))?;
    Ok(name.to_string())
}

fn version(cmd: &str, p: &Value, default: Option<&str>) -> Result<String> {
    let version = match (p.get("version"), default) {
        (Some(value), _) => value.as_str().ok_or_else(|| bad(cmd, "version must be a string"))?,
        (None, Some(default)) => default,
        (None, None) => return Err(bad(cmd, "version is required")),
    };
    npm::validate_version(version, true).map_err(|e| bad(cmd, e.to_string()))?;
    Ok(version.to_string())
}

fn is_active_source(binding: &ProjectBinding, package: &str) -> bool {
    binding.config.sources.iter().any(|source| source.package == package && !source.removed)
}

pub(super) fn require_active_source<'a>(cmd: &str, binding: &'a ProjectBinding, package: &str) -> Result<&'a Source> {
    let source = binding.config.sources.iter().find(|source| source.package == package && !source.removed);
    source.ok_or_else(|| bad(cmd, format!("package \"{package}\" is not a source of this project")))
}

pub(super) fn absolute_root(cmd: &str, root: &str) -> Result<String> {
    let absolute = path::absolute(root).map_err(|e| bad(cmd, format!("cannot resolve the project folder {root}: {e}")))?;
    Ok(absolute.to_string_lossy().into_owned())
}

fn in_flight<'a>(s: &Session, root: &str, packages: &'a [String]) -> Option<&'a String> {
    let busy = s.capture_jobs().into_iter().filter(|job| {
        matches!(job.state, JobState::Queued | JobState::Running) && changes_source(&job.command) && file_access::same_file(&job.root, root)
    });
    let busy: Vec<String> = busy.flat_map(|job| job.packages).collect();
    packages.iter().find(|package| busy.contains(package))
}

fn submit(s: &mut Session, cmd: &str, binding: &ProjectBinding, packages: &[String], job: CaptureJob) -> Result<Value> {
    file_access::allow(Path::new(&binding.root).join(WORK_DIR).join(SOURCES_DIR), Access::Write)?;
    file_access::allow(project::manifest_path(&binding.root), Access::Write)?;
    if let Some(package) = in_flight(s, job.root(), packages) {
        return Err(bad(cmd, project::in_progress(package)));
    }
    let id = s.submit_capture(cmd, &packages.join(", "), job).map_err(|e| bad(cmd, e.message))?;
    Ok(json!({"job": id}))
}

fn install(s: &mut Session, cmd: &str, binding: &ProjectBinding, package: String, version: String) -> Result<Value> {
    let job = CaptureJob::Install { root: absolute_root(cmd, &binding.root)?, package: package.clone(), version };
    submit(s, cmd, binding, &[package], job)
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, ADD)?;
    let package = package(ADD, p)?;
    let version = version(ADD, p, Some(LATEST))?;
    if is_active_source(&binding, &package) {
        return Err(bad(ADD, project::already_source(&package)));
    }
    install(s, ADD, &binding, package, version)
}

fn add_many(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, ADD_MANY)?;
    let line = p.get("line").ok_or_else(|| bad(ADD_MANY, "line is required"))?.as_str().ok_or_else(|| bad(ADD_MANY, "line must be a string"))?;
    let specs = npm::parse_install_line(line).map_err(|e| bad(ADD_MANY, e.to_string()))?;
    if let Some(spec) = specs.iter().find(|spec| is_active_source(&binding, &spec.name)) {
        return Err(bad(ADD_MANY, project::already_source(&spec.name)));
    }
    let names: Vec<String> = specs.iter().map(|spec| spec.name.clone()).collect();
    let job = CaptureJob::InstallMany { root: absolute_root(ADD_MANY, &binding.root)?, packages: specs };
    submit(s, ADD_MANY, &binding, &names, job)
}

fn listed_packages(cmd: &str, p: &Value) -> Result<Vec<String>> {
    let Some(list) = p.get("packages") else { return Ok(vec![package(cmd, p)?]) };
    let list = list.as_array().filter(|list| !list.is_empty()).ok_or_else(|| bad(cmd, "packages must be a non-empty array"))?;
    let mut names: Vec<String> = Vec::with_capacity(list.len());
    for entry in list {
        let name = package(cmd, &json!({"package": entry}))?;
        if names.contains(&name) {
            return Err(bad(cmd, format!("package \"{name}\" is listed twice")));
        }
        names.push(name);
    }
    Ok(names)
}

fn set_version(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, SET_VERSION)?;
    let mut names = listed_packages(SET_VERSION, p)?;
    let version = version(SET_VERSION, p, None)?;
    for name in &names {
        require_active_source(SET_VERSION, &binding, name)?;
    }
    if names.len() == 1 {
        return install(s, SET_VERSION, &binding, names.remove(0), version);
    }
    let specs = names.iter().map(|name| npm::PackageSpec { name: name.clone(), version: version.clone() }).collect();
    let job = CaptureJob::InstallMany { root: absolute_root(SET_VERSION, &binding.root)?, packages: specs };
    submit(s, SET_VERSION, &binding, &names, job)
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = binding(s, REMOVE)?;
    let package = package(REMOVE, p)?;
    require_active_source(REMOVE, &binding, &package)?;
    let job = CaptureJob::Uninstall { root: absolute_root(REMOVE, &binding.root)?, package: package.clone() };
    submit(s, REMOVE, &binding, &[package], job)
}

fn reinstall(s: &mut Session, _p: &Value) -> Result<Value> {
    let binding = binding(s, REINSTALL)?;
    let active = binding.config.sources.iter().filter(|source| !source.removed);
    let specs: Vec<npm::PackageSpec> =
        active.map(|source| npm::PackageSpec { name: source.package.clone(), version: source.version.clone() }).collect();
    if specs.is_empty() {
        return Err(bad(REINSTALL, "this project has no source library to reinstall"));
    }
    for spec in &specs {
        npm::validate_package_name(&spec.name).map_err(|e| bad(REINSTALL, e.to_string()))?;
        npm::validate_version(&spec.version, false).map_err(|e| bad(REINSTALL, format!("{}: {e}", spec.name)))?;
    }
    let names: Vec<String> = specs.iter().map(|spec| spec.name.clone()).collect();
    let job = CaptureJob::InstallMany { root: absolute_root(REINSTALL, &binding.root)?, packages: specs };
    submit(s, REINSTALL, &binding, &names, job)
}

fn status(s: &mut Session, p: &Value) -> Result<Value> {
    let only = match p.get("job") {
        None => None,
        Some(value) => Some(value.as_u64().ok_or_else(|| bad("capture.status", "job must be a whole number"))?),
    };
    if let Some(wanted) = only.filter(|wanted| !s.capture_jobs().iter().any(|job| job.job == *wanted)) {
        return Err(bad("capture.status", format!("job {wanted} is unknown: it never ran, or it ended long ago and is forgotten")));
    }
    let jobs: Vec<Value> = s
        .capture_jobs()
        .into_iter()
        .filter(|job| only.is_none_or(|wanted| wanted == job.job))
        .map(|job| {
            let (state, error) = match job.state {
                JobState::Queued => ("queued", None),
                JobState::Running => ("running", None),
                JobState::Done => ("done", None),
                JobState::Failed(message) => ("failed", Some(message)),
            };
            let mut entry = json!({"job": job.job, "command": job.command, "package": job.package, "root": job.root, "state": state});
            if let (Some(message), Some(fields)) = (error, entry.as_object_mut()) {
                fields.insert("error".to_string(), Value::String(message));
            }
            entry
        })
        .collect();
    Ok(json!({"jobs": jobs}))
}

pub(crate) fn changes_source(command: &str) -> bool {
    matches!(command, ADD | ADD_MANY | SET_VERSION | REMOVE | REINSTALL)
}

/// Write what the finished job `command` on `packages` changed into the `uigen.json` of folder
/// `root`, read again first, and give every tab of the project the new manifest. A command that
/// is not a source command changes nothing; a result the service must not be believed on, or a
/// manifest that can't be written, comes back as the failure of the job.
pub(crate) fn apply(s: &mut Session, command: &str, packages: &[String], root: &str, result: &Value) -> std::result::Result<(), CaptureError> {
    if !changes_source(command) {
        return Ok(());
    }
    let package = packages.join(", ");
    let not_saved = |cause: EngineError| {
        let done = if command == REMOVE { "uninstalled" } else { "installed" };
        CaptureError { kind: CaptureErrorKind::Internal, message: format!("{package} was {done} but uigen.json could not be updated: {cause}") }
    };
    let mut config = project::load(root).map_err(not_saved)?;
    if command == REMOVE {
        match config.sources.iter_mut().find(|source| source.package == package) {
            Some(source) => source.removed = true,
            None => return Ok(()),
        }
    } else if command == REINSTALL {
        restore_all(&mut config, packages, result)?;
    } else if command == ADD_MANY || (command == SET_VERSION && packages.len() > 1) {
        upsert_all(&mut config, packages, result)?;
    } else {
        upsert(&mut config, &package, result)?;
    }
    project::save(root, &config).map_err(not_saved)?;
    s.rebind_project(root, config);
    Ok(())
}

fn refused(message: String) -> CaptureError {
    CaptureError { kind: CaptureErrorKind::Protocol, message }
}

fn optional_text<'a>(result: &'a Value, package: &str, key: &str) -> std::result::Result<Option<&'a str>, CaptureError> {
    match result.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(refused(format!("the install of {package} reported a {key} that is not text"))),
    }
}

fn upsert_all(config: &mut Project, packages: &[String], result: &Value) -> std::result::Result<(), CaptureError> {
    let reported = result.get("packages").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    if reported.len() != packages.len() {
        return Err(refused(format!("the install of {} answered for {} packages", packages.join(", "), reported.len())));
    }
    for (package, entry) in packages.iter().zip(reported) {
        upsert(config, package, entry)?;
    }
    Ok(())
}

fn restore_all(config: &mut Project, packages: &[String], result: &Value) -> std::result::Result<(), CaptureError> {
    let before = config.sources.clone();
    upsert_all(config, packages, result)?;
    for source in &mut config.sources {
        if let Some(old) = before.iter().find(|old| old.package == source.package) {
            source.framework = source.framework.take().or_else(|| old.framework.clone());
            source.license = source.license.take().or_else(|| old.license.clone());
        }
    }
    Ok(())
}

fn upsert(config: &mut Project, package: &str, result: &Value) -> std::result::Result<(), CaptureError> {
    let reported = result.get("package").and_then(Value::as_str);
    if reported != Some(package) {
        return Err(refused(format!("the install of {package} answered for {}", reported.unwrap_or("no package"))));
    }
    let version =
        result.get("version").and_then(Value::as_str).ok_or_else(|| refused(format!("the install of {package} reported no version")))?.to_string();
    let framework = optional_text(result, package, "framework")?;
    if let Some(framework) = framework.filter(|framework| !FRAMEWORKS.contains(framework)) {
        return Err(refused(format!("the install of {package} reported the unknown framework \"{}\"", framework.escape_debug())));
    }
    let license = optional_text(result, package, "license")?;
    if license.is_some_and(|license| license.len() > MAX_LICENSE_BYTES || license.chars().any(char::is_control)) {
        return Err(refused(format!("the install of {package} reported a license longer than {MAX_LICENSE_BYTES} bytes or with control characters")));
    }
    let (framework, license) = (framework.map(str::to_string), license.map(str::to_string));
    match config.sources.iter_mut().find(|source| source.package == package) {
        Some(source) => {
            source.version = version;
            source.framework = framework;
            source.license = license;
            source.removed = false;
        }
        None => config.sources.push(Source { package: package.to_string(), version, framework, license, removed: false, extra: Default::default() }),
    }
    Ok(())
}
