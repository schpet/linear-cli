//! GraphQL value literals for default values in `linear schema` output:
//! parsing, coercion and printing. Adapted from graphql-js 16.13.2
//! (MIT, GraphQL Contributors; see rust/licenses/graphql-js-MIT.txt).
use crate::{
    error::AppError,
    graphql::schema_introspection::{Kind, Model, TypeRef, name, print_string, shape},
};
use serde_json::{Map, Number, Value};
use std::collections::BTreeSet;

const MAX_DEFAULT_DEPTH: usize = 128;
fn check_depth(depth: usize) -> Result<(), AppError> {
    if depth > MAX_DEFAULT_DEPTH {
        Err(shape("Schema default nesting exceeds 128 levels"))
    } else {
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(String),
    Float(String),
    String(String),
    Enum(String),
    Variable,
    List(Vec<Literal>),
    Object(Vec<(String, Literal)>),
}
struct Lexer<'a> {
    rest: &'a str,
}
impl Lexer<'_> {
    fn skip(&mut self) {
        loop {
            self.rest = self
                .rest
                .trim_start_matches([' ', '\t', '\n', '\r', ',', '\u{feff}']);
            if self.rest.starts_with('#') {
                self.rest = self
                    .rest
                    .find(['\r', '\n'])
                    .and_then(|i| self.rest.get(i..))
                    .unwrap_or("");
            } else {
                break;
            }
        }
    }
    fn take(&mut self, c: char) -> bool {
        self.skip();
        if let Some(rest) = self.rest.strip_prefix(c) {
            self.rest = rest;
            true
        } else {
            false
        }
    }
    fn required(&mut self, c: char) -> Result<(), AppError> {
        if self.take(c) {
            Ok(())
        } else {
            Err(shape(format!("Syntax Error: Expected {c}.")))
        }
    }
    fn named(&mut self) -> Result<String, AppError> {
        self.skip();
        let n = self
            .rest
            .bytes()
            .take_while(|c| *c == b'_' || c.is_ascii_alphanumeric())
            .count();
        let text = self
            .rest
            .get(..n)
            .ok_or_else(|| shape("Syntax Error: Invalid name"))?;
        name(text).map_err(|_| shape("Syntax Error: Expected Name."))?;
        let result = text.to_owned();
        self.rest = self
            .rest
            .get(n..)
            .ok_or_else(|| shape("Invalid name offset"))?;
        Ok(result)
    }
    fn value(&mut self, depth: usize) -> Result<Literal, AppError> {
        check_depth(depth)?;
        self.skip();
        if self.take('$') {
            self.named()?;
            return Ok(Literal::Variable);
        }
        if self.take('[') {
            let mut values = Vec::new();
            while !self.take(']') {
                values.push(self.value(depth + 1)?);
            }
            return Ok(Literal::List(values));
        }
        if self.take('{') {
            let mut values = Vec::new();
            while !self.take('}') {
                let key = self.named()?;
                self.required(':')?;
                values.push((key, self.value(depth + 1)?));
            }
            return Ok(Literal::Object(values));
        }
        if self.rest.starts_with('"') {
            return self.string().map(Literal::String);
        }
        if self.rest.starts_with('-')
            || self.rest.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            return self.number();
        }
        match self.named()?.as_str() {
            "null" => Ok(Literal::Null),
            "true" => Ok(Literal::Bool(true)),
            "false" => Ok(Literal::Bool(false)),
            name => Ok(Literal::Enum(name.to_owned())),
        }
    }
    fn number(&mut self) -> Result<Literal, AppError> {
        let start = self.rest;
        let negative = self.rest.starts_with('-');
        if negative {
            self.rest = self
                .rest
                .strip_prefix('-')
                .ok_or_else(|| shape("Invalid numeric sign"))?;
        }
        let digits = self.rest.bytes().take_while(u8::is_ascii_digit).count();
        let integral = self
            .rest
            .get(..digits)
            .ok_or_else(|| shape("Invalid numeric offset"))?;
        if digits == 0 || (digits > 1 && integral.starts_with('0')) {
            return Err(shape("Syntax Error: Invalid number."));
        }
        self.rest = self
            .rest
            .get(digits..)
            .ok_or_else(|| shape("Invalid numeric offset"))?;
        let mut float = false;
        if self.rest.starts_with('.') {
            float = true;
            self.rest = self
                .rest
                .strip_prefix('.')
                .ok_or_else(|| shape("Invalid decimal"))?;
            self.digits()?;
        }
        if self.rest.starts_with(['e', 'E']) {
            float = true;
            self.rest = self
                .rest
                .get(1..)
                .ok_or_else(|| shape("Invalid exponent"))?;
            if self.rest.starts_with(['+', '-']) {
                self.rest = self
                    .rest
                    .get(1..)
                    .ok_or_else(|| shape("Invalid exponent sign"))?;
            }
            self.digits()?;
        }
        if self
            .rest
            .chars()
            .next()
            .is_some_and(|c| c == '.' || c == '_' || c.is_ascii_alphabetic())
        {
            return Err(shape("Syntax Error: Invalid number."));
        }
        let n = start.len() - self.rest.len();
        let raw = start
            .get(..n)
            .ok_or_else(|| shape("Invalid numeric offset"))?
            .to_owned();
        Ok(if float {
            Literal::Float(raw)
        } else {
            Literal::Int(raw)
        })
    }
    fn digits(&mut self) -> Result<(), AppError> {
        let n = self.rest.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            return Err(shape("Syntax Error: Invalid number."));
        }
        self.rest = self
            .rest
            .get(n..)
            .ok_or_else(|| shape("Invalid number offset"))?;
        Ok(())
    }
    fn char(&mut self) -> Result<char, AppError> {
        let c = self
            .rest
            .chars()
            .next()
            .ok_or_else(|| shape("Syntax Error: Unterminated string."))?;
        self.rest = self
            .rest
            .get(c.len_utf8()..)
            .ok_or_else(|| shape("Invalid string offset"))?;
        Ok(c)
    }
    fn hex(&mut self, count: usize) -> Result<u32, AppError> {
        let s = self
            .rest
            .get(..count)
            .ok_or_else(|| shape("Syntax Error: Invalid Unicode escape."))?;
        if !s.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(shape("Syntax Error: Invalid Unicode escape."));
        }
        let n = u32::from_str_radix(s, 16)
            .map_err(|_| shape("Syntax Error: Invalid Unicode escape."))?;
        self.rest = self
            .rest
            .get(count..)
            .ok_or_else(|| shape("Invalid escape offset"))?;
        Ok(n)
    }
    fn string(&mut self) -> Result<String, AppError> {
        if let Some(rest) = self.rest.strip_prefix("\"\"\"") {
            self.rest = rest;
            let mut value = String::new();
            loop {
                if let Some(rest) = self.rest.strip_prefix("\\\"\"\"") {
                    value.push_str("\"\"\"");
                    self.rest = rest;
                    continue;
                }
                if let Some(rest) = self.rest.strip_prefix("\"\"\"") {
                    self.rest = rest;
                    return Ok(dedent(&value));
                }
                let c = self.char()?;
                value.push(c);
            }
        }
        self.required('"')?;
        let mut value = String::new();
        loop {
            let c = self.char()?;
            match c {
                '"' => return Ok(value),
                '\r' | '\n' => return Err(shape("Syntax Error: Unterminated string.")),
                '\\' => {
                    let c = self.char()?;
                    match c {
                        '"' | '\\' | '/' => value.push(c),
                        'b' => value.push('\u{8}'),
                        'f' => value.push('\u{c}'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        'u' => {
                            let n = if let Some(rest) = self.rest.strip_prefix('{') {
                                self.rest = rest;
                                let n = self.rest.bytes().take_while(u8::is_ascii_hexdigit).count();
                                if n == 0 || n > 8 {
                                    return Err(shape("Syntax Error: Invalid Unicode escape."));
                                }
                                let number = self.hex(n)?;
                                if !self.rest.starts_with('}') {
                                    return Err(shape("Syntax Error: Invalid Unicode escape."));
                                }
                                self.rest = self
                                    .rest
                                    .strip_prefix('}')
                                    .ok_or_else(|| shape("Invalid Unicode escape"))?;
                                number
                            } else {
                                let high = self.hex(4)?;
                                if (0xd800..=0xdbff).contains(&high) {
                                    self.rest = self.rest.strip_prefix("\\u").ok_or_else(|| {
                                        shape("Syntax Error: Invalid Unicode escape.")
                                    })?;
                                    let low = self.hex(4)?;
                                    if !(0xdc00..=0xdfff).contains(&low) {
                                        return Err(shape("Syntax Error: Invalid Unicode escape."));
                                    }
                                    0x10000 + (high - 0xd800) * 0x400 + low - 0xdc00
                                } else {
                                    high
                                }
                            };
                            value.push(
                                char::from_u32(n).ok_or_else(|| {
                                    shape("Syntax Error: Invalid Unicode escape.")
                                })?,
                            );
                        }
                        _ => return Err(shape("Syntax Error: Invalid character escape sequence.")),
                    }
                }
                _ => value.push(c),
            }
        }
    }
}
fn dedent(raw: &str) -> String {
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");
    let lines = text.split('\n').collect::<Vec<_>>();
    let mut indent = usize::MAX;
    let mut first = None;
    let mut last = 0;
    for (i, line) in lines.iter().enumerate() {
        let n = line
            .bytes()
            .take_while(|c| matches!(c, b' ' | b'\t'))
            .count();
        if n == line.len() {
            continue;
        }
        first.get_or_insert(i);
        last = i;
        if i != 0 {
            indent = indent.min(n);
        }
    }
    let Some(first) = first else {
        return String::new();
    };
    lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i >= first && *i <= last)
        .map(|(i, line)| {
            if i == 0 {
                (*line).to_owned()
            } else {
                line.get(indent.min(line.len())..).unwrap_or("").to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn parse(text: &str) -> Result<Literal, AppError> {
    let mut lexer = Lexer { rest: text };
    let value = lexer.value(0)?;
    lexer.skip();
    if !lexer.rest.is_empty() {
        return Err(shape("Syntax Error: Expected <EOF>."));
    }
    Ok(value)
}
/// An integer literal stays an integer; any other number must be a finite float.
fn number(text: &str) -> Option<Value> {
    text.parse::<i64>().map(Value::from).ok().or_else(|| {
        text.parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number)
    })
}
fn untyped(value: &Literal, depth: usize) -> Result<Option<Value>, AppError> {
    check_depth(depth)?;
    Ok(match value {
        Literal::Null => Some(Value::Null),
        Literal::Bool(v) => Some(Value::Bool(*v)),
        Literal::Int(v) | Literal::Float(v) => number(v),
        Literal::Enum(v) | Literal::String(v) => Some(Value::String(v.clone())),
        Literal::Variable => None,
        Literal::List(v) => Some(Value::Array(
            v.iter()
                .map(|v| Ok(untyped(v, depth + 1)?.unwrap_or(Value::Null)))
                .collect::<Result<_, AppError>>()?,
        )),
        Literal::Object(v) => {
            let mut fields = Vec::new();
            for (key, value) in v {
                if let Some(value) = untyped(value, depth + 1)? {
                    fields.push((key.clone(), value));
                }
            }
            Some(Value::Object(fields.into_iter().collect::<Map<_, _>>()))
        }
    })
}
fn coerce(
    model: &Model,
    r: &TypeRef,
    literal: &Literal,
    depth: usize,
    defaults: &mut BTreeSet<(String, String)>,
) -> Result<Option<Value>, AppError> {
    check_depth(depth)?;
    if matches!(literal, Literal::Variable) {
        return Ok(None);
    }
    if r.kind == Kind::NonNull {
        if matches!(literal, Literal::Null) {
            return Ok(None);
        }
        return coerce(model, r.inner()?, literal, depth + 1, defaults);
    }
    if matches!(literal, Literal::Null) {
        return Ok(Some(Value::Null));
    }
    if r.kind == Kind::List {
        let inner = r.inner()?;
        let list = match literal {
            Literal::List(list) => list.clone(),
            value => vec![value.clone()],
        };
        let mut values = Vec::new();
        for v in list {
            let value = if matches!(v, Literal::Variable) && inner.kind != Kind::NonNull {
                Some(Value::Null)
            } else {
                coerce(model, inner, &v, depth + 1, defaults)?
            };
            let Some(value) = value else {
                return Ok(None);
            };
            values.push(value);
        }
        return Ok(Some(Value::Array(values)));
    }
    let t = model.named(r)?;
    if t.kind == Kind::InputObject {
        let Literal::Object(fields) = literal else {
            return Ok(None);
        };
        let fields = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut values = Vec::new();
        for field in model.sorted_inputs(
            t.input_fields
                .as_deref()
                .ok_or_else(|| shape("Missing inputFields"))?,
        ) {
            let v = match fields
                .get(field.name.as_str())
                .filter(|v| !matches!(v, Literal::Variable))
            {
                Some(v) => coerce(model, &field.r#type, v, depth + 1, defaults)?,
                None => match &field.default_value {
                    Some(v) => {
                        let token = (t.name.clone(), field.name.clone());
                        if !defaults.insert(token.clone()) {
                            return Err(shape(format!(
                                "Cyclic input-object default: {}.{}",
                                token.0, token.1
                            )));
                        }
                        let expanded = parse(v).and_then(|value| {
                            coerce(model, &field.r#type, &value, depth + 1, defaults)
                        });
                        assert!(
                            defaults.remove(&token),
                            "active default token is retained until expansion ends"
                        );
                        expanded?
                    }
                    None => None,
                },
            };
            match v {
                Some(v) => values.push((field.name.clone(), v)),
                None if field.r#type.kind == Kind::NonNull => return Ok(None),
                None => {}
            }
            if fields
                .get(field.name.as_str())
                .is_some_and(|v| !matches!(v, Literal::Variable))
                && values.last().is_none_or(|(k, _)| k != &field.name)
            {
                return Ok(None);
            }
        }
        return Ok(Some(Value::Object(
            values.into_iter().collect::<Map<_, _>>(),
        )));
    }
    let value = match t.name.as_str() {
        "Int" => match literal {
            Literal::Int(v) => v.parse::<i32>().ok().map(Value::from),
            _ => None,
        },
        "Float" => match literal {
            Literal::Int(v) | Literal::Float(v) => number(v),
            _ => None,
        },
        "String" => match literal {
            Literal::String(v) => Some(Value::String(v.clone())),
            _ => None,
        },
        "Boolean" => match literal {
            Literal::Bool(v) => Some(Value::Bool(*v)),
            _ => None,
        },
        "ID" => match literal {
            Literal::String(v) | Literal::Int(v) => Some(Value::String(v.clone())),
            _ => None,
        },
        _ if t.kind == Kind::Enum => match literal {
            Literal::Enum(v)
                if t.enum_values
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .any(|e| e.name == *v) =>
            {
                Some(Value::String(v.clone()))
            }
            _ => None,
        },
        _ if t.kind == Kind::Scalar => untyped(literal, depth)?,
        _ => return Err(shape("Unexpected input type")),
    };
    Ok(value)
}
fn integer(text: &str) -> bool {
    let raw = text.strip_prefix('-').unwrap_or(text);
    raw == "0"
        || (!raw.starts_with('0') && !raw.is_empty() && raw.bytes().all(|c| c.is_ascii_digit()))
}
fn from_value(
    model: &Model,
    r: &TypeRef,
    value: &Value,
    depth: usize,
) -> Result<Option<String>, AppError> {
    check_depth(depth)?;
    if r.kind == Kind::NonNull {
        if value == &Value::Null {
            return Ok(None);
        }
        return from_value(model, r.inner()?, value, depth + 1);
    }
    if value == &Value::Null {
        return Ok(Some("null".into()));
    }
    if r.kind == Kind::List {
        let inner = r.inner()?;
        if let Value::Array(values) = value {
            let mut out = Vec::new();
            for v in values {
                if let Some(v) = from_value(model, inner, v, depth + 1)? {
                    out.push(v);
                }
            }
            return Ok(Some(format!("[{}]", out.join(", "))));
        }
        return from_value(model, inner, value, depth + 1);
    }
    let t = model.named(r)?;
    if t.kind == Kind::InputObject {
        let Value::Object(object) = value else {
            return Ok(None);
        };
        let mut fields = Vec::new();
        for f in model.sorted_inputs(
            t.input_fields
                .as_deref()
                .ok_or_else(|| shape("Missing inputFields"))?,
        ) {
            if let Some(value) = object.get(&f.name)
                && let Some(value) = from_value(model, &f.r#type, value, depth + 1)?
            {
                fields.push(format!("{}: {value}", f.name));
            }
        }
        return Ok(Some(format!("{{{}}}", fields.join(", "))));
    }
    match value {
        Value::Null => Ok(Some("null".into())),
        Value::Bool(v) => Ok(Some(v.to_string())),
        Value::Number(v) => Ok(Some(v.to_string())),
        Value::String(v) => Ok(Some(
            if t.kind == Kind::Enum || (t.name == "ID" && integer(v)) {
                v.clone()
            } else {
                print_string(v)
            },
        )),
        Value::Array(_) | Value::Object(_) => Err(shape("Cannot convert value to AST.")),
    }
}
pub fn render(model: &Model, r: &TypeRef, value: &Literal) -> Result<Option<String>, AppError> {
    match coerce(model, r, value, 0, &mut BTreeSet::new())? {
        Some(v) => from_value(model, r, &v, 0),
        None => Ok(None),
    }
}
