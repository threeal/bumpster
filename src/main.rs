//! A command-line interface for bumping the version of a tool in a repository.

#[cfg(not(tarpaulin_include))]
fn main() {
    let target_arg = |tags: &'static [&'static str]| {
        clap::Arg::new("target")
            .value_name("VERSION")
            .help(format!(
                "The version to bump to, or a tag of it: {}",
                tags.join(", ")
            ))
            .default_value(tags[0])
    };
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
                .arg(target_arg(&bumpster::dprint::TAGS)),
        )
        .subcommand(
            clap::Command::new("node")
                .about("Bump Node.js in the devEngines.runtime field of package.json")
                .arg(target_arg(&bumpster::node::TAGS)),
        )
        .subcommand(
            clap::Command::new("pnpm")
                .about("Bump pnpm in the devEngines.packageManager or packageManager field of package.json")
                .arg(target_arg(&bumpster::pnpm::TAGS)),
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
            bumpster::dprint::bump(plugin, target(matches))
        }
        Some(("node", matches)) => bumpster::node::bump(target(matches)),
        Some(("pnpm", matches)) => bumpster::pnpm::bump(target(matches)),
        _ => unreachable!("a subcommand is required"),
    };

    match result {
        Ok(url) => println!("opened {url}"),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
    }
}

/// Returns the target to bump to, given as a version or a tag of it, or the
/// tool's default tag if not given.
#[cfg(not(tarpaulin_include))]
fn target(matches: &clap::ArgMatches) -> &str {
    matches
        .get_one::<String>("target")
        .map_or("", String::as_str)
}
