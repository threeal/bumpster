//! A command-line interface for bumping the version of a tool in a repository.

use std::io::IsTerminal;

#[cfg(not(tarpaulin_include))]
fn main() {
    let target_arg = |tags: &'static [&'static str], default: &str| {
        clap::Arg::new("target").value_name("VERSION").help(format!(
            "The version to bump to, or a tag of it: {}. If not given, bumps to {default}",
            tags.join(", ")
        ))
    };
    let yes_arg = || clap::arg!(-y --yes "Answer yes to every question, including discarding uncommitted changes");
    let matches = clap::Command::new("bumpster")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Bump the version of a tool, or every outdated one, in the current repository")
        .args_conflicts_with_subcommands(true)
        .arg(yes_arg())
        .subcommand(
            clap::Command::new("dprint")
                .about("Bump a dprint plugin, or every outdated one, in the plugins field of dprint.json")
                .arg(clap::arg!([plugin] "The plugin to bump. If not given, bumps every plugin in dprint.json that is behind its latest release").value_parser(
                    clap::builder::PossibleValuesParser::new(
                        bumpster::dprint::PLUGINS.iter().map(|(plugin, _)| *plugin),
                    ),
                ))
                .arg(target_arg(
                    &bumpster::dprint::TAGS,
                    "the plugin's latest release (latest)",
                ))
                .arg(yes_arg()),
        )
        .subcommand(
            clap::Command::new("node")
                .about("Bump Node.js in the devEngines.runtime field of package.json")
                .arg(target_arg(
                    &bumpster::node::TAGS,
                    "the newest release of its current major version",
                ))
                .arg(yes_arg()),
        )
        .subcommand(
            clap::Command::new("pnpm")
                .about("Bump pnpm in the devEngines.packageManager or packageManager field of package.json")
                .arg(target_arg(
                    &bumpster::pnpm::TAGS,
                    "the newest release of its current major version",
                ))
                .arg(yes_arg()),
        )
        .get_matches();

    let result = match matches.subcommand() {
        Some(("dprint", matches)) => matches.get_one::<String>("plugin").map_or_else(
            || {
                bumpster::dprint::prepare_outdated(yes(matches)).and_then(|bumps| {
                    let bumps = bumps.into_iter().map(bumpster::all::Bump::Dprint);
                    finish_all(bumps.collect(), "every plugin is up to date", yes(matches))
                })
            },
            |plugin| bump_dprint_plugin(plugin, matches),
        ),
        Some(("node", matches)) => bumpster::node::prepare(target(matches), yes(matches))
            .and_then(|bump| finish(&bump.change(), yes(matches), || bump.apply())),
        Some(("pnpm", matches)) => bumpster::pnpm::prepare(target(matches), yes(matches))
            .and_then(|bump| finish(&bump.change(), yes(matches), || bump.apply())),
        None => bumpster::all::prepare_outdated(yes(&matches))
            .and_then(|bumps| finish_all(bumps, "everything is up to date", yes(&matches))),
        Some((name, _)) => unreachable!("{name} is not a subcommand"),
    };

    if let Err(err) = result {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

/// Returns the target to bump to, given as a version or a tag of it, or `None`
/// if not given, leaving the tool to decide what to bump to.
#[cfg(not(tarpaulin_include))]
fn target(matches: &clap::ArgMatches) -> Option<&str> {
    matches.get_one::<String>("target").map(String::as_str)
}

/// Returns whether the bump is made without asking to confirm it, given as
/// the yes flag.
#[cfg(not(tarpaulin_include))]
fn yes(matches: &clap::ArgMatches) -> bool {
    matches.get_flag("yes")
}

/// Finishes a prepared bump that makes the given change, by showing the
/// change and, unless confirmation is skipped, asking whether to go ahead,
/// then applying it and showing the URL of its pull request, saying whether it
/// was opened or an already open one was updated.
#[cfg(not(tarpaulin_include))]
fn finish(
    change: &str,
    yes: bool,
    apply: impl FnOnce() -> anyhow::Result<bumpster::PullRequest>,
) -> anyhow::Result<()> {
    println!("will update {change}");
    if !confirm("open a pull request", yes)? {
        println!("aborted");
        return Ok(());
    }
    show(&apply()?);
    Ok(())
}

/// Bumps the dprint plugin with the given short name to the target given in
/// the arguments.
#[cfg(not(tarpaulin_include))]
fn bump_dprint_plugin(plugin: &str, matches: &clap::ArgMatches) -> anyhow::Result<()> {
    let (_, plugin) = bumpster::dprint::PLUGINS
        .iter()
        .find(|(name, _)| *name == plugin)
        .expect("the plugin is validated by clap");
    bumpster::dprint::prepare(plugin, target(matches), yes(matches))
        .and_then(|bump| finish(&bump.change(), yes(matches), || bump.apply()))
}

/// Finishes the prepared bumps of everything that is outdated, by showing
/// their changes and, unless confirmation is skipped, asking once whether to
/// go ahead, then applying them one after another from the main branch,
/// showing the URL of each pull request as it goes and stopping at the first
/// failure.
///
/// If there are no bumps, it shows the given message saying everything is up
/// to date instead.
#[cfg(not(tarpaulin_include))]
fn finish_all(bumps: Vec<bumpster::all::Bump>, up_to_date: &str, yes: bool) -> anyhow::Result<()> {
    let question = match bumps.len() {
        0 => {
            println!("{up_to_date}");
            return Ok(());
        }
        1 => "open a pull request".to_string(),
        count => format!("open {count} pull requests"),
    };
    for bump in &bumps {
        println!("will update {}", bump.change());
    }
    if !confirm(&question, yes)? {
        println!("aborted");
        return Ok(());
    }
    for bump in bumps {
        show(&bump.refresh(yes)?.apply()?);
    }
    Ok(())
}

/// Confirms going ahead with the bump by asking the given question with yes as
/// the default answer, unless confirmation is skipped.
///
/// Returns an error if there's no terminal to ask on.
#[cfg(not(tarpaulin_include))]
fn confirm(question: &str, yes: bool) -> anyhow::Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "no terminal to confirm the bump on, pass --yes to make it without confirming"
        );
    }
    bumpster::prompt::confirm(question, true)
}

/// Shows the URL of the pull request of a bump, saying whether it was opened
/// or an already open one was updated.
#[cfg(not(tarpaulin_include))]
fn show(pull_request: &bumpster::PullRequest) {
    if pull_request.opened {
        println!("opened {}", pull_request.url);
    } else {
        println!("updated {}", pull_request.url);
    }
}
