//! A command-line interface for bumping the version of a tool in a repository.

#[cfg(not(tarpaulin_include))]
fn main() {
    let version = || clap::arg!(<target> "The version to bump to").value_name("VERSION");
    let matches = clap::Command::new("bumpster")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Bump the version of a tool in the current repository")
        .subcommand_required(true)
        .subcommand(
            clap::Command::new("node")
                .about("Bump Node.js in the devEngines.runtime field of package.json")
                .arg(version()),
        )
        .subcommand(
            clap::Command::new("pnpm")
                .about("Bump pnpm in the packageManager field of package.json")
                .arg(version()),
        )
        .get_matches();

    let (tool, matches) = match matches.subcommand() {
        Some(("node", matches)) => (&bumpster::node::TOOL, matches),
        Some(("pnpm", matches)) => (&bumpster::pnpm::TOOL, matches),
        _ => unreachable!("a subcommand is required"),
    };
    let version = matches
        .get_one::<String>("target")
        .map_or("", String::as_str);

    match tool.bump(version) {
        Ok(pull_request) => println!("{pull_request}"),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
    }
}
