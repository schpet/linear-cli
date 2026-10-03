use super::*;

#[test]
fn typed_url_kinds_and_boundaries() {
    assert!(matches!(
        parse_linear_url("plain name"),
        LinearUrlParse::NotLinear
    ));
    assert!(matches!(
        parse_linear_url("\u{feff}  "),
        LinearUrlParse::NotLinear
    ));
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/issue/eng-12#comment-abcdef12"), LinearUrlParse::Known(LinearUrlRef::Issue { identifier, comment_id_prefix: Some(comment), .. }) if identifier == "ENG-12" && comment == "abcdef12")
    );
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/project/Name-ABCDEF123456"), LinearUrlParse::Known(LinearUrlRef::Project { slug_id, .. }) if slug_id == "abcdef123456")
    );
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/document/X-abcdef123456"),
        LinearUrlParse::Known(LinearUrlRef::Document { .. })
    ));
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/initiative/X-abcdef123456"),
        LinearUrlParse::Known(LinearUrlRef::Initiative { .. })
    ));
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/eng/cycle/upcoming"), LinearUrlParse::Known(LinearUrlRef::Cycle { team_key, cycle: CycleSelector::Next, .. }) if team_key == "ENG")
    );
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/ß/projects%2Fall"), LinearUrlParse::Known(LinearUrlRef::Team { team_key, .. }) if team_key == "SS")
    );
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/eng/cycle/4294967296"), LinearUrlParse::Unsupported(reason) if reason == "\"4294967296\" is not a cycle number: the largest cycle number is 4294967295")
    );
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/eng/cycle/0"), LinearUrlParse::Unsupported(reason) if reason == "\"0\" is not a cycle number: cycle numbers start at 1")
    );
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/eng/cycle/007"), LinearUrlParse::Unsupported(reason) if reason == "\"007\" is not a cycle number: it has a leading zero")
    );
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/team/eng/cycle/4294967295"),
        LinearUrlParse::Known(LinearUrlRef::Cycle {
            cycle: CycleSelector::Number(number),
            ..
        }) if number.get() == u32::MAX
    ));
    assert!(
        matches!(parse_linear_url("https://linear.app/acme/team/eng/cycle/Constructor"), LinearUrlParse::Unsupported(reason) if reason == "\"Constructor\" is not a cycle number")
    );
}

#[test]
fn hosts_anchors_and_slug_rules() {
    assert!(matches!(
        parse_linear_url("http://www.linear.app./acme/team/eng#"),
        LinearUrlParse::Known(LinearUrlRef::Team { team_key, .. }) if team_key == "ENG"
    ));
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/issue/eng-12#comment-abcdef12"),
        LinearUrlParse::Known(LinearUrlRef::Issue { comment_id_prefix: Some(prefix), .. }) if prefix == "abcdef12"
    ));
    assert!(matches!(
        parse_linear_url("https://linear.app/acme/project/ABCDEF123456/activity#project-update-abcdef12"),
        LinearUrlParse::Known(LinearUrlRef::Project { slug_id, .. }) if slug_id == "abcdef123456"
    ));
    assert_eq!(
        parse_linear_url("https://linear.app/acme/issue/ENG-0"),
        LinearUrlParse::Unsupported("\"ENG-0\" is not an issue identifier".to_owned())
    );
    assert_eq!(
        parse_linear_url("https://linear.app/acme/issue/ENG-01"),
        LinearUrlParse::Unsupported("\"ENG-01\" is not an issue identifier".to_owned())
    );
    assert_eq!(
        parse_linear_url("https://linear.app/acme/project/no-id"),
        LinearUrlParse::Unsupported("\"no-id\" does not end in a Linear slug ID".to_owned())
    );
    assert_eq!(
        parse_linear_url("https://linear.app/acme/issue/ENG-1#other"),
        LinearUrlParse::Unsupported("\"#other\" is not a comment link".to_owned())
    );
    assert_eq!(
        parse_linear_url("https://linear.app/acme/document/X-abcdef123456#other"),
        LinearUrlParse::Unsupported("\"#other\" is not a link this command can use".to_owned())
    );
}

#[test]
fn descendant_refusals_are_specific() {
    for (input, reason) in [
        (
            "https://linear.app/acme/team/eng/secret",
            "\"secret\" is not a team page this command can use",
        ),
        (
            "https://linear.app/acme/team/eng/Cycle/3",
            "\"Cycle/3\" is not a team page this command can use",
        ),
        (
            "https://linear.app/acme/project/X-abcdef123456/secret",
            "\"secret\" is not a page this command can use",
        ),
        (
            "https://linear.app/acme/team/eng/cycle",
            "it does not name a cycle",
        ),
        (
            "https://linear.app/acme/team/eng/cycle/3/extra",
            "\"3/extra\" is not a cycle page",
        ),
    ] {
        assert_eq!(
            parse_linear_url(input),
            LinearUrlParse::Unsupported(reason.to_owned()),
            "{input}"
        );
    }
}
