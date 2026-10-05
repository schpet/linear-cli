use super::*;

const MARKER: &str = "lin_api_SECRET_MARKER";

fn parse(bytes: impl Into<Vec<u8>>) -> Result<ConfigTier, ConfigParseError> {
    parse_config_tier(RawConfigFile {
        path: PathBuf::from("/tmp/linear.toml"),
        bytes: bytes.into(),
    })
}

fn parsed(text: &str) -> ConfigTier {
    parse(text).unwrap_or_else(|error| panic!("{text:?}: {error}"))
}

fn failure(bytes: impl Into<Vec<u8>>) -> ConfigParseErrorKind {
    match parse(bytes) {
        Ok(_) => panic!("expected a parse failure"),
        Err(error) => error.kind,
    }
}

#[test]
fn values_keep_their_types_and_file_order() {
    let tier = parsed("b = \"two\"\na = 1\n[c]\nd = true\n");
    assert_eq!(tier.path, PathBuf::from("/tmp/linear.toml"));
    assert_eq!(tier.table.keys().collect::<Vec<_>>(), ["b", "a", "c"]);
    assert_eq!(tier.table["b"].as_str(), Some("two"));
    assert_eq!(tier.table["a"].as_integer(), Some(1));
    assert_eq!(tier.table["c"]["d"].as_bool(), Some(true));
    assert!(parsed("").table.is_empty());
}

#[test]
fn invalid_toml_reports_line_column_and_message() {
    let kind = failure("first = 1\nfirst = 2\n");
    let ConfigParseErrorKind::InvalidToml {
        line,
        column,
        message,
    } = &kind
    else {
        panic!("{kind:?}");
    };
    assert_eq!((*line, *column), (2, 1));
    assert!(message.contains("duplicate key"), "{message}");
    assert!(
        kind.to_string()
            .starts_with("invalid TOML at line 2, column 1: "),
        "{kind}"
    );

    for text in [
        "a = [\n",
        "a = \"\\q\"\n",
        "a = 9223372036854775808\n",
        "a = 1\rb = 2\r",
    ] {
        assert!(
            matches!(failure(text), ConfigParseErrorKind::InvalidToml { .. }),
            "{text:?}"
        );
    }
}

#[test]
fn encoding_and_size_limits_are_explicit() {
    assert_eq!(failure([0xff]), ConfigParseErrorKind::InvalidUtf8);
    assert_eq!(
        failure([0xef, 0xbb, 0xbf, b'a', b'=', b'1']),
        ConfigParseErrorKind::ByteOrderMark
    );
    assert!(parsed(&" ".repeat(1024 * 1024)).table.is_empty());
    assert_eq!(
        failure(vec![b' '; 1024 * 1024 + 1]),
        ConfigParseErrorKind::TooLarge
    );
    assert_eq!(parsed("a = 1\r\nb = 2\r\n").table.len(), 2);
}

#[test]
fn deep_nesting_fails_or_parses_without_crashing() {
    let deep = |levels: usize| format!("v = {}1{}\n", "[".repeat(levels), "]".repeat(levels));
    assert!(parse(deep(50)).is_ok());
    let _ = parse(deep(100_000));
    let _ = parse(format!("v = {}\n", "[".repeat(200_000)));
}

#[test]
fn errors_never_echo_file_contents() {
    for text in [
        format!("api_key = \"{MARKER}\"\nbroken = ["),
        format!("api_key = \"{MARKER}\"\napi_key = \"other\""),
        format!("api_key = \"{MARKER}"),
    ] {
        let error = match parse(text.into_bytes()) {
            Ok(_) => panic!("expected failure"),
            Err(error) => error,
        };
        assert!(!format!("{error:?}").contains(MARKER), "{error:?}");
        assert!(!error.to_string().contains(MARKER), "{error}");
    }
}
