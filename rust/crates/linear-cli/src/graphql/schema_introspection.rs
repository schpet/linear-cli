//! Builds, sorts and prints a schema from an introspection result for
//! `linear schema`. Adapted from graphql-js 16.13.2 (MIT, GraphQL
//! Contributors; see rust/licenses/graphql-js-MIT.txt).
use crate::{
    error::{AppError, AppErrorKind},
    graphql::schema_defaults,
    js_value::{JsValue, js_stringify},
};
use serde::Deserialize;
use std::{cmp::Ordering, collections::BTreeMap};
pub const QUERY: &str = include_str!("introspection.graphql");
pub fn shape(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Validation, message)
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Kind {
    Scalar,
    Object,
    Interface,
    Union,
    Enum,
    InputObject,
    List,
    NonNull,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeRef {
    pub kind: Kind,
    pub name: Option<String>,
    pub of_type: Option<Box<TypeRef>>,
}
impl TypeRef {
    pub fn inner(&self) -> Result<&TypeRef, AppError> {
        self.of_type
            .as_deref()
            .ok_or_else(|| shape("Decorated type deeper than introspection query."))
    }
    pub fn render(&self) -> Result<String, AppError> {
        match self.kind {
            Kind::List => Ok(format!("[{}]", self.inner()?.render()?)),
            Kind::NonNull => Ok(format!("{}!", self.inner()?.render()?)),
            _ => self
                .name
                .clone()
                .ok_or_else(|| shape("Unknown type reference")),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputValue {
    pub name: String,
    pub description: Option<String>,
    pub r#type: TypeRef,
    pub default_value: Option<String>,
    pub deprecation_reason: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub description: Option<String>,
    pub args: Vec<InputValue>,
    pub r#type: TypeRef,
    pub deprecation_reason: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumValue {
    pub name: String,
    pub description: Option<String>,
    pub deprecation_reason: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDef {
    pub kind: Kind,
    pub name: String,
    pub description: Option<String>,
    pub fields: Option<Vec<Field>>,
    pub input_fields: Option<Vec<InputValue>>,
    pub interfaces: Option<Vec<TypeRef>>,
    pub enum_values: Option<Vec<EnumValue>>,
    pub possible_types: Option<Vec<TypeRef>>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Directive {
    pub name: String,
    pub description: Option<String>,
    pub locations: Vec<String>,
    pub args: Vec<InputValue>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Named {
    pub name: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    pub query_type: Option<Named>,
    pub mutation_type: Option<Named>,
    pub subscription_type: Option<Named>,
    pub types: Vec<TypeDef>,
    pub directives: Option<Vec<Directive>>,
    pub description: Option<String>,
}
#[derive(Deserialize)]
struct Introspection {
    __schema: Schema,
}
pub struct Model {
    pub schema: Schema,
    pub types: BTreeMap<String, TypeDef>,
}
pub fn natural(a: &str, b: &str) -> Ordering {
    let (a, b) = (
        a.encode_utf16().collect::<Vec<_>>(),
        b.encode_utf16().collect::<Vec<_>>(),
    );
    let (mut i, mut j) = (0, 0);
    while let (Some(x), Some(y)) = (a.get(i), b.get(j)) {
        if (48..=57).contains(x) && (48..=57).contains(y) {
            let (mut an, mut bn) = (0.0, 0.0);
            loop {
                if let Some(x) = a.get(i) {
                    an = an * 10.0 + f64::from(*x) - 48.0;
                    i += 1;
                }
                if !(an > 0.0 && a.get(i).is_some_and(|x| (48..=57).contains(x))) {
                    break;
                }
            }
            loop {
                if let Some(y) = b.get(j) {
                    bn = bn * 10.0 + f64::from(*y) - 48.0;
                    j += 1;
                }
                if !(bn > 0.0 && b.get(j).is_some_and(|y| (48..=57).contains(y))) {
                    break;
                }
            }
            if an < bn {
                return Ordering::Less;
            }
            if an > bn {
                return Ordering::Greater;
            }
        } else {
            let result = x.cmp(y);
            if result != Ordering::Equal {
                return result;
            }
            i += 1;
            j += 1;
        }
    }
    a.len().cmp(&b.len())
}
pub fn name(name: &str) -> Result<(), AppError> {
    let mut chars = name.bytes();
    if !chars
        .next()
        .is_some_and(|c| c == b'_' || c.is_ascii_alphabetic())
        || !chars.all(|c| c == b'_' || c.is_ascii_alphanumeric())
    {
        return Err(shape(format!(
            "Names must only contain [_a-zA-Z0-9] and begin with [_a-zA-Z]: {name}"
        )));
    }
    Ok(())
}
fn sorted_dedup<T>(values: &[T], get: impl Fn(&T) -> &str) -> Vec<&T> {
    let mut map = BTreeMap::new();
    for v in values {
        map.insert(get(v), v);
    }
    let mut v: Vec<_> = map.into_values().collect();
    v.sort_by(|a, b| natural(get(a), get(b)));
    v
}
impl Model {
    pub fn parse(value: &JsValue) -> Result<Self, AppError> {
        let data: Introspection = serde_json::from_str(&js_stringify(value)).map_err(|e| {
            shape(format!("Invalid or incomplete introspection result: {e}")).with_source(e)
        })?;
        let mut types = BTreeMap::new();
        for t in &data.__schema.types {
            name(&t.name)?;
            if matches!(t.kind, Kind::List | Kind::NonNull) {
                return Err(shape(
                    "Invalid or incomplete introspection result: wrapped named type",
                ));
            }
            types.insert(t.name.clone(), t.clone());
        }
        // Standard types from the response are replaced with the canonical
        // builtins. This is the standard meta-schema only, never application SDL.
        let builtins: Vec<TypeDef> =
            serde_json::from_str(include_str!("schema_builtin_types.json")).map_err(|e| {
                AppError::new(
                    crate::error::AppErrorKind::Invariant,
                    "Invalid compiled GraphQL standard types",
                )
                .with_source(e)
            })?;
        for builtin in builtins {
            if types.contains_key(&builtin.name) {
                types.insert(builtin.name.clone(), builtin);
            }
        }
        let model = Self {
            schema: data.__schema,
            types,
        };
        model.validate()?;
        Ok(model)
    }
    pub fn named(&self, reference: &TypeRef) -> Result<&TypeDef, AppError> {
        let name = reference
            .name
            .as_deref()
            .ok_or_else(|| shape("Unknown type reference"))?;
        self.types.get(name).ok_or_else(||shape(format!("Invalid or incomplete schema, unknown type: {name}. Ensure that a full introspection query is used in order to build a client schema.")))
    }
    fn reference(&self, r: &TypeRef, input: bool) -> Result<(), AppError> {
        match r.kind {
            Kind::List => self.reference(r.inner()?, input),
            Kind::NonNull => {
                let inner = r.inner()?;
                if inner.kind == Kind::NonNull {
                    return Err(shape("Expected nullable type"));
                }
                self.reference(inner, input)
            }
            _ => {
                let t = self.named(r)?;
                let allowed = if input {
                    matches!(t.kind, Kind::Scalar | Kind::Enum | Kind::InputObject)
                } else {
                    matches!(
                        t.kind,
                        Kind::Scalar | Kind::Object | Kind::Interface | Kind::Union | Kind::Enum
                    )
                };
                if !allowed {
                    return Err(shape(format!(
                        "Introspection must provide {} type, received {}",
                        if input { "input" } else { "output" },
                        t.name
                    )));
                }
                Ok(())
            }
        }
    }
    fn inputs(&self, values: &[InputValue]) -> Result<(), AppError> {
        for v in values {
            name(&v.name)?;
            self.reference(&v.r#type, true)?;
            if let Some(default) = &v.default_value {
                schema_defaults::parse(default)?;
            }
        }
        Ok(())
    }
    fn validate(&self) -> Result<(), AppError> {
        for root in [
            &self.schema.query_type,
            &self.schema.mutation_type,
            &self.schema.subscription_type,
        ]
        .into_iter()
        .flatten()
        {
            if self
                .types
                .get(&root.name)
                .is_none_or(|t| t.kind != Kind::Object)
            {
                return Err(shape(format!(
                    "Expected {} to be a GraphQL Object type.",
                    root.name
                )));
            }
        }
        for t in self.types.values() {
            if specified_type(&t.name) {
                continue;
            }
            match t.kind {
                Kind::Scalar => {}
                Kind::Object | Kind::Interface => {
                    let interfaces = match (&t.interfaces, t.kind) {
                        (Some(v), _) => v.as_slice(),
                        (None, Kind::Interface) => &[],
                        _ => return Err(shape("Introspection result missing interfaces")),
                    };
                    for r in interfaces {
                        if self.named(r)?.kind != Kind::Interface {
                            return Err(shape("Expected GraphQL interface type"));
                        }
                    }
                    for f in t
                        .fields
                        .as_deref()
                        .ok_or_else(|| shape("Introspection result missing fields"))?
                    {
                        name(&f.name)?;
                        self.reference(&f.r#type, false)?;
                        self.inputs(&f.args)?;
                    }
                }
                Kind::Union => {
                    for r in t
                        .possible_types
                        .as_deref()
                        .ok_or_else(|| shape("Introspection result missing possibleTypes"))?
                    {
                        if self.named(r)?.kind != Kind::Object {
                            return Err(shape("Expected GraphQL object type"));
                        }
                    }
                }
                Kind::Enum => {
                    for v in t
                        .enum_values
                        .as_deref()
                        .ok_or_else(|| shape("Introspection result missing enumValues"))?
                    {
                        name(&v.name)?;
                        if matches!(v.name.as_str(), "true" | "false" | "null") {
                            return Err(shape(format!("Enum values cannot be named: {}", v.name)));
                        }
                    }
                }
                Kind::InputObject => self.inputs(
                    t.input_fields
                        .as_deref()
                        .ok_or_else(|| shape("Introspection result missing inputFields"))?,
                )?,
                Kind::List | Kind::NonNull => return Err(shape("Unexpected named wrapper type")),
            }
        }
        for d in self.schema.directives.as_deref().unwrap_or(&[]) {
            name(&d.name)?;
            self.inputs(&d.args)?;
            for location in &d.locations {
                if !matches!(
                    location.as_str(),
                    "QUERY"
                        | "MUTATION"
                        | "SUBSCRIPTION"
                        | "FIELD"
                        | "FRAGMENT_DEFINITION"
                        | "FRAGMENT_SPREAD"
                        | "INLINE_FRAGMENT"
                        | "VARIABLE_DEFINITION"
                        | "SCHEMA"
                        | "SCALAR"
                        | "OBJECT"
                        | "FIELD_DEFINITION"
                        | "ARGUMENT_DEFINITION"
                        | "INTERFACE"
                        | "UNION"
                        | "ENUM"
                        | "ENUM_VALUE"
                        | "INPUT_OBJECT"
                        | "INPUT_FIELD_DEFINITION"
                ) {
                    return Err(shape(format!("Invalid directive location {location}")));
                }
            }
        }
        Ok(())
    }
    pub fn sorted_inputs<'a>(&self, v: &'a [InputValue]) -> Vec<&'a InputValue> {
        sorted_dedup(v, |i| &i.name)
    }
    pub fn input(&self, v: &InputValue) -> Result<String, AppError> {
        let mut text = format!("{}: {}", v.name, v.r#type.render()?);
        if let Some(raw) = &v.default_value
            && let Some(default) =
                schema_defaults::render(self, &v.r#type, &schema_defaults::parse(raw)?)?
        {
            text.push_str(&format!(" = {default}"));
        }
        text.push_str(&deprecated(v.deprecation_reason.as_deref()));
        Ok(text)
    }
    fn args(&self, args: &[InputValue], indent: &str) -> Result<String, AppError> {
        let args = self.sorted_inputs(args);
        if args.is_empty() {
            return Ok(String::new());
        }
        if args
            .iter()
            .all(|a| a.description.as_ref().is_none_or(String::is_empty))
        {
            return Ok(format!(
                "({})",
                args.iter()
                    .map(|a| self.input(a))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ));
        }
        let inside = format!("  {indent}");
        let mut out = Vec::new();
        for (i, a) in args.iter().enumerate() {
            out.push(format!(
                "{}{}{}",
                description(a.description.as_deref(), &inside, i == 0),
                inside,
                self.input(a)?
            ));
        }
        Ok(format!("(\n{}\n{indent})", out.join("\n")))
    }
    pub fn print(&self) -> Result<String, AppError> {
        let mut blocks = Vec::new();
        let roots = [
            ("query", self.schema.query_type.as_ref(), "Query"),
            ("mutation", self.schema.mutation_type.as_ref(), "Mutation"),
            (
                "subscription",
                self.schema.subscription_type.as_ref(),
                "Subscription",
            ),
        ];
        if self.schema.description.is_some()
            || roots
                .iter()
                .any(|(_, r, n)| r.is_some_and(|r| r.name != *n))
        {
            let lines = roots
                .iter()
                .filter_map(|(k, r, _)| r.map(|r| format!("  {k}: {}", r.name)))
                .collect::<Vec<_>>();
            blocks.push(format!(
                "{}schema {{\n{}\n}}",
                description(self.schema.description.as_deref(), "", true),
                lines.join("\n")
            ));
        }
        let mut directives = self
            .schema
            .directives
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .collect::<Vec<_>>();
        directives.sort_by(|a, b| natural(&a.name, &b.name));
        for d in directives {
            if matches!(
                d.name.as_str(),
                "include" | "skip" | "deprecated" | "specifiedBy" | "oneOf"
            ) {
                continue;
            }
            let mut locations = d.locations.clone();
            locations.sort_by(|a, b| natural(a, b));
            blocks.push(format!(
                "{}directive @{}{} on {}",
                description(d.description.as_deref(), "", true),
                d.name,
                self.args(&d.args, "")?,
                locations.join(" | ")
            ));
        }
        let mut types = self.types.values().collect::<Vec<_>>();
        types.sort_by(|a, b| natural(&a.name, &b.name));
        for t in types {
            if specified_type(&t.name) {
                continue;
            }
            let mut out = description(t.description.as_deref(), "", true);
            match t.kind {
                Kind::Scalar => out.push_str(&format!("scalar {}", t.name)),
                Kind::Object | Kind::Interface => {
                    out.push_str(&format!(
                        "{} {}",
                        if t.kind == Kind::Object {
                            "type"
                        } else {
                            "interface"
                        },
                        t.name
                    ));
                    let mut interfaces = t
                        .interfaces
                        .as_deref()
                        .unwrap_or(&[])
                        .iter()
                        .map(|r| self.named(r).map(|t| t.name.clone()))
                        .collect::<Result<Vec<_>, _>>()?;
                    interfaces.sort_by(|a, b| natural(a, b));
                    if !interfaces.is_empty() {
                        out.push_str(&format!(" implements {}", interfaces.join(" & ")));
                    }
                    let fields = sorted_dedup(
                        t.fields.as_deref().ok_or_else(|| shape("Missing fields"))?,
                        |f| &f.name,
                    );
                    let mut lines = Vec::new();
                    for (i, f) in fields.iter().enumerate() {
                        lines.push(format!(
                            "{}  {}{}: {}{}",
                            description(f.description.as_deref(), "  ", i == 0),
                            f.name,
                            self.args(&f.args, "  ")?,
                            f.r#type.render()?,
                            deprecated(f.deprecation_reason.as_deref())
                        ));
                    }
                    out.push_str(&block(lines));
                }
                Kind::Union => {
                    out.push_str(&format!("union {}", t.name));
                    let mut types = t
                        .possible_types
                        .as_deref()
                        .unwrap_or(&[])
                        .iter()
                        .map(|r| self.named(r).map(|t| t.name.clone()))
                        .collect::<Result<Vec<_>, _>>()?;
                    types.sort_by(|a, b| natural(a, b));
                    if !types.is_empty() {
                        out.push_str(&format!(" = {}", types.join(" | ")));
                    }
                }
                Kind::Enum => {
                    out.push_str(&format!("enum {}", t.name));
                    let values = sorted_dedup(
                        t.enum_values
                            .as_deref()
                            .ok_or_else(|| shape("Missing enumValues"))?,
                        |v| &v.name,
                    );
                    out.push_str(&block(
                        values
                            .iter()
                            .enumerate()
                            .map(|(i, v)| {
                                format!(
                                    "{}  {}{}",
                                    description(v.description.as_deref(), "  ", i == 0),
                                    v.name,
                                    deprecated(v.deprecation_reason.as_deref())
                                )
                            })
                            .collect(),
                    ));
                }
                Kind::InputObject => {
                    out.push_str(&format!("input {}", t.name));
                    let fields = self.sorted_inputs(
                        t.input_fields
                            .as_deref()
                            .ok_or_else(|| shape("Missing inputFields"))?,
                    );
                    out.push_str(&block(
                        fields
                            .iter()
                            .enumerate()
                            .map(|(i, v)| {
                                Ok(format!(
                                    "{}  {}",
                                    description(v.description.as_deref(), "  ", i == 0),
                                    self.input(v)?
                                ))
                            })
                            .collect::<Result<Vec<_>, AppError>>()?,
                    ));
                }
                Kind::List | Kind::NonNull => return Err(shape("Unexpected wrapped named type")),
            }
            blocks.push(out);
        }
        Ok(blocks.join("\n\n"))
    }
}
fn specified_type(name: &str) -> bool {
    matches!(name, "String" | "Int" | "Float" | "Boolean" | "ID") || name.starts_with("__")
}
fn block(lines: Vec<String>) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!(" {{\n{}\n}}", lines.join("\n"))
    }
}
pub fn print_string(value: &str) -> String {
    let mut text = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            '\u{8}' => text.push_str("\\b"),
            '\t' => text.push_str("\\t"),
            '\n' => text.push_str("\\n"),
            '\u{c}' => text.push_str("\\f"),
            '\r' => text.push_str("\\r"),
            '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' => {
                text.push_str(&format!("\\u{:04X}", u32::from(c)))
            }
            _ => text.push(c),
        }
    }
    text.push('"');
    text
}
fn deprecated(reason: Option<&str>) -> String {
    match reason {
        None => String::new(),
        Some("No longer supported") => " @deprecated".into(),
        Some(r) => format!(" @deprecated(reason: {})", print_string(r)),
    }
}
fn printable(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let (mut empty, mut indent, mut common, mut seen) = (true, false, true, false);
    for c in value.chars() {
        match c {
            '\u{0}'..='\u{8}' | '\u{b}'..='\u{f}' => return false,
            '\n' => {
                if empty && !seen {
                    return false;
                }
                seen = true;
                empty = true;
                indent = false;
            }
            '\t' | ' ' => indent |= empty,
            _ => {
                common &= indent;
                empty = false;
            }
        }
    }
    !(empty || common && seen)
}
fn block_string(value: &str) -> String {
    let escaped = value.replace("\"\"\"", "\\\"\"\"");
    let single = !value.contains('\n');
    let leading = !single
        && escaped
            .split('\n')
            .skip(1)
            .all(|s| s.is_empty() || s.starts_with([' ', '\t']));
    let triple = escaped.ends_with("\\\"\"\"");
    let trailing = (value.ends_with('"') && !triple) || value.ends_with('\\');
    let multi = !single || value.encode_utf16().count() > 70 || trailing || leading || triple;
    format!(
        "\"\"\"{}{}{}\"\"\"",
        if (multi && !(single && value.starts_with([' ', '\t']))) || leading {
            "\n"
        } else {
            ""
        },
        escaped,
        if multi || trailing { "\n" } else { "" }
    )
}
fn description(value: Option<&str>, indent: &str, first: bool) -> String {
    match value {
        None => String::new(),
        Some(value) => {
            let s = if printable(value) {
                block_string(value)
            } else {
                print_string(value)
            };
            format!(
                "{}{}{}\n",
                if !indent.is_empty() && !first {
                    "\n"
                } else {
                    ""
                },
                indent,
                s.replace('\n', &format!("\n{indent}"))
            )
        }
    }
}
