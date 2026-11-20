//! Bumping the version of SDKMAN! candidates pinned in a `.sdkmanrc` file,
//! either one candidate at a time or every outdated one.
//!
//! Java is the only supported candidate, and only from Eclipse Temurin, whose
//! identifiers end with `-tem`, since its release notes can be linked from a
//! bump's pull request. Java from any other vendor is not supported.

use crate::git;
use crate::tool::{self, Edit, Outdated, PullRequest};
use anyhow::{Context, Result, bail};
use std::cmp::Ordering;
use std::fmt;

/// A SDKMAN! candidate pinned in `.sdkmanrc` by a Temurin identifier, such as
/// `25.0.5.1+2-tem` for Java.
pub struct Candidate {
    /// The key of the candidate in `.sdkmanrc` and in SDKMAN!, also used as its
    /// short name and in branch names.
    key: &'static str,
    /// The name of the candidate, used in commit messages and pull requests.
    name: &'static str,
}

/// The supported SDKMAN! candidates.
pub static CANDIDATES: [Candidate; 1] = [Candidate {
    key: "java",
    name: "Java",
}];

/// The tags that a SDKMAN! candidate can be bumped to instead of a version.
pub const TAGS: [&str; 1] = ["latest"];

/// The platform whose identifiers are looked up in SDKMAN!, which is the one
/// CI usually runs on, since SDKMAN! serves slightly different identifiers for
/// each platform.
const PLATFORM: &str = "linuxx64";

impl Candidate {
    /// Returns the key of the candidate in `.sdkmanrc`, such as `java`.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        self.key
    }
}

/// A bump of a SDKMAN! candidate, prepared up to the edit of `.sdkmanrc` that
/// it makes.
pub struct Bump<'a> {
    /// The candidate that is bumped.
    candidate: &'a Candidate,
    /// The identifier that the candidate is bumped to.
    #[cfg_attr(tarpaulin_include, allow(dead_code))] // Read only by `apply` and `refresh`.
    identifier: String,
    /// The edit of `.sdkmanrc`.
    edit: Edit,
    /// The default branch that the bump's pull request targets.
    #[cfg_attr(tarpaulin_include, allow(dead_code))] // Read only by `apply`.
    base: String,
}

/// Prepares a bump of a SDKMAN! candidate in `.sdkmanrc` to the given target,
/// which is either a version or one of [`TAGS`], without changing anything
/// yet.
///
/// The target is resolved to the newest Temurin identifier whose version is
/// the given version or starts with it, such as `27` for `27.0.1.1+4-tem`, or
/// to the newest one of any version for `latest`.
///
/// If no target is given, the candidate is bumped to the newest identifier of
/// the major version it's already on in `.sdkmanrc`, since a JDK is usually
/// kept on an LTS release on purpose, so moving to another major version is
/// left to be asked for.
///
/// If the repository is not on its default branch or its working tree is not
/// clean, it asks whether to switch to the default branch or discard the
/// changes first, unless `yes` is set.
///
/// Returns `None`, showing that the candidate is up to date, if it's already
/// at the identifier the target resolves to.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bump, if the
/// candidate is not pinned by a Temurin identifier, if the target is not a
/// version or a known tag, if no identifier matches it, or if the candidate
/// can't be bumped to it, such as when it's already at a newer version.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn prepare<'a>(
    candidate: &'a Candidate,
    target: Option<&str>,
    yes: bool,
) -> Result<Option<Bump<'a>>> {
    println!("checking that the repository is ready");
    let base = tool::prepare_repository(yes)?;
    tool::check_paths(&[".sdkmanrc"])?;
    let sdkmanrc = tool::read(".sdkmanrc")?;
    let current = current_identifier(&sdkmanrc, candidate)?;
    let target = target.map_or_else(|| current.major().to_string(), str::to_string);
    let identifier = resolve_identifier(candidate, &target)?;
    if identifier == current.raw {
        println!("{} is up to date at version {current}", candidate.name);
        return Ok(None);
    }
    Bump::new(candidate, &sdkmanrc, identifier, base).map(Some)
}

/// Prepares a bump of every supported candidate in `.sdkmanrc` that is behind
/// the newest Temurin identifier of its major version, without changing
/// anything yet.
///
/// It warns about each candidate that isn't supported, including one pinned
/// by an identifier from another vendor, and shows each one that is already up
/// to date.
///
/// If the repository is not on its default branch or its working tree is not
/// clean, it asks whether to switch to the default branch or discard the
/// changes first, unless `yes` is set.
///
/// Returns the bumps in the order their candidates appear in `.sdkmanrc`,
/// which is empty if every supported candidate is up to date, along with
/// whether any candidate was skipped for not being supported.
///
/// # Errors
///
/// Returns an error if the repository is not ready for the bumps, or if the
/// identifier of a candidate can't be read or resolved.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub fn prepare_outdated(yes: bool) -> Result<Outdated<Bump<'static>>> {
    println!("checking that the repository is ready");
    let base = tool::prepare_repository(yes)?;
    tool::check_paths(&[".sdkmanrc"])?;
    find_outdated(&tool::read(".sdkmanrc")?, &base)
}

/// Finds every supported candidate in the given `.sdkmanrc` file that is
/// behind the newest Temurin identifier of its major version, warning about
/// each candidate that isn't supported, including one pinned by an identifier
/// from another vendor, and showing each one that is already up to date.
///
/// Returns a bump for each of them, with a pull request against the given base
/// branch, in the order their candidates appear in the file, along with
/// whether any candidate was skipped for not being supported.
///
/// # Errors
///
/// Returns an error if the identifier of a candidate can't be read or
/// resolved.
#[cfg(not(tarpaulin_include))] // Sends requests, so it is not unit tested.
pub(crate) fn find_outdated(sdkmanrc: &str, base: &str) -> Result<Outdated<Bump<'static>>> {
    let candidates = find_candidates(sdkmanrc);
    for (key, value) in &candidates.unsupported {
        println!("warning: {key}={value} in .sdkmanrc is not supported, skipping");
    }

    let mut outdated = Outdated {
        bumps: Vec::new(),
        skipped: !candidates.unsupported.is_empty(),
    };
    for candidate in candidates.supported {
        let current = current_identifier(sdkmanrc, candidate)?;
        let identifier = resolve_identifier(candidate, current.major())?;
        let newer = Identifier::parse(&identifier)
            .is_some_and(|identifier| identifier.compare(&current).is_gt());
        if !newer {
            println!("{} is up to date at version {current}", candidate.name);
            continue;
        }
        outdated.bumps.push(Bump::new(
            candidate,
            sdkmanrc,
            identifier,
            base.to_string(),
        )?);
    }
    Ok(outdated)
}

/// Resolves the target of a bump, which is either a version or one of
/// [`TAGS`], to the newest Temurin identifier of a candidate in SDKMAN!,
/// showing what it looks for and finds.
#[cfg(not(tarpaulin_include))] // Sends a request, so it is not unit tested.
fn resolve_identifier(candidate: &Candidate, target: &str) -> Result<String> {
    if !tool::is_version(target) {
        tool::check_tag(candidate.name, target, &TAGS)?;
    }
    if target == "latest" {
        println!("finding the latest version of {}", candidate.name);
    } else {
        println!(
            "finding the newest {target}.x version of {}",
            candidate.name
        );
    }
    let identifier = newest_identifier(candidate, &fetch_identifiers(candidate)?, target)?;
    println!("found {} {identifier}", candidate.name);
    Ok(identifier)
}

impl Bump<'_> {
    /// Prepares a bump of a candidate to the given identifier, editing the
    /// given `.sdkmanrc` file, with a pull request against the given base
    /// branch.
    pub(crate) fn new<'c>(
        candidate: &'c Candidate,
        sdkmanrc: &str,
        identifier: String,
        base: String,
    ) -> Result<Bump<'c>> {
        let edit = bump_sdkmanrc(sdkmanrc, candidate, &identifier)?;
        Ok(Bump {
            candidate,
            identifier,
            edit,
            base,
        })
    }

    /// Prepares the bump again from the latest default branch, editing
    /// `.sdkmanrc` as it is there, so a bump made after another one starts
    /// from the default branch rather than the branch of the one before.
    ///
    /// If the repository is not on its default branch or its working tree is
    /// not clean, it asks whether to switch to the default branch or discard
    /// the changes first, unless `yes` is set.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository is not ready for the bump, or if the
    /// candidate can no longer be bumped to the identifier.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn refresh(self, yes: bool) -> Result<Self> {
        println!("checking that the repository is ready");
        let base = tool::prepare_repository(yes)?;
        tool::check_paths(&[".sdkmanrc"])?;
        Self::new(
            self.candidate,
            &tool::read(".sdkmanrc")?,
            self.identifier,
            base,
        )
    }

    /// Returns the change that the bump makes, naming the candidate and file
    /// it edits along with the identifiers it changes from and to.
    #[must_use]
    pub fn change(&self) -> String {
        format!(
            "{} in .sdkmanrc: {} -> {}",
            self.candidate.key, self.edit.from, self.edit.to
        )
    }

    /// Applies the bump, then commits the change to a new branch with the given
    /// options, pushes it, and opens a pull request for it linking to the
    /// release notes of the identifier, or updates the one already open.
    ///
    /// Returns the pull request, which is the one already open for the bump if
    /// there is one.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of it fails.
    #[cfg(not(tarpaulin_include))] // Runs commands, so it is not unit tested.
    pub fn apply(self, options: tool::CommitOptions) -> Result<PullRequest> {
        let candidate = self.candidate;
        let branch = tool::branch_name(candidate.key, &self.identifier);
        println!("creating branch {branch}");
        git::create_branch(&branch)?;
        println!("updating {}", self.change());
        tool::write(".sdkmanrc", &self.edit.content)?;
        println!("committing .sdkmanrc");
        git::commit(
            &tool::commit_message(candidate.name, &self.identifier),
            &[".sdkmanrc"],
            options,
        )?;
        println!("pushing {branch} to origin");
        git::push(&branch)?;

        let release_url = Identifier::parse(&self.identifier)
            .context("the bumped identifier is not a Temurin identifier")?
            .release_url();
        tool::open_or_update_pull_request(
            &branch,
            &self.base,
            candidate.name,
            &self.identifier,
            &release_url,
        )
    }
}

/// A Temurin identifier of a SDKMAN! candidate, such as `25.0.5.1+2-tem`,
/// split into its version and its build.
#[derive(Debug, PartialEq, Eq)]
struct Identifier<'a> {
    /// The whole identifier.
    raw: &'a str,
    /// The plain version, such as `25.0.5.1`.
    version: &'a str,
    /// The plain build number after `+`, such as `2`.
    build: &'a str,
}

impl<'a> Identifier<'a> {
    /// Parses a Temurin identifier, or returns `None` if it's from another
    /// vendor, such as `25.0.5.1+2-zulu`, or if its version or build is not
    /// plain.
    fn parse(raw: &'a str) -> Option<Self> {
        let (version, build) = raw.strip_suffix("-tem")?.split_once('+')?;
        let plain = tool::is_version(version) && tool::is_version(build);
        plain.then_some(Self {
            raw,
            version,
            build,
        })
    }

    /// Returns the major version of the identifier, such as `25` for
    /// `25.0.5.1+2-tem`.
    fn major(&self) -> &'a str {
        self.version.split('.').next().unwrap_or_default()
    }

    /// Returns whether the version of the identifier is the given plain
    /// version or starts with it on whole parts, so `25` matches `25.0.5.1`
    /// but not `250.0.0`.
    fn matches(&self, version: &str) -> bool {
        self.version == version
            || self
                .version
                .strip_prefix(version)
                .is_some_and(|rest| rest.starts_with('.'))
    }

    /// Compares the identifier with another one by their versions, then by
    /// their builds.
    fn compare(&self, other: &Self) -> Ordering {
        tool::compare_versions(self.version, other.version)
            .then_with(|| tool::compare_versions(self.build, other.build))
    }

    /// Returns the name of the Temurin release of the identifier, such as
    /// `jdk-25.0.5.1+2` for `25.0.5.1+2-tem`, `jdk-25.0.5+8` for
    /// `25.0.5.0+8-tem`, since Java drops trailing zero parts of its versions,
    /// or `jdk8u512-b09` for `8.0.512.0+9-tem`, since Java 8 is named
    /// differently.
    fn release_name(&self) -> String {
        let mut parts: Vec<&str> = self.version.split('.').collect();
        if parts[0] == "8" {
            let update = parts.get(2).copied().unwrap_or("0");
            return format!("jdk8u{update}-b{:0>2}", self.build);
        }
        while parts.len() > 1
            && parts
                .last()
                .is_some_and(|part| part.trim_start_matches('0').is_empty())
        {
            parts.pop();
        }
        format!("jdk-{}+{}", parts.join("."), self.build)
    }

    /// Returns the URL of the release notes of the Temurin release of the
    /// identifier on the Adoptium website.
    fn release_url(&self) -> String {
        let name = self.release_name().replace('+', "%2B");
        format!("https://adoptium.net/temurin/release-notes/?version={name}")
    }
}

impl fmt::Display for Identifier<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.raw)
    }
}

/// The candidates pinned in a `.sdkmanrc` file, split by whether they are
/// supported.
struct FoundCandidates<'a> {
    /// The supported candidates, in the order they appear.
    supported: Vec<&'static Candidate>,
    /// The keys and values of the candidates that aren't supported, in the
    /// order they appear.
    unsupported: Vec<(&'a str, &'a str)>,
}

/// Finds the candidates pinned in a `.sdkmanrc` file, where a supported
/// candidate pinned by an identifier that is not from Temurin counts as not
/// supported.
fn find_candidates(sdkmanrc: &str) -> FoundCandidates<'_> {
    let mut found = FoundCandidates {
        supported: Vec::new(),
        unsupported: Vec::new(),
    };
    for (key, value) in sdkmanrc.lines().filter_map(entry) {
        let candidate = CANDIDATES.iter().find(|candidate| candidate.key == key);
        match candidate.filter(|_| Identifier::parse(value).is_some()) {
            Some(candidate) => found.supported.push(candidate),
            None => found.unsupported.push((key, value)),
        }
    }
    found
}

/// Returns the key and value of a line of a `.sdkmanrc` file if the line is a
/// `key=value` entry, rather than a comment or blank.
fn entry(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.starts_with('#') {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

/// Returns the identifier of a candidate pinned in a `.sdkmanrc` file.
fn current_identifier<'a>(sdkmanrc: &'a str, candidate: &Candidate) -> Result<Identifier<'a>> {
    let key = candidate.key;
    let values: Vec<&str> = sdkmanrc
        .lines()
        .filter_map(entry)
        .filter(|(entry_key, _)| *entry_key == key)
        .map(|(_, value)| value)
        .collect();
    let count = values.len();
    let [value] = values[..] else {
        bail!("expected exactly one {key} entry in .sdkmanrc, found {count}");
    };
    match Identifier::parse(value) {
        Some(identifier) => Ok(identifier),
        None => {
            bail!("{key} in .sdkmanrc is not a Temurin identifier like 25.0.5.1+2-tem: {value}")
        }
    }
}

/// Returns the edit of a `.sdkmanrc` file with the given candidate bumped to
/// the given identifier, editing only the identifier text to keep the rest of
/// the file as is.
fn bump_sdkmanrc(sdkmanrc: &str, candidate: &Candidate, identifier: &str) -> Result<Edit> {
    let current = current_identifier(sdkmanrc, candidate)?;
    let name = candidate.name;
    let Some(new) = Identifier::parse(identifier) else {
        bail!("{identifier} is not a Temurin identifier of {name} like 25.0.5.1+2-tem");
    };
    match new.compare(&current) {
        Ordering::Greater => {}
        Ordering::Equal => bail!("{name} is already at version {current}"),
        Ordering::Less => bail!("{name} is already at version {current}, newer than {new}"),
    }

    let content = sdkmanrc
        .split_inclusive('\n')
        .map(|line| match entry(line) {
            Some((key, _)) if key == candidate.key => line.replacen(current.raw, new.raw, 1),
            _ => line.to_string(),
        })
        .collect();
    Ok(Edit {
        content,
        from: current.raw.to_string(),
        to: new.raw.to_string(),
    })
}

/// Fetches the identifiers of a candidate that SDKMAN! serves for
/// [`PLATFORM`], as a comma-separated list.
#[cfg(not(tarpaulin_include))] // Sends a request, so it is not unit tested.
fn fetch_identifiers(candidate: &Candidate) -> Result<String> {
    let url = format!(
        "https://api.sdkman.io/2/candidates/{}/{PLATFORM}/versions/all",
        candidate.key
    );
    tool::http_agent()
        .get(&url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .with_context(|| format!("failed to request {url}"))
}

/// Returns the newest Temurin identifier in a comma-separated list of
/// identifiers of a candidate that matches the given target, which is either
/// a plain version, matched as [`Identifier::matches`] does, or `latest`,
/// matching every version.
///
/// SDKMAN! keeps only the newest identifiers, so the current one is usually
/// not in the list, and is only compared against.
fn newest_identifier(candidate: &Candidate, identifiers: &str, target: &str) -> Result<String> {
    let name = candidate.name;
    identifiers
        .split(',')
        .filter_map(|identifier| Identifier::parse(identifier.trim()))
        .filter(|identifier| target == "latest" || identifier.matches(target))
        .max_by(Identifier::compare)
        .map(|identifier| identifier.raw.to_string())
        .with_context(|| {
            let release = if target == "latest" {
                String::new()
            } else {
                format!("{target}.x ")
            };
            format!("no {release}Temurin release of {name} is found in SDKMAN!")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SDKMANRC: &str = "# Enable auto-env through the sdkman_auto_env config\n# Add key=value pairs of SDKs to use below\njava=25.0.5.0+8-tem\n";

    fn java() -> &'static Candidate {
        &CANDIDATES[0]
    }

    fn identifier(raw: &str) -> Identifier<'_> {
        Identifier::parse(raw).unwrap()
    }

    #[test]
    fn candidate_key() {
        assert_eq!(java().key(), "java");
    }

    #[test]
    fn bump_sdkmanrc_identifier() {
        let edit = bump_sdkmanrc(SDKMANRC, java(), "25.0.5.1+2-tem").unwrap();
        assert_eq!(
            edit.content,
            SDKMANRC.replace("25.0.5.0+8-tem", "25.0.5.1+2-tem")
        );
        assert_eq!(edit.from, "25.0.5.0+8-tem");
        assert_eq!(edit.to, "25.0.5.1+2-tem");
    }

    #[test]
    fn bump_sdkmanrc_keeps_other_entries() {
        let sdkmanrc = "gradle=9.2.0\n java = 25.0.5.0+8-tem \n#java=25.0.5.0+8-tem\nmaven=3.9.11";
        assert_eq!(
            bump_sdkmanrc(sdkmanrc, java(), "25.0.5.1+2-tem")
                .unwrap()
                .content,
            "gradle=9.2.0\n java = 25.0.5.1+2-tem \n#java=25.0.5.0+8-tem\nmaven=3.9.11"
        );
    }

    #[test]
    fn bump_sdkmanrc_invalid() {
        let cases = [
            (
                SDKMANRC,
                "25.0.5.1+2-zulu",
                "25.0.5.1+2-zulu is not a Temurin identifier of Java like 25.0.5.1+2-tem",
            ),
            (
                SDKMANRC,
                "25.0.5.0+8-tem",
                "Java is already at version 25.0.5.0+8-tem",
            ),
            (
                SDKMANRC,
                "25.0.5.0+7-tem",
                "Java is already at version 25.0.5.0+8-tem, newer than 25.0.5.0+7-tem",
            ),
            (
                SDKMANRC,
                "21.0.13.0+9-tem",
                "Java is already at version 25.0.5.0+8-tem, newer than 21.0.13.0+9-tem",
            ),
            (
                "java=25.0.5.0+8-zulu\n",
                "25.0.5.1+2-tem",
                "java in .sdkmanrc is not a Temurin identifier like 25.0.5.1+2-tem: 25.0.5.0+8-zulu",
            ),
        ];
        for (sdkmanrc, identifier, error) in cases {
            assert_eq!(
                bump_sdkmanrc(sdkmanrc, java(), identifier)
                    .unwrap_err()
                    .to_string(),
                error
            );
        }
    }

    #[test]
    fn bump_change() {
        let bump = Bump::new(
            java(),
            SDKMANRC,
            "25.0.5.1+2-tem".to_string(),
            "main".to_string(),
        )
        .unwrap();
        assert_eq!(
            bump.change(),
            "java in .sdkmanrc: 25.0.5.0+8-tem -> 25.0.5.1+2-tem"
        );
    }

    #[test]
    fn identifier_parse_valid() {
        let cases = [
            ("25.0.5.1+2-tem", "25.0.5.1", "2"),
            ("8.0.512.0+9-tem", "8.0.512.0", "9"),
        ];
        for (raw, version, build) in cases {
            assert_eq!(
                Identifier::parse(raw),
                Some(Identifier {
                    raw,
                    version,
                    build
                }),
                "{raw}"
            );
        }
    }

    #[test]
    fn identifier_parse_invalid() {
        for raw in [
            "25.0.5.1+2-zulu",
            "21.0.13.0-crac+3.1-zulu",
            "25.0.5.1-graal",
            "25.0.5.1-tem",
            "25.0.5.1+b2-tem",
            "25.0.2.r25+1-tem",
            "25.0.5.1",
            "",
        ] {
            assert_eq!(Identifier::parse(raw), None, "{raw}");
        }
    }

    #[test]
    fn identifier_major() {
        assert_eq!(identifier("25.0.5.1+2-tem").major(), "25");
        assert_eq!(identifier("8.0.512.0+9-tem").major(), "8");
    }

    #[test]
    fn identifier_matches() {
        let current = identifier("25.0.5.1+2-tem");
        for version in ["25", "25.0", "25.0.5", "25.0.5.1"] {
            assert!(current.matches(version), "{version}");
        }
        for version in ["2", "25.0.5.10", "26", "25.1"] {
            assert!(!current.matches(version), "{version}");
        }
    }

    #[test]
    fn identifier_compare() {
        let cases = [
            ("25.0.5.1+2-tem", "25.0.5.0+8-tem", Ordering::Greater),
            ("25.0.5.0+10-tem", "25.0.5.0+8-tem", Ordering::Greater),
            ("21.0.13.0+9-tem", "25.0.5.0+8-tem", Ordering::Less),
            ("25.0.5.0+8-tem", "25.0.5.0+8-tem", Ordering::Equal),
        ];
        for (a, b, ordering) in cases {
            assert_eq!(
                identifier(a).compare(&identifier(b)),
                ordering,
                "{a} vs {b}"
            );
        }
    }

    #[test]
    fn identifier_release_name() {
        let cases = [
            ("25.0.5.1+2-tem", "jdk-25.0.5.1+2"),
            ("25.0.5.0+8-tem", "jdk-25.0.5+8"),
            ("11.0.33.0+10-tem", "jdk-11.0.33+10"),
            ("25.0.0.0+36-tem", "jdk-25+36"),
            ("27.0.1.0.2+1-tem", "jdk-27.0.1.0.2+1"),
            ("8.0.512.0+9-tem", "jdk8u512-b09"),
            ("8.0.504.1+11-tem", "jdk8u504-b11"),
            ("8+1-tem", "jdk8u0-b01"),
        ];
        for (raw, name) in cases {
            assert_eq!(identifier(raw).release_name(), name, "{raw}");
        }
    }

    #[test]
    fn identifier_release_url() {
        assert_eq!(
            identifier("25.0.5.1+2-tem").release_url(),
            "https://adoptium.net/temurin/release-notes/?version=jdk-25.0.5.1%2B2"
        );
    }

    #[test]
    fn identifier_display() {
        assert_eq!(identifier("25.0.5.1+2-tem").to_string(), "25.0.5.1+2-tem");
    }

    #[test]
    fn find_supported_and_unsupported_candidates() {
        let sdkmanrc =
            "# java=25.0.5.0+8-tem\ngradle=9.2.0\n\njava=25.0.5.0+8-tem\nkotlin=2.2.21\n";
        let found = find_candidates(sdkmanrc);
        let keys: Vec<_> = found
            .supported
            .iter()
            .map(|candidate| candidate.key)
            .collect();
        assert_eq!(keys, ["java"]);
        assert_eq!(
            found.unsupported,
            [("gradle", "9.2.0"), ("kotlin", "2.2.21")]
        );
    }

    #[test]
    fn find_other_vendor_unsupported() {
        let found = find_candidates("java=25.0.5.1+2-zulu\n");
        assert!(found.supported.is_empty());
        assert_eq!(found.unsupported, [("java", "25.0.5.1+2-zulu")]);
    }

    #[test]
    fn find_no_candidates() {
        let found = find_candidates("# nothing pinned\n\n");
        assert!(found.supported.is_empty());
        assert!(found.unsupported.is_empty());
    }

    #[test]
    fn current_identifier_valid() {
        assert_eq!(
            current_identifier(SDKMANRC, java()).unwrap(),
            identifier("25.0.5.0+8-tem")
        );
    }

    #[test]
    fn current_identifier_invalid() {
        let cases = [
            (
                "gradle=9.2.0\n",
                "expected exactly one java entry in .sdkmanrc, found 0",
            ),
            (
                "java=25.0.5.0+8-tem\njava=21.0.13.0+9-tem\n",
                "expected exactly one java entry in .sdkmanrc, found 2",
            ),
            (
                "java=25\n",
                "java in .sdkmanrc is not a Temurin identifier like 25.0.5.1+2-tem: 25",
            ),
        ];
        for (sdkmanrc, error) in cases {
            assert_eq!(
                current_identifier(sdkmanrc, java())
                    .unwrap_err()
                    .to_string(),
                error
            );
        }
    }

    const IDENTIFIERS: &str = "11.0.33.0+10-tem,21.0.13.0+9-tem,21.0.13.0-crac+3.1-zulu,25.0.2.r25-mandrel,25.0.5.1+2-tem,25.0.6.0+1-zulu,26.0.2.1+1-tem, 27.0.1.1+4-tem,8.0.512.0+9-tem,250.0.0+1-tem";

    #[test]
    fn newest_identifier_valid() {
        let cases = [
            ("25", "25.0.5.1+2-tem"),
            ("25.0.5.1", "25.0.5.1+2-tem"),
            ("27", "27.0.1.1+4-tem"),
            ("8", "8.0.512.0+9-tem"),
            ("latest", "250.0.0+1-tem"),
        ];
        for (target, newest) in cases {
            assert_eq!(
                newest_identifier(java(), IDENTIFIERS, target).unwrap(),
                newest,
                "{target}"
            );
        }
    }

    #[test]
    fn newest_identifier_invalid() {
        let cases = [
            (
                IDENTIFIERS,
                "28",
                "no 28.x Temurin release of Java is found in SDKMAN!",
            ),
            (
                "25.0.5.1+2-zulu,25.0.5.1-graal",
                "latest",
                "no Temurin release of Java is found in SDKMAN!",
            ),
        ];
        for (identifiers, target, error) in cases {
            assert_eq!(
                newest_identifier(java(), identifiers, target)
                    .unwrap_err()
                    .to_string(),
                error
            );
        }
    }
}
