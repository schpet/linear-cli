//! `issue commits`: the jj commits whose trailers name an issue.
use crate::{
    cli::issue::IssueCommits,
    config::Vcs,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    platform::process,
};

pub fn run(ctx: &Ctx, args: &IssueCommits) -> Result<()> {
    show_commits(ctx, args).context("Failed to show commits")
}

fn show_commits(ctx: &Ctx, args: &IssueCommits) -> Result<()> {
    check_vcs(super::vcs(ctx))?;
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    ctx.spin(true, super::id::fetch(client, &identifier))?;
    ctx.flush()?;
    let revset = revset(&identifier);
    let jj = || process::command("jj", ctx.cwd(), &ctx.config().child_env);
    let probe = process::checked_output(jj().args([
        "log",
        "-r",
        &revset,
        "--no-graph",
        "-T",
        "commit_id",
    ]))?;
    if process::text(&probe.stdout).is_empty() {
        return Err(Error::not_found("Commits", &identifier));
    }
    // jj reports its own failures, so its exit status becomes ours.
    process::exit_like(process::status(jj().args([
        "log",
        "-r",
        &revset,
        "-p",
        "--git",
        "--no-graph",
        "-T",
        "builtin_log_compact_full_description",
    ]))?)
}

fn check_vcs(vcs: Vcs) -> Result<()> {
    match vcs {
        Vcs::Jj => Ok(()),
        Vcs::Git => Err(Error::new("commits is only supported with jj-vcs")
            .with_hint("This command requires jujutsu (jj) version control.")),
    }
}

/// The changes with a `Linear-issue` trailer naming exactly `identifier`
/// (`ENG-1` and not `ENG-10`).
fn revset(identifier: &str) -> String {
    let pattern = format!(r"(?mi)^Linear-issue:.*\b{}\b", regex_escape(identifier));
    format!("description(regex:{})", string_literal(&pattern))
}

fn regex_escape(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            let escape = !(c.is_ascii_alphanumeric() || c == '_' || c == '-');
            escape.then_some('\\').into_iter().chain([c])
        })
        .collect()
}

/// `text` as a double-quoted jj revset string.
fn string_literal(text: &str) -> String {
    let mut literal = String::from("\"");
    for c in text.chars() {
        if matches!(c, '"' | '\\') {
            literal.push('\\');
        }
        literal.push(c);
    }
    literal.push('"');
    literal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_revset_matches_the_whole_identifier() {
        assert_eq!(
            revset("ENG-1"),
            r#"description(regex:"(?mi)^Linear-issue:.*\\bENG-1\\b")"#
        );
        assert_eq!(regex_escape("A.B-1*"), r"A\.B-1\*");
        assert_eq!(string_literal(r#"a"b\c"#), r#""a\"b\\c""#);
    }
}
