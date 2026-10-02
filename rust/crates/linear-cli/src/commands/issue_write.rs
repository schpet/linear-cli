use crate::{
    error::{AppError, AppErrorKind},
    graphql::{edit::Edit, operations::issue_update::IssueUpdateInput},
};
use std::future::Future;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    pub id: String,
    pub key: String,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub position: f64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub color: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parent {
    pub title: String,
    pub identifier: String,
    pub project_id: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Created {
    pub id: String,
    pub identifier: String,
    pub url: String,
    pub team_key: String,
}
#[derive(Clone, Debug)]
pub struct Updated {
    pub identifier: String,
    pub title: String,
    pub url: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignSelf {
    Always,
    Auto,
    Never,
}
#[derive(Clone, Debug)]
pub struct CreateSettings {
    pub default_team: Option<String>,
    pub assign_self: AssignSelf,
    pub ask_project: bool,
}

pub fn validation(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Validation, message)
}
pub fn truthy(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
pub fn description(inline: Option<&str>, file: Option<&str>) -> Result<Option<String>, AppError> {
    if truthy(inline).is_some() && truthy(file).is_some() {
        return Err(validation(
            "Cannot specify both --description and --description-file",
        ));
    }
    match truthy(file) {
        None => Ok(inline.map(str::to_owned)),
        Some(path) => crate::commands::text_input::read_file(path)
            .map(Some)
            .map_err(|error| {
                validation(format!("Failed to read description file: {path}"))
                    .with_suggestion(format!("Error: {error}"))
                    .with_source(error)
            }),
    }
}
pub fn integer(value: Option<f64>, field: &str) -> Result<Option<i32>, AppError> {
    value
        .map(|value| {
            if !value.is_finite() || value.fract() != 0.0 {
                return Err(validation(format!("{field} must be a GraphQL integer")));
            }
            value.to_string().parse::<i32>().map_err(|error| {
                validation(format!("{field} is outside the GraphQL integer range"))
                    .with_source(error)
            })
        })
        .transpose()
}
/// JS parseInt's consumed integer prefix; checked i32 conversion follows it.
/// Empty/NaN is omission, syntactically valid overflow is a typed refusal.
pub fn menu_estimate(value: &str) -> Result<Option<i32>, AppError> {
    let value = value.trim_start();
    let (negative, value) = match value.strip_prefix('-') {
        Some(value) => (true, value),
        None => (false, value.strip_prefix('+').unwrap_or(value)),
    };
    let (radix, value) = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(value) => (16, value),
        None => (10, value),
    };
    let digits: String = value.chars().take_while(|c| c.is_digit(radix)).collect();
    if digits.is_empty() {
        return Ok(None);
    }
    let magnitude = i64::from_str_radix(&digits, radix).map_err(|error| {
        validation("estimate is outside the GraphQL integer range").with_source(error)
    })?;
    let signed = if negative { -magnitude } else { magnitude };
    i32::try_from(signed).map(Some).map_err(|error| {
        validation("estimate is outside the GraphQL integer range").with_source(error)
    })
}
pub fn default_state(states: &[State]) -> Result<Option<String>, AppError> {
    let mut lowest: Option<&State> = None;
    for state in states.iter().filter(|state| state.kind == "unstarted") {
        if lowest.is_none_or(|old| state.position < old.position) {
            lowest = Some(state)
        }
    }
    Ok(lowest
        .or_else(|| states.first())
        .map(|state| state.id.clone()))
}
pub fn edit<T>(clear: bool, value: Option<T>) -> Edit<T> {
    if clear {
        Edit::Clear
    } else {
        Edit::set_or_unchanged(value)
    }
}
/// The concrete network adapter must preserve command-local captured Client
/// preferred/errors[0]/raw observation before full selected-model decode.
/// No generic transport/friendly-error wrapper is allowed here.
pub trait Backend: Clone + Send + 'static {
    fn team(&self, reference: String) -> impl Future<Output = Result<Team, AppError>> + Send;
    fn find_team(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Option<Team>, AppError>> + Send;
    fn teams(&self) -> impl Future<Output = Result<Vec<Team>, AppError>> + Send;
    fn team_options(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Vec<Named>, AppError>> + Send;
    fn viewer(&self) -> impl Future<Output = Result<String, AppError>> + Send;
    fn auto_assign(&self) -> impl Future<Output = Result<bool, AppError>> + Send;
    fn user(&self, reference: String) -> impl Future<Output = Result<String, AppError>> + Send;
    fn states(&self, team_key: String)
    -> impl Future<Output = Result<Vec<State>, AppError>> + Send;
    fn state(
        &self,
        team_key: String,
        reference: String,
    ) -> impl Future<Output = Result<String, AppError>> + Send;
    fn label(
        &self,
        team_key: String,
        reference: String,
    ) -> impl Future<Output = Result<Option<String>, AppError>> + Send;
    fn label_options(
        &self,
        team_key: String,
        reference: String,
    ) -> impl Future<Output = Result<Vec<Named>, AppError>> + Send;
    fn labels(&self, team_key: String)
    -> impl Future<Output = Result<Vec<Label>, AppError>> + Send;
    fn project(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Option<String>, AppError>> + Send;
    fn project_options(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Vec<Named>, AppError>> + Send;
    fn projects(
        &self,
        team_key: String,
    ) -> impl Future<Output = Result<Vec<Named>, AppError>> + Send;
    fn milestone(
        &self,
        project_id: String,
        reference: String,
    ) -> impl Future<Output = Result<String, AppError>> + Send;
    fn cycle(
        &self,
        team_id: String,
        reference: String,
    ) -> impl Future<Output = Result<String, AppError>> + Send;
    fn parent_id(&self, reference: String)
    -> impl Future<Output = Result<String, AppError>> + Send;
    fn parent_metadata(
        &self,
        id: String,
    ) -> impl Future<Output = Result<Option<Parent>, AppError>> + Send;
    fn issue_project(
        &self,
        id: String,
    ) -> impl Future<Output = Result<Option<String>, AppError>> + Send;
    fn create(
        &self,
        input: super::issue_create::Input,
    ) -> impl Future<Output = Result<Created, AppError>> + Send;
    fn update(
        &self,
        id: String,
        input: IssueUpdateInput,
    ) -> impl Future<Output = Result<Updated, AppError>> + Send;
}
/// Owned UI adapter suspends before network/editor/output, resumes only for the
/// next prompt; same stdin reader and raw owner survive all prompts.
pub trait Ui {
    fn text(
        &mut self,
        message: &str,
        minimum: usize,
        default: Option<&str>,
    ) -> Result<String, AppError>;
    fn choose(
        &mut self,
        message: &str,
        options: &[Named],
        default: usize,
        search: bool,
    ) -> Result<String, AppError>;
    fn checkbox(
        &mut self,
        message: &str,
        options: &[Named],
        search: bool,
    ) -> Result<Vec<String>, AppError>;
    fn suspend(&mut self) -> Result<(), AppError>;
    fn output(&mut self, text: &str) -> Result<(), AppError>;
    fn error(&mut self, text: &str) -> Result<(), AppError>;
    fn discover_editor(&mut self) -> Result<Option<String>, AppError>;
    fn optional_editor(&mut self) -> Result<Option<String>, AppError>;
}
