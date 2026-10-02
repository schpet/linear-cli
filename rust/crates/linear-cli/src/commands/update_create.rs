//! Full project/initiative status-update create with separate attended editor policy.
use crate::{
    commands::{initiative_view::Reference, text_input},
    config::ChildEnvOverlay,
    error::Error,
    graphql::{
        bulk_error,
        envelope::GraphQlRequest,
        operations::{
            initiative_view::{ResolveInitiativeBySlug, UrlSlugVariables},
            update_create::*,
        },
        transport::{GraphQlTransport, classify_typed},
    },
    platform::{
        editor,
        prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession},
        prompt_text::TextOptions,
    },
    refs::is_linear_uuid,
};
use cynic::{MutationBuilder, QueryBuilder};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read, Write};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Project,
    Initiative,
}
impl Mode {
    pub const fn context(self) -> &'static str {
        match self {
            Self::Project => "Failed to create project update",
            Self::Initiative => "Failed to create initiative status update",
        }
    }
    pub const fn opening(self) -> &'static str {
        match self {
            Self::Project => "Opening editor for update content...",
            Self::Initiative => "Opening editor for status update content...",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    OnTrack,
    AtRisk,
    OffTrack,
}
impl Health {
    pub fn parse(value: Option<&str>, mode: Mode) -> Result<Option<Self>, Error> {
        match value.filter(|v| !v.is_empty()) {
            None => Ok(None),
            Some("onTrack") => Ok(Some(Self::OnTrack)),
            Some("atRisk") => Ok(Some(Self::AtRisk)),
            Some("offTrack") => Ok(Some(Self::OffTrack)),
            Some(value) => Err(
                Error::new(format!("Invalid health value: {value}")).with_hint(match mode {
                    Mode::Project => "Must be one of: onTrack, atRisk, offTrack",
                    Mode::Initiative => "Valid values: onTrack, atRisk, offTrack",
                }),
            ),
        }
    }
    fn project(self) -> ProjectHealthInput {
        match self {
            Self::OnTrack => ProjectHealthInput::OnTrack,
            Self::AtRisk => ProjectHealthInput::AtRisk,
            Self::OffTrack => ProjectHealthInput::OffTrack,
        }
    }
    fn initiative(self) -> InitiativeHealthInput {
        match self {
            Self::OnTrack => InitiativeHealthInput::OnTrack,
            Self::AtRisk => InitiativeHealthInput::AtRisk,
            Self::OffTrack => InitiativeHealthInput::OffTrack,
        }
    }
}
#[derive(Default, Debug)]
pub struct Fields {
    pub body: Option<String>,
    pub health: Option<Health>,
}
pub fn attended(
    explicit: bool,
    stdin_tty: bool,
    stdout_tty: bool,
    body: Option<&str>,
    file: Option<&str>,
    health: Option<&str>,
) -> Result<bool, Error> {
    if explicit && !(stdin_tty && stdout_tty) {
        return Err(
            Error::new("Interactive mode requires terminal stdin and stdout").with_hint(
                "Use --body, --body-file, or --health without --interactive when piping.",
            ),
        );
    }
    Ok(explicit
        || (stdin_tty
            && stdout_tty
            && [body, file, health]
                .iter()
                .all(|value| value.is_none_or(str::is_empty))))
}
pub fn file(path: &str, mode: Mode, interactive: bool) -> Result<String, Error> {
    match text_input::read_file(path) {
        Ok(text) => Ok(text),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && (!interactive || mode == Mode::Project) =>
        {
            Err(Error::not_found("File", path))
        }
        Err(error) => Err(Error::new(format!(
            "Failed to read {}file: {error}",
            if interactive { "" } else { "body " }
        ))
        .with_source(error)),
    }
}
pub enum ExchangeFailure {
    Ordinary(Error),
    Shape(Error),
}
impl ExchangeFailure {
    pub fn into_error(self) -> Error {
        match self {
            Self::Ordinary(error) | Self::Shape(error) => error,
        }
    }
}
/// Decode the execution boundary first, then the full operation. Never reissue a request.
async fn exchange<T: DeserializeOwned, V: Serialize>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<V>,
    mutation: Option<&str>,
) -> Result<T, ExchangeFailure> {
    let response = transport
        .send_request(request)
        .await
        .map_err(|e| ExchangeFailure::Ordinary(Error::from(e)))?;
    let observed = bulk_error::observe_source_error(&response, request)
        .map_err(|e| ExchangeFailure::Ordinary(e.into_error()))?;
    if let Some(error) = observed {
        return Err(ExchangeFailure::Ordinary(Error::new(
            error.preferred_message.unwrap_or(error.message),
        )));
    }
    let data: serde_json::Value =
        classify_typed(response).map_err(|e| ExchangeFailure::Ordinary(Error::from(e)))?;
    let confirmed = mutation.is_some_and(|field| {
        data.get(field)
            .and_then(|payload| payload.get("success"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
    });
    serde_json::from_value(data).map_err(|error| {
        ExchangeFailure::Shape(
            Error::new(format!(
                "UPDATE-CREATE-UNEXPECTED-SHAPE: {error}{}",
                match mutation {
                    None => "",
                    Some(_) if confirmed =>
                        "; creation confirmed by success:true; do not retry automatically",
                    Some(_) => "; creation outcome unknown; do not retry automatically",
                }
            ))
            .with_source(error),
        )
    })
}
async fn resolve_text(
    transport: &GraphQlTransport,
    text: &str,
    original: &str,
) -> Result<String, Error> {
    let request =
        GraphQlRequest::with_variables(GetInitiativeBySlugForStatusUpdate::build(SlugVariables {
            slug_id: text.to_owned(),
        }));
    let slug: Result<GetInitiativeBySlugForStatusUpdate, _> =
        exchange(transport, &request, None).await;
    match slug {
        Ok(data) => {
            if let Some(first) = data.initiatives.nodes.into_iter().next() {
                return nonempty(first.id.into_inner(), original);
            }
        }
        Err(ExchangeFailure::Ordinary(_)) => {}
        Err(error @ ExchangeFailure::Shape(_)) => return Err(error.into_error()),
    }
    let request =
        GraphQlRequest::with_variables(GetInitiativeByNameForStatusUpdate::build(NameVariables {
            name: text.to_owned(),
        }));
    let name: Result<GetInitiativeByNameForStatusUpdate, _> =
        exchange(transport, &request, None).await;
    match name {
        Ok(data) => data
            .initiatives
            .nodes
            .into_iter()
            .next()
            .map(|node| nonempty(node.id.into_inner(), original))
            .unwrap_or_else(|| Err(Error::not_found("Initiative", original))),
        Err(ExchangeFailure::Ordinary(_)) => Err(Error::not_found("Initiative", original)),
        Err(error @ ExchangeFailure::Shape(_)) => Err(error.into_error()),
    }
}
fn nonempty(id: String, original: &str) -> Result<String, Error> {
    if id.is_empty() {
        Err(Error::not_found("Initiative", original))
    } else {
        Ok(id)
    }
}
pub async fn initiative_id(
    transport: &GraphQlTransport,
    reference: &Reference,
    original: &str,
) -> Result<String, Error> {
    match reference {
        Reference::Id(id) => Ok(id.clone()),
        Reference::NameOrSlug(text) => resolve_text(transport, text, original).await,
        Reference::UrlSlug(slug) => {
            let request =
                GraphQlRequest::with_variables(ResolveInitiativeBySlug::build(UrlSlugVariables {
                    slug_id: slug.clone(),
                    include_archived: Some(false),
                }));
            let data: ResolveInitiativeBySlug = exchange(transport, &request, None)
                .await
                .map_err(ExchangeFailure::into_error)?;
            let first = data
                .initiatives
                .nodes
                .into_iter()
                .next()
                .ok_or_else(|| Error::not_found("Initiative", original))?;
            let id = nonempty(first.id.into_inner(), original)?;
            if is_linear_uuid(&id) {
                Ok(id)
            } else {
                resolve_text(transport, &id, original).await
            }
        }
    }
}
pub async fn initiative_name(transport: &GraphQlTransport, id: &str, original: &str) -> String {
    let request =
        GraphQlRequest::with_variables(GetInitiativeNameForStatusUpdate::build(IdVariables {
            id: id.to_owned(),
        }));
    let result: Result<GetInitiativeNameForStatusUpdate, _> =
        exchange(transport, &request, None).await;
    match result {
        Ok(data) => data
            .initiative
            .map(|value| value.name)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| original.to_owned()),
        Err(_) => original.to_owned(),
    }
}
pub fn project_request(id: &str, fields: Fields) -> GraphQlRequest<ProjectVariables> {
    GraphQlRequest::with_variables(CreateProjectUpdate::build(ProjectVariables {
        input: ProjectInput {
            project_id: id.to_owned(),
            body: fields.body.filter(|v| !v.is_empty()),
            health: fields.health.map(Health::project),
        },
    }))
}
pub fn initiative_request(id: &str, fields: Fields) -> GraphQlRequest<InitiativeVariables> {
    GraphQlRequest::with_variables(CreateInitiativeUpdate::build(InitiativeVariables {
        input: InitiativeInput {
            initiative_id: id.to_owned(),
            body: fields.body,
            health: fields.health.map(Health::initiative),
        },
    }))
}
pub async fn create(
    transport: &GraphQlTransport,
    id: &str,
    fields: Fields,
    mode: Mode,
) -> Result<Vec<u8>, Error> {
    let (success, name, health, url) = match mode {
        Mode::Project => {
            let data: CreateProjectUpdate = exchange(
                transport,
                &project_request(id, fields),
                Some("projectUpdateCreate"),
            )
            .await
            .map_err(ExchangeFailure::into_error)?;
            let payload = data.project_update_create;
            let update = payload.project_update;
            (
                payload.success,
                update
                    .project
                    .map(|v| v.name)
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| "Unknown project".to_owned()),
                update.health.map(|h| h.as_str().to_owned()),
                update.url,
            )
        }
        Mode::Initiative => {
            let data: CreateInitiativeUpdate = exchange(
                transport,
                &initiative_request(id, fields),
                Some("initiativeUpdateCreate"),
            )
            .await
            .map_err(ExchangeFailure::into_error)?;
            let payload = data.initiative_update_create;
            let update = payload.initiative_update;
            (
                payload.success,
                update
                    .initiative
                    .map(|v| v.name)
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| "Unknown".to_owned()),
                update.health.map(|h| h.as_str().to_owned()),
                update.url,
            )
        }
    };
    if !success {
        return Err(Error::new(mode.context()));
    }
    let mut out = format!("Created status update for: {name}\n");
    if let Some(health) = health.filter(|v| !v.is_empty()) {
        out.push_str(&format!("Health: {health}\n"))
    }
    if mode == Mode::Project || !url.is_empty() {
        out.push_str(&format!("{url}\n"))
    }
    Ok(out.into_bytes())
}
/// Opens an empty editor. A missing editor is reported on stderr and leaves
/// the body empty; Ctrl-C in the editor cancels.
pub fn edit(
    env: &ChildEnvOverlay,
    stderr: &mut dyn Write,
) -> Result<PromptOutcome<Option<String>>, Error> {
    match editor::edit("", env) {
        Ok(text) => Ok(PromptOutcome::Submitted(text_input::edited_body(&text))),
        Err(error) if error.kind() == crate::error::ErrorKind::Cancelled => {
            Ok(PromptOutcome::Interrupted)
        }
        Err(error) if editor::configured(env).is_none() => {
            writeln!(stderr, "{error}")
                .map_err(|e| Error::new("Failed to write editor diagnostic").with_source(e))?;
            Ok(PromptOutcome::Submitted(None))
        }
        Err(error) => Err(error),
    }
}
fn choice(label: &str, value: &str) -> PlainOption {
    PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: value.to_owned(),
    }
}
pub fn prompt<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    stderr: &mut dyn Write,
    env: &ChildEnvOverlay,
    mode: Mode,
) -> Result<PromptOutcome<Fields>, Error> {
    macro_rules! answer {
        ($value:expr) => {
            match $value? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
            }
        };
    }
    let health = answer!(prompt_health(session, mode));
    let editor = editor::configured(env);
    let label = editor
        .as_deref()
        .and_then(crate::commands::document_write::editor_label);
    let mut methods = vec![
        choice("Skip (no content)", "skip"),
        choice("Enter inline", "inline"),
    ];
    if let Some(label) = &label {
        methods.push(choice(&format!("Open {label}"), "editor"))
    }
    methods.push(choice("Read from file", "file"));
    let method = answer!(session.select(&PlainSelect {
        message: "How would you like to enter the update content?",
        options: &methods,
        default_index: 0,
        default_hint: None
    }));
    let body = match method.as_str() {
        "skip" => None,
        "inline" => text_input::edited_body(&answer!(session.text_with_options(
            match mode {
                Mode::Project => "Update content (markdown)",
                Mode::Initiative => "Content (markdown)",
            },
            TextOptions {
                required: false,
                default: Some("")
            }
        ))),
        "file" => Some(file(
            &answer!(session.text_with_options(
                "File path",
                TextOptions {
                    required: false,
                    default: None
                }
            )),
            mode,
            true,
        )?),
        "editor" => {
            let label = label.ok_or_else(|| Error::new("editor choice has no label"))?;
            session.print_line(&format!("Opening {label}..."))?;
            session.suspend()?;
            let result = edit(env, stderr);
            session.resume()?;
            let body = answer!(result);
            if let Some(body) = &body {
                session.print_line(&format!(
                    "Content entered ({} characters)",
                    body.chars().count()
                ))?
            }
            body
        }
        _ => {
            return Err(Error::new("unknown update body method"));
        }
    };
    Ok(PromptOutcome::Submitted(Fields { body, health }))
}

/// Separate public prompt→typed-health boundary, with no editor/config side effects.
pub fn prompt_health<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    mode: Mode,
) -> Result<PromptOutcome<Option<Health>>, Error> {
    let options = match mode {
        Mode::Project => vec![
            choice("On Track", "onTrack"),
            choice("At Risk", "atRisk"),
            choice("Off Track", "offTrack"),
            choice("No change", "none"),
        ],
        Mode::Initiative => vec![
            choice("Skip (no change)", "skip"),
            choice("On Track", "onTrack"),
            choice("At Risk", "atRisk"),
            choice("Off Track", "offTrack"),
        ],
    };
    let outcome = session.select(&PlainSelect {
        message: match mode {
            Mode::Project => "Project health status",
            Mode::Initiative => "Health status",
        },
        options: &options,
        default_index: match mode {
            Mode::Project => 3,
            Mode::Initiative => 0,
        },
        default_hint: None,
    })?;
    let health = match outcome {
        PromptOutcome::Submitted(health) => health,
        PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
        PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
    };
    let health = match health.as_str() {
        "none" | "skip" => None,
        value => Health::parse(Some(value), mode)?,
    };
    Ok(PromptOutcome::Submitted(health))
}
