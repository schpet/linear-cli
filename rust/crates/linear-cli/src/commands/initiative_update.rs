//! Complete C040 update; raw uncaught exchanges stay command-local.
use crate::{
    commands::{initiative_list::select_owner, initiative_view::Reference},
    error::{AppError, AppErrorKind},
    graphql::{
        bulk_error,
        envelope::GraphQlRequest,
        operations::{
            initiative_update::*,
            initiative_view::{
                DetailVariables, NameVariables, ResolveInitiativeBySlug, SlugVariables,
                UrlSlugVariables,
            },
            initiatives::{
                GetViewerId, GetViewerIdVariables, InitiativeStatus, LookupUser,
                LookupUserVariables,
            },
        },
        scalars::TimelessDate,
        transport::{GraphQlTransport, classify_typed},
    },
    platform::{
        prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession},
        prompt_text::TextOptions,
    },
    refs::{is_linear_uuid, reject_linear_url},
};
use cynic::{MutationBuilder, QueryBuilder};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read, Write};
#[derive(Clone, Default, Debug)]
pub struct Fields {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub target_date: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
}
impl Fields {
    pub fn empty(&self) -> bool {
        [
            &self.name,
            &self.description,
            &self.status,
            &self.owner,
            &self.target_date,
            &self.color,
            &self.icon,
        ]
        .iter()
        .all(|v| v.is_none())
    }
    pub fn should_prompt(&self, interactive: bool, stdout_tty: bool) -> bool {
        interactive
            && stdout_tty
            && [
                &self.name,
                &self.description,
                &self.status,
                &self.owner,
                &self.target_date,
                &self.color,
                &self.icon,
            ]
            .iter()
            .all(|v| v.as_deref().is_none_or(str::is_empty))
    }
    pub fn input(self, owner_id: Option<String>) -> InitiativeUpdateInput {
        InitiativeUpdateInput {
            name: self.name,
            description: self.description,
            status: self
                .status
                .map(|s| InitiativeStatus::Unknown(s.to_lowercase())),
            owner_id,
            target_date: self.target_date.map(TimelessDate),
            color: self.color,
            icon: self.icon,
        }
    }
}
enum Failure {
    Ordinary(AppError),
    Shape(AppError),
}
impl Failure {
    fn error(self) -> AppError {
        match self {
            Self::Ordinary(e) | Self::Shape(e) => e,
        }
    }
}
async fn exchange<T: DeserializeOwned, V: Serialize>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<V>,
    raw: bool,
    mutation: bool,
) -> Result<T, Failure> {
    let response = transport
        .send_request(request)
        .await
        .map_err(|e| Failure::Ordinary(e.into()))?;
    let observed = bulk_error::observe_source_error(&response, request)
        .map_err(|e| Failure::Ordinary(e.into_error()))?;
    if let Some(e) = observed {
        return Err(Failure::Ordinary(AppError::new(
            AppErrorKind::GraphQl,
            if raw {
                e.message
            } else {
                e.preferred_message.unwrap_or(e.message)
            },
        )));
    }
    let data: serde_json::Value =
        classify_typed(response).map_err(|e| Failure::Ordinary(e.into()))?;
    let confirmed = data
        .get("initiativeUpdate")
        .and_then(|v| v.get("success"))
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    serde_json::from_value(data).map_err(|e| {
        Failure::Shape(
            AppError::new(
                AppErrorKind::GraphQl,
                format!(
                    "C040-UNEXPECTED-SHAPE: {e}{}",
                    if !mutation {
                        ""
                    } else if confirmed {
                        "; update confirmed by success:true; do not retry automatically"
                    } else {
                        "; update outcome unknown; do not retry automatically"
                    }
                ),
            )
            .with_source(e),
        )
    })
}
pub async fn resolve(
    transport: &GraphQlTransport,
    reference: &Reference,
    original: &str,
) -> Result<String, AppError> {
    match reference {
        Reference::Id(id) => Ok(id.clone()),
        Reference::UrlSlug(slug) => {
            let req =
                GraphQlRequest::with_variables(ResolveInitiativeBySlug::build(UrlSlugVariables {
                    slug_id: slug.clone(),
                    include_archived: Some(false),
                }));
            let result: ResolveInitiativeBySlug = exchange(transport, &req, true, false)
                .await
                .map_err(Failure::error)?;
            let id = result
                .initiatives
                .nodes
                .first()
                .map(|n| n.id.inner().to_owned())
                .ok_or_else(|| AppError::not_found("Initiative", original))?;
            if is_linear_uuid(&id) {
                Ok(id)
            } else {
                Err(AppError::new(
                    AppErrorKind::GraphQl,
                    "C040-UNEXPECTED-SHAPE: URL resolver returned a non-UUID initiative ID; no update attempted",
                ))
            }
        }
        Reference::NameOrSlug(text) => {
            let req = GraphQlRequest::with_variables(GetInitiativeBySlug::build(SlugVariables {
                slug_id: text.clone(),
            }));
            let result: Result<GetInitiativeBySlug, _> =
                exchange(transport, &req, true, false).await;
            match result {
                Ok(data) => {
                    if let Some(first) = data.initiatives.nodes.first() {
                        return selected_id(first.id.inner(), original);
                    }
                }
                Err(Failure::Ordinary(_)) => {}
                Err(e) => return Err(e.error()),
            }
            let req = GraphQlRequest::with_variables(GetInitiativeByName::build(NameVariables {
                name: text.clone(),
            }));
            let result: Result<GetInitiativeByName, _> =
                exchange(transport, &req, true, false).await;
            match result {
                Ok(data) => {
                    if let Some(first) = data.initiatives.nodes.first() {
                        return selected_id(first.id.inner(), original);
                    }
                }
                Err(Failure::Ordinary(_)) => {}
                Err(e) => return Err(e.error()),
            }
            Err(AppError::not_found("Initiative", original))
        }
    }
}
fn selected_id(id: &str, original: &str) -> Result<String, AppError> {
    if id.is_empty() {
        Err(AppError::not_found("Initiative", original))
    } else {
        Ok(id.to_owned())
    }
}
pub async fn details(
    transport: &GraphQlTransport,
    id: &str,
    original: &str,
) -> Result<CurrentInitiative, AppError> {
    let req = GraphQlRequest::with_variables(GetInitiativeForUpdate::build(DetailVariables {
        id: id.to_owned(),
    }));
    let result: GetInitiativeForUpdate = exchange(transport, &req, false, false)
        .await
        .map_err(|e| e.error().with_context("Failed to fetch initiative details"))?;
    result
        .initiative
        .ok_or_else(|| AppError::not_found("Initiative", original))
}
pub async fn owner(
    transport: &GraphQlTransport,
    input: Option<&str>,
) -> Result<Option<String>, AppError> {
    let Some(input) = input else { return Ok(None) };
    reject_linear_url(input, "an email, username, display name, or @me")?;
    let id = if input == "@me" || input == "self" {
        let req = GraphQlRequest::with_variables(GetViewerId::build(GetViewerIdVariables {}));
        let result: GetViewerId = exchange(transport, &req, true, false)
            .await
            .map_err(Failure::error)?;
        Some(result.viewer.id)
    } else {
        let req = GraphQlRequest::with_variables(LookupUser::build(LookupUserVariables {
            input: input.to_owned(),
        }));
        let result: LookupUser = exchange(transport, &req, true, false)
            .await
            .map_err(Failure::error)?;
        select_owner(&result.users.nodes, input)
    };
    id.filter(|id| !id.inner().is_empty())
        .map(|id| Some(id.into_inner()))
        .ok_or_else(|| AppError::not_found("Owner", input))
}
pub async fn submit(
    transport: &GraphQlTransport,
    id: &str,
    input: InitiativeUpdateInput,
) -> Result<Vec<u8>, AppError> {
    let req = GraphQlRequest::with_variables(UpdateInitiative::build(UpdateVariables {
        id: id.to_owned(),
        input,
    }));
    let result: UpdateInitiative = exchange(transport, &req, false, true)
        .await
        .map_err(|e| e.error().with_context("Failed to update initiative"))?;
    if !result.initiative_update.success {
        return Err(
            AppError::new(AppErrorKind::GraphQl, "Failed to update initiative")
                .with_context("Failed to update initiative"),
        );
    }
    let updated = result.initiative_update.initiative;
    let mut out = format!("✓ Updated initiative: {}\n", updated.name);
    if !updated.url.is_empty() {
        out.push_str(&updated.url);
        out.push('\n');
    }
    Ok(out.into_bytes())
}
pub fn prompt<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    current: &CurrentInitiative,
) -> Result<PromptOutcome<Fields>, AppError> {
    let mut fields = Fields::default();
    macro_rules! text {
        ($message:expr,$default:expr) => {
            match session.text_with_display_default(
                $message,
                TextOptions {
                    minimum_utf16_length: 0,
                    default: Some($default),
                },
            )? {
                PromptOutcome::Submitted(v) => v,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
            }
        };
    }
    let name = text!("Name:", &current.name);
    if name != current.name {
        fields.name = Some(name);
    }
    let default = current.description.as_deref().unwrap_or("");
    let value = text!("Description:", default);
    if value != default {
        fields.description = (!value.is_empty()).then_some(value);
    }
    let options: Vec<_> = [
        ("Planned", "planned"),
        ("Active", "active"),
        ("Completed", "completed"),
        ("Paused", "paused"),
    ]
    .into_iter()
    .map(|(label, value)| PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: value.to_owned(),
    })
    .collect();
    let current_status = current.status.as_ref().map(|s| s.as_str().to_lowercase());
    let index = options
        .iter()
        .position(|s| Some(s.value.as_str()) == current_status.as_deref())
        .unwrap_or(0);
    let status = match session.select(&PlainSelect {
        message: "Status:",
        options: &options,
        default_index: index,
        default_hint: None,
    })? {
        PromptOutcome::Submitted(v) => v,
        PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
        PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
    };
    if Some(status.as_str()) != current_status.as_deref() {
        fields.status = Some(status);
    }
    let default = current
        .target_date
        .as_ref()
        .map(|d| d.0.as_str())
        .unwrap_or("");
    let value = text!("Target date (YYYY-MM-DD):", default);
    if value != default {
        fields.target_date = (!value.is_empty()).then_some(value);
    }
    let default = current.color.as_deref().unwrap_or("");
    let value = text!("Color (hex, e.g., #5E6AD2):", default);
    if value != default {
        fields.color = (!value.is_empty()).then_some(value);
    }
    Ok(PromptOutcome::Submitted(fields))
}
