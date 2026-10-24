//! A command-line interface for bumping the version of a tool in a repository.

#[cfg(not(tarpaulin_include))]
fn main() {
    let matches = clap::Command::new("bumpster")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Bump the version of a tool in the current repository")
        .arg(clap::arg!(<tool> "The tool to bump"))
        .arg(
            clap::arg!([target] "The version to bump to, defaults to the latest")
                .value_name("VERSION"),
        )
        .get_matches();

    let tool = matches.get_one::<String>("tool").map_or("", String::as_str);
    let version = matches.get_one::<String>("target").map(String::as_str);

    if let Err(err) = bumpster::bump(tool, version) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
