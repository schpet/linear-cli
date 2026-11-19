//! Terminal styling is applied only after the caller has laid out the spans.
use std::borrow::Cow;
use std::collections::hash_map::{Entry, RandomState};
use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;

use minijinja::value::{
    Kwargs, Object, Rest, Serde, StringInput, Value, ValueOrKwargs, from_args, merge_maps,
};
use minijinja::{AutoEscape, Error, ErrorKind, Output, State, Template};
use serde::Serialize;

use crate::platform::markdown_terminal;

const MAX_DISPLAY_BYTES: usize = 4 * 1024 * 1024;
const STYLED_KEY: &str = "\0linear-display-styled";
const TOKEN_START: char = '\u{1f}';
const TOKEN_END: char = '\u{1e}';
// Two 64-bit hashes each take 13 base-32 digits. The alphabet uses two-byte
// C1 control characters so a sliced token has no printable fragments.
const TOKEN_BYTES: usize = 52;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DisplayText {
    pub spans: Vec<DisplaySpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplaySpan {
    pub text: String,
    pub url: Option<String>,
}

impl DisplayText {
    pub fn plain(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }

    pub fn terminal(&self) -> String {
        let mut output = String::new();
        self.write_terminal(&mut output, None);
        output
    }

    /// Writes the full display text or a prefix ending at the given UTF-8 byte boundary.
    pub fn write_terminal(&self, output: &mut String, prefix: Option<usize>) {
        let mut remaining = prefix.unwrap_or(usize::MAX);
        for span in &self.spans {
            if remaining == 0 {
                break;
            }
            let length = remaining.min(span.text.len());
            let part = span
                .text
                .get(..length)
                .expect("the kept prefix ends on a character boundary");
            let text = inline(part, true, false);
            match &span.url {
                Some(url) if !text.is_empty() => {
                    output.push_str(&markdown_terminal::hyperlink(&text, url));
                }
                _ => output.push_str(&text),
            }
            remaining -= length;
        }
    }
}

#[derive(Debug)]
struct Styled;

impl Object for Styled {}

// A macro can return a string containing one or more links. Only this render's
// registered tokens can turn into spans. Plain text, template literals, and
// strings transformed by filters cannot supply their own link metadata.
#[derive(Default)]
struct Links {
    keys: (RandomState, RandomState),
    spans: HashMap<String, DisplaySpan>,
}

impl Links {
    fn register(&mut self, span: DisplaySpan) -> Result<Value, Error> {
        let index = self.spans.len();
        let (first, second) = &self.keys;
        let mut key = String::with_capacity(TOKEN_BYTES);
        for mut hash in [first.hash_one(index), second.hash_one(index)] {
            for _ in 0..13 {
                let symbol = u32::try_from(hash & 31).expect("base-32 digit fits in u32");
                key.push(char::from_u32(0x80 + symbol).expect("C1 control is a valid character"));
                hash >>= 5;
            }
        }
        let token = format!("{TOKEN_START}{key}{TOKEN_END}");
        let Entry::Vacant(entry) = self.spans.entry(key) else {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "could not register hyperlink",
            ));
        };
        entry.insert(span);
        Ok(Value::from_safe_string(token))
    }
}

pub(super) fn render(
    template: &Template<'_, '_>,
    context: &impl Serialize,
    styled: bool,
) -> Result<DisplayText, Error> {
    let root = Value::from(Serde(context));
    let root = if styled {
        // A NUL is outside MiniJinja's identifier syntax. The private render
        // setting is available to the helper, including from lexical macros.
        merge_maps([
            root,
            Value::from(BTreeMap::from([(STYLED_KEY, Value::from_object(Styled))])),
        ])
    } else {
        root
    };
    let rendered = template.render_captured(root)?;
    expand(rendered.output(), rendered.state().get_extension::<Links>())
}

pub(super) fn format_value(
    out: &mut Output,
    state: &mut State,
    value: &Value,
) -> Result<(), Error> {
    let custom_escape = matches!(state.auto_escape(), AutoEscape::Custom(_));
    if value.is_safe() {
        return if custom_escape {
            write!(out, "{value}").map_err(Error::from)
        } else {
            minijinja::escape_formatter(out, state, value)
        };
    }

    let text = value
        .as_str()
        .map_or_else(|| Cow::Owned(value.to_string()), Cow::Borrowed);
    let text = inline(&text, false, true);
    if custom_escape {
        out.write_str(&text).map_err(Error::from)
    } else {
        minijinja::escape_formatter(out, state, &Value::from(text))
    }
}

fn inline(text: &str, tabs: bool, pending_tokens: bool) -> Cow<'_, str> {
    let safe = |ch: char| {
        if ch.is_control() {
            (tabs && ch == '\t')
                || (pending_tokens && matches!(ch, TOKEN_START | TOKEN_END | '\u{80}'..='\u{9f}'))
        } else {
            !matches!(ch, '\u{2028}' | '\u{2029}')
        }
    };
    if text.chars().all(safe) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.replace("\r\n", "\n")
            .chars()
            .map(|ch| {
                if safe(ch) {
                    ch
                } else if matches!(ch, '\n' | '\r' | '\t' | '\u{2028}' | '\u{2029}') {
                    ' '
                } else {
                    '\u{fffd}'
                }
            })
            .collect(),
    )
}

pub(super) fn hyperlink(state: &mut State, args: &[Value]) -> Result<Value, Error> {
    let (url, text, fallback) = match args {
        [url, text] => (url, text, None),
        [url, text, fallback] => (url, text, Some(fallback)),
        _ => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "expected hyperlink(url, text[, fallback])",
            ));
        }
    };
    let url = string_arg(url, "url")?;
    let text = string_arg(text, "text")?;
    let fallback = fallback
        .map(|value| string_arg(value, "fallback"))
        .transpose()?;

    let styled = state
        .lookup(STYLED_KEY)
        .is_some_and(|value| value.downcast_object_ref::<Styled>().is_some());
    let visible = if styled {
        text
    } else {
        fallback.unwrap_or(text)
    };
    let work = url.len().saturating_add(visible.len());
    state.consume_fuel(u64::try_from(work / 16 + 1).expect("byte count fits in u64"))?;
    if !valid_url(url) {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "hyperlink url must be a valid absolute URL without spaces or control characters",
        ));
    }

    let visible = if let Some(links) = state
        .get_extension::<Links>()
        .filter(|_| visible.contains(TOKEN_START))
    {
        expand(visible, Some(links))?.plain()
    } else {
        visible.to_owned()
    };
    let text = inline(&visible, false, false).into_owned();
    if !styled || text.is_empty() {
        return Ok(Value::from(text));
    }
    state
        .get_or_insert_extension_with(Links::default)
        .register(DisplaySpan {
            text,
            url: Some(url.to_owned()),
        })
}

fn string_arg<'a>(value: &'a Value, arg: &str) -> Result<&'a str, Error> {
    value.as_str().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("hyperlink {arg} must be a string"),
        )
    })
}

fn valid_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    if url
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_control() || "\"<>\\^`{|}".contains(ch))
    {
        return false;
    }
    // The URL parser will turn malformed HTTP links such as `https:/host`
    // into ordinary URLs. Require an explicit authority for those schemes.
    if (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        && !rest.strip_prefix("//").is_some_and(|rest| {
            rest.split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| !authority.is_empty() && !authority.ends_with(':'))
        })
    {
        return false;
    }
    let mut parts = url.split('%');
    parts.next();
    if parts.any(|part| {
        !part
            .get(..2)
            .is_some_and(|pair| pair.bytes().all(|b| b.is_ascii_hexdigit()))
    }) {
        return false;
    }
    reqwest::Url::parse(url).is_ok()
}

fn expand(output: &str, links: Option<&Links>) -> Result<DisplayText, Error> {
    if output.len() > MAX_DISPLAY_BYTES {
        return Err(display_limit());
    }

    let mut result = DisplayText::default();
    let mut size = 0;
    let mut output = output;
    while let Some(links) = links {
        let Some((plain, rest)) = output.split_once(TOKEN_START) else {
            break;
        };
        push_text(&mut result, &mut size, plain)?;
        if let Some((span, after)) = token_span(rest, links) {
            // Include each URL and the OSC delimiters in the budget. A macro
            // can repeat a compact token with a much larger linked span.
            let url_bytes = span.url.as_ref().map_or(0, String::len);
            charge_size(
                &mut size,
                span.text.len().saturating_add(url_bytes).saturating_add(16),
            )?;
            result.spans.push(span.clone());
            output = after;
        } else {
            push_text(&mut result, &mut size, "\u{fffd}")?;
            output = rest;
        }
    }
    push_text(&mut result, &mut size, output)?;
    Ok(result)
}

fn token_span<'a, 'b>(input: &'a str, links: &'b Links) -> Option<(&'b DisplaySpan, &'a str)> {
    let token = input.get(..TOKEN_BYTES)?;
    let after = input.get(TOKEN_BYTES..)?.strip_prefix(TOKEN_END)?;
    let span = links.spans.get(token)?;
    Some((span, after))
}

fn push_text(result: &mut DisplayText, size: &mut usize, text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Ok(());
    }
    let text = inline(text, true, false);
    charge_size(size, text.len())?;
    if let Some(last) = result.spans.last_mut().filter(|span| span.url.is_none()) {
        last.text.push_str(&text);
    } else {
        result.spans.push(DisplaySpan {
            text: text.into_owned(),
            url: None,
        });
    }
    Ok(())
}

fn charge_size(size: &mut usize, bytes: usize) -> Result<(), Error> {
    *size = size.saturating_add(bytes);
    if *size > MAX_DISPLAY_BYTES {
        return Err(display_limit());
    }
    Ok(())
}

fn display_limit() -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        "display template exceeded its output size limit",
    )
}

pub(super) fn indent(state: &State, args: Rest<ValueOrKwargs>) -> Result<Value, Error> {
    let args = args.into_values();
    let (value, width, first, blank, kwargs): (
        StringInput<'_>,
        Option<usize>,
        Option<bool>,
        Option<bool>,
        Kwargs,
    ) = from_args(&args)?;
    let columns = match width {
        Some(width) => width,
        None => kwargs.peek::<Option<usize>>("width")?.unwrap_or(4),
    };
    let lines = value.as_str().bytes().filter(|&b| b == b'\n').count() + 1;
    let bytes = columns.saturating_mul(lines);
    state.consume_fuel(u64::try_from(bytes / 64 + 1).expect("byte count fits in u64"))?;
    minijinja::filters::indent(value, width, first, blank, kwargs)
}
