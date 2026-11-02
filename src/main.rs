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
    let yes_arg = || clap::arg!(-y --yes "Open the pull request without asking to confirm it");
    let matches = clap::Command::new("bumpster")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Bump the version of a tool in the current repository")
        .subcommand_required(true)
        .subcommand(
            clap::Command::new("dprint")
                .about("Bump a dprint plugin in the plugins field of dprint.json")
                .arg(clap::arg!(<plugin> "The plugin to bump").value_parser(
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
                .arg(target_arg(&bumpster::pnpm::TAGS, "pnpm's latest release (latest)"))
                .arg(yes_arg()),
        )
        .get_matches();

    let result = match matches.subcommand() {
        Some(("dprint", matches)) => {
            let plugin = matches
                .get_one::<String>("plugin")
                .map_or("", String::as_str);
            let (_, plugin) = bumpster::dprint::PLUGINS
                .iter()
                .find(|(name, _)| *name == plugin)
                .expect("the plugin is validated by clap");
            bumpster::dprint::prepare(plugin, target(matches))
                .and_then(|bump| finish(&bump.change(), yes(matches), || bump.apply()))
        }
        Some(("node", matches)) => bumpster::node::prepare(target(matches))
            .and_then(|bump| finish(&bump.change(), yes(matches), || bump.apply())),
        Some(("pnpm", matches)) => bumpster::pnpm::prepare(target(matches))
            .and_then(|bump| finish(&bump.change(), yes(matches), || bump.apply())),
        _ => unreachable!("a subcommand is required"),
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
/// then applying it and showing the URL of its opened pull request.
#[cfg(not(tarpaulin_include))]
fn finish(
    change: &str,
    yes: bool,
    apply: impl FnOnce() -> anyhow::Result<String>,
) -> anyhow::Result<()> {
    println!("will update {change}");
    if !yes {
        if !std::io::stdin().is_terminal() {
            anyhow::bail!(
                "no terminal to confirm the bump on, pass --yes to make it without confirming"
            );
        }
        if !bumpster::prompt::confirm("open a pull request")? {
            println!("aborted");
            return Ok(());
        }
    }
    println!("opened {}", apply()?);
    Ok(())
}
