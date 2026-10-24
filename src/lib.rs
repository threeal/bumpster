//! A library for bumping the version of a tool in a repository.

/// Bumps the given tool in the current repository to the given version, or to
/// the latest version if none is given.
///
/// # Errors
///
/// Returns an error if the tool is not supported.
pub fn bump(tool: &str, _version: Option<&str>) -> Result<(), String> {
    Err(format!("unsupported tool: {tool}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_unsupported_tool() {
        assert_eq!(bump("foo", None), Err("unsupported tool: foo".to_string()));
        assert_eq!(
            bump("foo", Some("1.0.0")),
            Err("unsupported tool: foo".to_string())
        );
    }
}
