//! A command-line interface for bumping the version of a tool in a repository.

#[cfg(not(tarpaulin_include))]
fn main() {
    let matches = clap::Command::new("bumpster")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Bump the version of a tool in the current repository")
        .subcommand_required(true)
        .subcommand(
            clap::Command::new("pnpm")
                .about("Bump pnpm in the packageManager field of package.json")
                .arg(clap::arg!(<target> "The version to bump to").value_name("VERSION")),
        )
        .get_matches();

    let result = match matches.subcommand() {
        Some(("pnpm", matches)) => {
            let version = matches
                .get_one::<String>("target")
                .map_or("", String::as_str);
            bumpster::pnpm::bump(version)
        }
        _ => unreachable!("a subcommand is required"),
    };

    match result {
        Ok(pull_request) => println!("{pull_request}"),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
    }
}
