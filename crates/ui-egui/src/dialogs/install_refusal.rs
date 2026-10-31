use figcraft_project::ProjectError;
use figcraft_project::npm::{LineRefusal, TokenProblem, line_refusal};

use crate::i18n::{tr, trf};

/// The reason `parse_install_line` refused a line, in the language of the user.
pub(crate) fn text(error: &ProjectError) -> Option<String> {
    Some(match line_refusal(error)? {
        LineRefusal::Empty => tr("type at least one package, for example: @scope/name another@1.2.0").to_string(),
        LineRefusal::TooMany { count, max } => trf("{count} packages on one line: {max} at most", &[("count", &count), ("max", &max)]),
        LineRefusal::NotInstall(word) => {
            trf("{word}: only \"npm install\" or \"npm i\" is understood here; type the package names", &[("word", &word)])
        }
        LineRefusal::NoPackagesAfterNpm => tr("npm: type the package names after \"npm install\"").to_string(),
        LineRefusal::OtherManager(word) => trf("{word}: only npm is understood here; type the package names", &[("word", &word)]),
        LineRefusal::Token { token, problem } => token_text(&token, problem),
    })
}

fn token_text(token: &str, problem: TokenProblem) -> String {
    let token = &token;
    match problem {
        TokenProblem::NameEmpty => trf("{token}: package name is empty", &[("token", token)]),
        TokenProblem::NameTooLong(bytes) => trf("{token}: package name is longer than {bytes} bytes", &[("token", token), ("bytes", &bytes)]),
        TokenProblem::NameStartsWithDash => trf("{token}: package name must not start with \"-\"", &[("token", token)]),
        TokenProblem::NameIsArchive => trf("{token}: package name must be a registry name, not an archive", &[("token", token)]),
        TokenProblem::NameNotRegistry => {
            trf("{token}: package name must be a lowercase registry name: \"name\" or \"@scope/name\"", &[("token", token)])
        }
        TokenProblem::VersionTooLong(bytes) => trf("{token}: version is longer than {bytes} bytes", &[("token", token), ("bytes", &bytes)]),
        TokenProblem::LatestNotAllowed => trf("{token}: version \"latest\" is not allowed here, give an exact version", &[("token", token)]),
        TokenProblem::VersionNotExact => trf("{token}: version must be an exact semver MAJOR.MINOR.PATCH, no range or tag", &[("token", token)]),
    }
}
