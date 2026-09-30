//! MIT mdast-util-to-markdown 2.1.2 encode-info and micromark classification.
use alloc::{
    format,
    string::{String, ToString},
};
use regex::Regex;

#[derive(Clone, Copy)]
pub struct Surrounding {
    pub before: bool,
    pub after: bool,
}
struct Sides {
    inside: bool,
    outside: bool,
}
#[derive(PartialEq)]
enum Kind {
    Letter,
    Whitespace,
    Punctuation,
}

fn classify(unit: Option<u16>) -> Kind {
    let Some(character) = unit.and_then(|unit| char::from_u32(u32::from(unit))) else {
        return Kind::Letter;
    };
    // ECMAScript \s excludes NEL and includes BOM; classification is per UTF-16 unit.
    if (character.is_whitespace() && character != '\u{85}') || character == '\u{feff}' {
        return Kind::Whitespace;
    }
    if Regex::new(r"[\p{P}\p{S}]")
        .expect("Unicode punctuation pattern")
        .is_match(&character.to_string())
    {
        Kind::Punctuation
    } else {
        Kind::Letter
    }
}

fn sides(outside: Option<u16>, inside: Option<u16>, marker: char) -> Sides {
    match (classify(outside), classify(inside)) {
        (Kind::Letter, Kind::Letter) => Sides {
            inside: marker == '_',
            outside: marker == '_',
        },
        (Kind::Letter | Kind::Whitespace, Kind::Whitespace) => Sides {
            inside: true,
            outside: true,
        },
        (Kind::Letter, Kind::Punctuation) => Sides {
            inside: false,
            outside: true,
        },
        (Kind::Punctuation, Kind::Whitespace) => Sides {
            inside: true,
            outside: false,
        },
        (Kind::Whitespace | Kind::Punctuation, Kind::Letter | Kind::Punctuation) => Sides {
            inside: false,
            outside: false,
        },
    }
}

pub fn reference(unit: u16) -> String {
    format!("&#x{:X};", unit)
}
pub fn encode_first(value: &str) -> String {
    let units = value.encode_utf16().collect::<alloc::vec::Vec<_>>();
    let Some(first) = units.first() else {
        return String::new();
    };
    format!(
        "{}{}",
        reference(*first),
        String::from_utf16_lossy(&units[1..])
    )
}
pub fn encode_last(value: &str) -> String {
    let units = value.encode_utf16().collect::<alloc::vec::Vec<_>>();
    let Some(last) = units.last() else {
        return String::new();
    };
    format!(
        "{}{}",
        String::from_utf16_lossy(&units[..units.len() - 1]),
        reference(*last)
    )
}

pub fn encode(
    mut between: String,
    before: &str,
    after: &str,
    marker: char,
) -> (String, Surrounding) {
    let open = sides(
        before.encode_utf16().last(),
        between.encode_utf16().next(),
        marker,
    );
    if open.inside {
        between = encode_first(&between);
    }
    let close = sides(
        after.encode_utf16().next(),
        between.encode_utf16().last(),
        marker,
    );
    if close.inside {
        between = encode_last(&between);
    }
    (
        between,
        Surrounding {
            before: open.outside,
            after: close.outside,
        },
    )
}
