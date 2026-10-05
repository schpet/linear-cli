/// Whether a reference is UUID-shaped; version and variant bits are not checked.
pub fn is_linear_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_shape_matches_linear_without_version_or_variant_rules() {
        assert!(is_linear_uuid("ABCDEF01-2345-6789-abCD-ef0123456789"));
        assert!(is_linear_uuid("00000000-0000-0000-0000-000000000000"));
        for input in [
            "abcdef01-2345-6789-abcd-ef012345678",
            "abcdef012345-6789-abcd-ef0123456789",
            "abcdef01-2345-6789-abcd-ef012345678g",
            "abcdef01-2345-6789-abcd-ef0123456789\n",
            "abcdef01-2345-6789-abcd-ef012345678é",
        ] {
            assert!(!is_linear_uuid(input), "{input}");
        }
    }
}
