//! Opt-in source printer layout for schema-generated built-in write lookups.
//! GraphQL.js wraps a field's single filter argument when its prefix exceeds80
//! UTF16 units. Cynic prints it on one line. This affects captured SDK error
//! text as well as request bytes; no user GraphQL document is parsed here.
pub fn printed_builtin(query: &str) -> String {
    let mut lines = Vec::new();
    for line in query.trim_end_matches('\n').lines() {
        let field = line.trim_start();
        let Some(prefix) = field.strip_suffix(" {") else {
            lines.push(line.to_owned());
            continue;
        };
        if prefix.encode_utf16().count() <= 80 || !prefix.contains("(filter: ") {
            lines.push(line.to_owned());
            continue;
        }
        // All callers use typed operations with one filter argument on these
        // fields. Assert generated shape rather than accepting a printer drift.
        let (name, filter) = prefix
            .split_once("(filter: ")
            .unwrap_or_else(|| unreachable!("filter argument checked"));
        let filter = filter
            .strip_suffix(')')
            .unwrap_or_else(|| unreachable!("generated filter field ends in a parenthesis"));
        assert!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric()
                        || character == ':'
                        || character == ' '),
            "unexpected generated field name"
        );
        let indent = &line[..line.len() - field.len()];
        lines.push(format!("{indent}{name}("));
        lines.push(format!("{indent}  filter: {filter}"));
        lines.push(format!("{indent}) {{"));
    }
    lines.join("\n")
}
