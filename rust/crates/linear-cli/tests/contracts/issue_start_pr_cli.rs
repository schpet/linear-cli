use linear_cli::cli::{self, RootCommand, issue::IssueCommand};
use std::ffi::OsString;
fn parse(args: &[&str]) -> cli::Cli {
    cli::parse(&args.iter().map(OsString::from).collect::<Vec<_>>()).unwrap()
}
#[test]
fn source_valid_empty_start_and_pr_values_reach_command_policy_and_pr_alias() {
    let start = parse(&["issue", "start", "ENG-1", "--branch", "", "--from-ref", ""]);
    let Some(RootCommand::Issue(issue)) = start.command else {
        panic!("issue")
    };
    let Some(IssueCommand::Start(start)) = issue.command else {
        panic!("start")
    };
    assert_eq!(start.branch.as_deref(), Some(""));
    assert_eq!(start.from_ref.as_deref(), Some(""));
    let pr = parse(&[
        "issue",
        "pr",
        "ENG-1",
        "--title",
        "",
        "--base",
        "",
        "--head",
        "",
        "--template",
        " ",
        "--no-template",
    ]);
    let Some(RootCommand::Issue(issue)) = pr.command else {
        panic!("issue")
    };
    let Some(IssueCommand::PullRequest(pr)) = issue.command else {
        panic!("pr alias")
    };
    assert_eq!(pr.title.as_deref(), Some(""));
    assert_eq!(pr.base.as_deref(), Some(""));
    assert_eq!(pr.head.as_deref(), Some(""));
    assert_eq!(pr.template.as_deref(), Some(" "));
    assert!(pr.no_template);
}
