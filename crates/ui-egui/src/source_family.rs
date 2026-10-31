use std::collections::HashMap;
use std::sync::Mutex;

pub struct SourceFacts<'a> {
    pub package: &'a str,
    pub version: &'a str,
    pub framework: &'a str,
    pub components: usize,
    pub depends_on: &'a [&'a str],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Family {
    pub label: String,
    pub name: String,
    pub version: String,
    pub packages: Vec<String>,
}

fn is_head(source: &SourceFacts) -> bool {
    !matches!(source.framework, "" | "unknown") && source.components > 0
}

fn scope(package: &str) -> Option<&str> {
    package.strip_prefix('@').and_then(|rest| rest.split_once('/')).map(|(scope, _)| scope)
}

fn head_of(heads: &[usize], sources: &[SourceFacts], satellite: &SourceFacts) -> Option<usize> {
    let depended_on = heads.iter().copied().find(|&head| sources[head].depends_on.contains(&satellite.package));
    depended_on.or_else(|| {
        let wanted = scope(satellite.package)?;
        heads.iter().copied().find(|&head| scope(sources[head].package) == Some(wanted) && sources[head].version == satellite.version)
    })
}

pub fn group_sources(sources: &[SourceFacts]) -> Vec<Family> {
    let heads: Vec<usize> = (0..sources.len()).filter(|&index| is_head(&sources[index])).collect();
    let mut members: Vec<(usize, Vec<usize>)> = heads.iter().map(|&head| (head, vec![head])).collect();
    for (index, source) in sources.iter().enumerate().filter(|(index, _)| !heads.contains(index)) {
        match head_of(&heads, sources, source).and_then(|head| members.iter_mut().find(|(lead, _)| *lead == head)) {
            Some((_, list)) => list.push(index),
            None => members.push((index, vec![index])),
        }
    }
    let mut families: Vec<(usize, Family)> = members
        .into_iter()
        .map(|(lead, list)| {
            let first = list.iter().copied().min().unwrap_or(lead);
            let head = &sources[lead];
            let multiple = list.len() > 1;
            let name = match scope(head.package) {
                Some(scope) if multiple => scope.to_string(),
                _ => head.package.to_string(),
            };
            let label = if multiple { format!("{name} {}", head.version) } else { name.clone() };
            let family = Family {
                label,
                name,
                version: head.version.to_string(),
                packages: list.iter().map(|&index| sources[index].package.to_string()).collect(),
            };
            (first, family)
        })
        .collect();
    families.sort_by_key(|(first, _)| *first);
    families.into_iter().map(|(_, family)| family).collect()
}

type DependencyKey = (String, String, String);

static DEPENDENCIES: Mutex<Option<HashMap<DependencyKey, Vec<String>>>> = Mutex::new(None);

/// The packages `package` (at `version`, installed in project folder `root`) depends on, read once
/// the package is installed.
pub fn dependencies_of(root: &str, package: &str, version: &str) -> Vec<String> {
    let key = (root.to_string(), package.to_string(), version.to_string());
    if let Ok(cache) = DEPENDENCIES.lock()
        && let Some(found) = cache.as_ref().and_then(|map| map.get(&key))
    {
        return found.clone();
    }
    let read = figcraft_engine::project::installed_dependencies(root, package);
    if !read.is_empty()
        && let Ok(mut cache) = DEPENDENCIES.lock()
    {
        cache.get_or_insert_with(HashMap::new).insert(key, read.clone());
    }
    read
}
