//! The lines a command prints when it finishes, or when the user declines.
use crate::ctx::Ctx;
use crate::error::Result;

/// `✓ Created issue ENG-1: Title`, with the URL on the next line when there
/// is one. `verb` is past tense; `label` names the entity as the user knows
/// it. Commands add any detail lines after this.
pub fn done(verb: &str, entity: &str, label: &str, url: Option<&str>) -> String {
    let mut output = format!("✓ {verb} {entity} {label}\n");
    if let Some(url) = url.filter(|url| !url.is_empty()) {
        output.push_str(url);
        output.push('\n');
    }
    output
}

/// Reports that the user declined a confirmation; the command then ends
/// successfully without changing anything.
pub fn canceled(ctx: &Ctx) -> Result<()> {
    ctx.eprint("Canceled.\n")
}

#[cfg(test)]
mod tests {
    use super::done;

    #[test]
    fn done_names_the_entity_and_puts_the_url_on_its_own_line() {
        assert_eq!(
            done(
                "Created",
                "issue",
                "ENG-1: Title",
                Some("https://linear.app/x")
            ),
            "✓ Created issue ENG-1: Title\nhttps://linear.app/x\n"
        );
        assert_eq!(
            done("Deleted", "label", "Bug", None),
            "✓ Deleted label Bug\n"
        );
        assert_eq!(
            done("Updated", "project", "P", Some("")),
            "✓ Updated project P\n"
        );
    }
}
