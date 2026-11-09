//! Prompts that ask the user on the terminal before going ahead.

use anyhow::{Context, Result};
use std::io::{BufRead, Write};

/// Asks the user whether to go ahead with the given question, answered with
/// `default` if the answer is empty.
///
/// Returns whether the user answered yes, or `false` if the input ended
/// without an answer.
///
/// # Errors
///
/// Returns an error if the question can't be shown or the answer can't be
/// read.
#[cfg(not(tarpaulin_include))] // Reads from the terminal, so it is not unit tested.
pub fn confirm(question: &str, default: bool) -> Result<bool> {
    let choices = if default { "Y/n" } else { "y/N" };
    print!("{question}? [{choices}] ");
    std::io::stdout()
        .flush()
        .context("failed to show the prompt")?;
    let mut answer = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut answer)
        .context("failed to read the answer")?;
    Ok(read > 0 && is_yes(&answer, default))
}

/// Returns whether an answer to a prompt is yes, which is either `y` or `yes`
/// in any case, or `default` if the answer is empty.
#[must_use]
pub fn is_yes(answer: &str, default: bool) -> bool {
    match answer.trim().to_lowercase().as_str() {
        "" => default,
        answer => matches!(answer, "y" | "yes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_yes_answers() {
        for default in [true, false] {
            for answer in ["y\n", "Y", "yes", " YES \n"] {
                assert!(is_yes(answer, default), "{answer:?}");
            }
            for answer in ["n\n", "N", "no", "nope", "x"] {
                assert!(!is_yes(answer, default), "{answer:?}");
            }
            for answer in ["", "\n", " \n"] {
                assert_eq!(is_yes(answer, default), default, "{answer:?}");
            }
        }
    }
}
