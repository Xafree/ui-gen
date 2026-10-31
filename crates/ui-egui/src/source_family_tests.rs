//! The family rule of the source libraries: one design system often ships as several npm packages
//! (`@heroui/react` with its components, `@heroui/styles` with its stylesheet), and the studio shows
//! them as one library. The rule is a pure function from what is known of each active source to
//! the families to show, in the order of the first source of each family.
//!
//! A source whose framework is unknown (`""` or `"unknown"`) or that exports no component is a
//! satellite: it joins the source that depends on it, or the source of its npm scope that has the
//! same version. A source that has a framework and components is a head and never joins another.
//! A family lists its head first, then its satellites in the order they were given, and its label
//! is the scope without the at sign plus the version of its head (`heroui 3.2.7`). A source with no
//! scope, or with no head to join, is a family of its own, labelled with its full package name.

use crate::source_family::{Family, SourceFacts, group_sources};

fn facts<'a>(package: &'a str, version: &'a str, framework: &'a str, components: usize) -> SourceFacts<'a> {
    SourceFacts { package, version, framework, components, depends_on: &[] }
}

fn shape(families: &[Family]) -> Vec<(String, Vec<String>)> {
    families.iter().map(|family| (family.label.clone(), family.packages.clone())).collect()
}

fn expected(label: &str, packages: &[&str]) -> (String, Vec<String>) {
    (label.to_string(), packages.iter().map(ToString::to_string).collect())
}

// covers: GROUP-1
#[test]
fn a_stylesheet_package_of_the_same_scope_and_version_joins_the_package_that_has_the_components() {
    let sources = [facts("@heroui/styles", "3.2.7", "", 0), facts("@heroui/react", "3.2.7", "react", 14)];
    assert_eq!(shape(&group_sources(&sources)), [expected("heroui 3.2.7", &["@heroui/react", "@heroui/styles"])]);
}

// covers: GROUP-1
#[test]
fn a_source_with_an_unknown_framework_or_with_no_component_is_a_satellite_whichever_the_reason() {
    let sources = [
        facts("@acme/react", "1.0.0", "react", 9),
        facts("@acme/icons", "1.0.0", "react", 0),
        facts("@acme/theme", "1.0.0", "unknown", 3),
        facts("@acme/tokens", "1.0.0", "", 2),
    ];
    let families = group_sources(&sources);
    assert_eq!(shape(&families), [expected("acme 1.0.0", &["@acme/react", "@acme/icons", "@acme/theme", "@acme/tokens"])]);
}

// covers: GROUP-1
#[test]
fn a_source_the_head_depends_on_joins_it_even_with_another_version() {
    let sources = [
        SourceFacts { depends_on: &["@acme/tokens"], ..facts("@acme/ui", "2.0.0", "react", 5) },
        facts("@acme/tokens", "1.4.0", "", 0),
        facts("@other/lib", "1.0.0", "react", 3),
    ];
    let families = shape(&group_sources(&sources));
    assert_eq!(families.len(), 2, "{families:?}");
    assert_eq!(families[0].1, ["@acme/ui", "@acme/tokens"]);
    assert_eq!(families[1], expected("@other/lib", &["@other/lib"]));
}

// covers: GROUP-1
#[test]
fn packages_without_a_scope_or_without_a_sibling_stay_alone_under_their_full_name() {
    let sources = [
        facts("plain-ui", "1.0.0", "react", 4),
        facts("plain-theme", "1.0.0", "", 0),
        facts("@solo/styles", "1.0.0", "", 0),
        facts("@heroui/react", "3.2.7", "react", 14),
    ];
    assert_eq!(
        shape(&group_sources(&sources)),
        [
            expected("plain-ui", &["plain-ui"]),
            expected("plain-theme", &["plain-theme"]),
            expected("@solo/styles", &["@solo/styles"]),
            expected("@heroui/react", &["@heroui/react"]),
        ]
    );
    assert!(group_sources(&[]).is_empty(), "no source, no family");
}

// covers: GROUP-1
#[test]
fn unrelated_packages_of_one_scope_with_different_versions_or_both_with_components_stay_apart() {
    let versions = [facts("@acme/ui", "2.0.0", "react", 5), facts("@acme/tokens", "1.4.0", "", 0)];
    assert_eq!(shape(&group_sources(&versions)), [expected("@acme/ui", &["@acme/ui"]), expected("@acme/tokens", &["@acme/tokens"])]);

    let heads = [facts("@mui/material", "7.3.11", "react", 40), facts("@mui/icons-material", "7.3.11", "react", 2100)];
    assert_eq!(
        shape(&group_sources(&heads)),
        [expected("@mui/material", &["@mui/material"]), expected("@mui/icons-material", &["@mui/icons-material"])]
    );
}
