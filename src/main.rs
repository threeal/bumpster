//! A command-line interface for bumping the version of a tool in a repository.

#[cfg(not(tarpaulin_include))]
fn main() {
    let version = || clap::arg!(<target> "The version to bump to").value_name("VERSION");
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
                .arg(version()),
        )
        .subcommand(
            clap::Command::new("node")
                .about("Bump Node.js in the devEngines.runtime field of package.json")
                .arg(version()),
        )
        .subcommand(
            clap::Command::new("pnpm")
                .about("Bump pnpm in the devEngines.packageManager or packageManager field of package.json")
                .arg(version()),
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

/// Returns the version to bump to, given as the target argument.
#[cfg(not(tarpaulin_include))]
fn target(matches: &clap::ArgMatches) -> &str {
    matches
        .get_one::<String>("target")
        .map_or("", String::as_str)
}
