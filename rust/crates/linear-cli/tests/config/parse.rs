use std::error::Error;
use std::path::PathBuf;

use linear_cli::config::{
    ConfigParseErrorKind, ConfigTier, ConfigValue, RawConfigFile, parse_config_tier,
};

const PATH: &str = "/private/config/linear.toml";
const MARKER: &str = "lin_api_fake_marker";

fn parse_bytes(
    bytes: impl Into<Vec<u8>>,
) -> Result<ConfigTier, linear_cli::config::ConfigParseError> {
    parse_config_tier(RawConfigFile {
        path: PathBuf::from(PATH),
        bytes: bytes.into(),
    })
}

fn parsed(text: &str) -> ConfigTier {
    match parse_bytes(text.as_bytes()) {
        Ok(tier) => tier,
        Err(error) => panic!("unexpected parse failure: {error}"),
    }
}

fn failed(bytes: impl Into<Vec<u8>>, expected: ConfigParseErrorKind) {
    let error = match parse_bytes(bytes) {
        Ok(_) => panic!("expected whole-tier failure"),
        Err(error) => error,
    };
    assert_eq!(error.kind, expected);
    assert_eq!(error.path, PathBuf::from(PATH));
    assert!(error.source().is_none());
}

fn names(entries: &[(String, ConfigValue)]) -> Vec<&str> {
    entries.iter().map(|(key, _)| key.as_str()).collect()
}

fn entry<'a>(entries: &'a [(String, ConfigValue)], key: &str) -> &'a ConfigValue {
    &entries
        .iter()
        .find(|(name, _)| name == key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .1
}

#[test]
fn empty_tier_is_selected_and_retains_its_path() {
    let tier = parsed("");
    assert_eq!(tier.path, PathBuf::from(PATH));
    assert!(tier.entries.is_empty());
}

#[test]
fn source_order_survives_mixed_workspace_and_inline_values() {
    let before = parsed("workspaces = { beta = \"B\", alpha = \"A\" }\nprimary = \"P\"\n");
    assert_eq!(names(&before.entries), ["workspaces", "primary"]);
    let ConfigValue::Table(workspaces) = entry(&before.entries, "workspaces") else {
        panic!("workspaces lost table shape");
    };
    assert_eq!(names(workspaces), ["beta", "alpha"]);
    let after = parsed("primary = \"P\"\nworkspaces = { alpha = \"A\", beta = \"B\" }\n");
    assert_eq!(names(&after.entries), ["primary", "workspaces"]);
    let ConfigValue::Table(workspaces) = entry(&after.entries, "workspaces") else {
        panic!("workspaces lost table shape");
    };
    assert_eq!(names(workspaces), ["alpha", "beta"]);
}

#[test]
fn numeric_looking_keys_and_nested_header_order_are_raw_toml_order() {
    let tier = parsed(
        "[workspaces]\n\"2\" = \"two\"\n\"10\" = \"ten\"\n\"01\" = \"zero-one\"\n\"-1\" = \"minus\"\na = \"alpha\"\n[nested]\nz = 1\na = 2\n",
    );
    assert_eq!(names(&tier.entries), ["workspaces", "nested"]);
    let ConfigValue::Table(workspaces) = entry(&tier.entries, "workspaces") else {
        panic!("workspaces lost table shape");
    };
    assert_eq!(names(workspaces), ["2", "10", "01", "-1", "a"]);
    let ConfigValue::Table(nested) = entry(&tier.entries, "nested") else {
        panic!("nested lost table shape");
    };
    assert_eq!(names(nested), ["z", "a"]);
}

#[test]
fn unicode_keys_and_values_are_preserved() {
    let tier = parsed("\"é\" = \"naïve\"\n\"\\u00fc\" = \"other\"\n");
    assert_eq!(names(&tier.entries), ["é", "ü"]);
    assert!(matches!(entry(&tier.entries, "é"), ConfigValue::String(value) if value == "naïve"));
}

#[test]
fn array_of_tables_keeps_item_and_field_order() {
    let tier = parsed("[[servers]]\nz = 1\na = 2\n[[servers]]\na = 3\nz = 4\n");
    let ConfigValue::Array(servers) = entry(&tier.entries, "servers") else {
        panic!("array of tables lost array shape");
    };
    assert_eq!(servers.len(), 2);
    let ConfigValue::Table(first) = &servers[0] else {
        panic!("first item lost table shape");
    };
    let ConfigValue::Table(second) = &servers[1] else {
        panic!("second item lost table shape");
    };
    assert_eq!(names(first), ["z", "a"]);
    assert_eq!(names(second), ["a", "z"]);
}

#[test]
fn scalar_and_container_types_remain_distinct_and_owned() {
    let tier = parsed(concat!(
        "string = \"hello\"\n",
        "boolean = true\n",
        "min = -9223372036854775808\n",
        "max = 9223372036854775807\n",
        "float = 1.25\n",
        "positive_inf = inf\n",
        "negative_inf = -inf\n",
        "nan = nan\n",
        "date = 2026-09-24T12:34:56Z\n",
        "array = [1, \"two\", false]\n",
        "inline = { z = 1, a = { y = 2, x = 3 } }\n",
        "api_key = 17\n",
    ));
    assert!(
        matches!(entry(&tier.entries, "string"), ConfigValue::String(value) if value == "hello")
    );
    assert!(matches!(
        entry(&tier.entries, "boolean"),
        ConfigValue::Boolean(true)
    ));
    assert!(matches!(
        entry(&tier.entries, "min"),
        ConfigValue::Integer(i64::MIN)
    ));
    assert!(matches!(
        entry(&tier.entries, "max"),
        ConfigValue::Integer(i64::MAX)
    ));
    assert!(matches!(entry(&tier.entries, "float"), ConfigValue::Float(value) if *value == 1.25));
    assert!(
        matches!(entry(&tier.entries, "positive_inf"), ConfigValue::Float(value) if value.is_infinite() && value.is_sign_positive())
    );
    assert!(
        matches!(entry(&tier.entries, "negative_inf"), ConfigValue::Float(value) if value.is_infinite() && value.is_sign_negative())
    );
    assert!(matches!(entry(&tier.entries, "nan"), ConfigValue::Float(value) if value.is_nan()));
    assert!(
        matches!(entry(&tier.entries, "date"), ConfigValue::Datetime(value) if value == "2026-09-24T12:34:56Z")
    );
    let ConfigValue::Array(array) = entry(&tier.entries, "array") else {
        panic!("array lost shape")
    };
    assert_eq!(array.len(), 3);
    assert!(matches!(&array[0], ConfigValue::Integer(1)));
    assert!(matches!(&array[1], ConfigValue::String(value) if value == "two"));
    assert!(matches!(&array[2], ConfigValue::Boolean(false)));
    let ConfigValue::Table(inline) = entry(&tier.entries, "inline") else {
        panic!("inline lost shape")
    };
    assert_eq!(names(inline), ["z", "a"]);
    let ConfigValue::Table(deep) = entry(inline, "a") else {
        panic!("nested inline lost shape")
    };
    assert_eq!(names(deep), ["y", "x"]);
    assert!(matches!(
        entry(&tier.entries, "api_key"),
        ConfigValue::Integer(17)
    ));
}

#[test]
fn malformed_duplicate_partial_and_overflow_fail_atomically() {
    for text in [
        "first = 1\nbroken = [\n",
        "first = 1\nfirst = 2\n",
        "first = 1\nvalue = \"\\q\"\n",
        "first = 1\nvalue = \"\\uD800\"\n",
        "first = 1\nvalue = 9223372036854775808\n",
        "first = 1\nvalue = -9223372036854775809\n",
    ] {
        failed(text.as_bytes(), ConfigParseErrorKind::InvalidToml);
    }
}

#[test]
fn utf8_bom_and_size_are_explicit_strict_boundaries() {
    failed([0xff], ConfigParseErrorKind::InvalidUtf8);
    failed(
        [0xef, 0xbb, 0xbf, b'a', b'=', b'1'],
        ConfigParseErrorKind::ByteOrderMark,
    );
    failed(
        [0xef, 0xbb, 0xbf, b'a', b'=', 0xff],
        ConfigParseErrorKind::InvalidUtf8,
    );
    assert!(parsed(&" ".repeat(1024 * 1024)).entries.is_empty());
    failed(vec![b' '; 1024 * 1024 + 1], ConfigParseErrorKind::TooLarge);
}

#[test]
fn line_endings_have_explicit_outcomes() {
    let tier = parsed("a = 1\r\nb = 2\r\n");
    assert_eq!(names(&tier.entries), ["a", "b"]);
    failed(b"a = 1\rb = 2\r", ConfigParseErrorKind::InvalidToml);
}

#[test]
fn pinned_toml_one_point_one_forms_are_explicit_parser_differences() {
    for text in [
        "a = { b = 1,\n c = 2 }\n",
        "a = { b = 1, }\n",
        "a = \"\\e\"\n",
        "a = \"\\x41\"\n",
        "a = 12:34\n",
    ] {
        let tier = parsed(text);
        assert_eq!(names(&tier.entries), ["a"]);
    }
    let escaped = parsed("a = \"\\e\"\n");
    assert!(
        matches!(entry(&escaped.entries, "a"), ConfigValue::String(value) if value == "\u{1b}")
    );
    let hex = parsed("a = \"\\x41\"\n");
    assert!(matches!(entry(&hex.entries, "a"), ConfigValue::String(value) if value == "A"));
    let time = parsed("a = 12:34\n");
    assert!(matches!(
        entry(&time.entries, "a"),
        ConfigValue::Datetime(_)
    ));
}

#[test]
fn depth_cap_counts_arrays_inline_tables_dotted_keys_and_headers() {
    let array = |levels: usize| format!("v = {}1{}\n", "[".repeat(levels), "]".repeat(levels));
    assert_eq!(names(&parsed(&array(63)).entries), ["v"]);
    failed(array(64), ConfigParseErrorKind::TooDeep);

    let inline =
        |levels: usize| format!("v = {}1{}\n", "{ a = ".repeat(levels), " }".repeat(levels));
    assert_eq!(names(&parsed(&inline(63)).entries), ["v"]);
    failed(inline(64), ConfigParseErrorKind::TooDeep);

    let dotted = |levels: usize| format!("{} = 1\n", vec!["a"; levels].join("."));
    assert_eq!(names(&parsed(&dotted(63)).entries), ["a"]);
    assert_eq!(names(&parsed(&dotted(64)).entries), ["a"]);
    failed(dotted(65), ConfigParseErrorKind::TooDeep);

    let header = |levels: usize| format!("[{}]\nv = 1\n", vec!["a"; levels].join("."));
    assert_eq!(names(&parsed(&header(62)).entries), ["a"]);
    assert_eq!(names(&parsed(&header(63)).entries), ["a"]);
    failed(header(64), ConfigParseErrorKind::TooDeep);
}

#[test]
fn extreme_depth_fails_without_panicking_or_exposing_input() {
    let bracket_run = format!("v = {}\n", "[".repeat(200_000));
    failed(bracket_run, ConfigParseErrorKind::InvalidToml);

    let dotted = format!("{} = 1\n", vec!["a"; 100].join("."));
    failed(dotted, ConfigParseErrorKind::InvalidToml);
}

#[test]
fn successful_values_and_all_failure_categories_have_no_secret_formatter_or_source() {
    let tier = parsed(&format!("api_key = \"{MARKER}\"\n"));
    assert!(
        matches!(entry(&tier.entries, "api_key"), ConfigValue::String(value) if value == MARKER)
    );
    for (text, kind) in [
        (
            format!("api_key = \"{MARKER}\"\nbroken = ["),
            ConfigParseErrorKind::InvalidToml,
        ),
        (
            format!("api_key = \"{MARKER}\"\napi_key = \"other\""),
            ConfigParseErrorKind::InvalidToml,
        ),
        (
            format!("api_key = \"{MARKER}\"\nhuge = 9223372036854775808"),
            ConfigParseErrorKind::InvalidToml,
        ),
        (
            format!("api_key = \"{MARKER}\"\n{} = 1", vec!["a"; 65].join(".")),
            ConfigParseErrorKind::TooDeep,
        ),
    ] {
        let error = match parse_bytes(text.into_bytes()) {
            Ok(_) => panic!("expected failure"),
            Err(error) => error,
        };
        assert_eq!(error.kind, kind);
        assert!(!format!("{error:?}").contains(MARKER));
        assert!(!error.to_string().contains(MARKER));
        assert!(error.source().is_none());
    }
}
