use crate::{
    error::Error,
    graphql::{edit::Edit, operations::issue::IssueUpdateInput},
    platform::prompt::Text,
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
    pub identifier: String,
    pub url: String,
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

pub fn validation(message: impl Into<String>) -> Error {
    Error::new(message)
}
pub fn truthy(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
pub fn description(inline: Option<&str>, file: Option<&str>) -> Result<Option<String>, Error> {
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
                    .with_hint(format!("Error: {error}"))
                    .with_source(error)
            }),
    }
}
pub fn default_state(states: &[State]) -> Result<Option<String>, Error> {
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
    fn team(&self, reference: String) -> impl Future<Output = Result<Team, Error>> + Send;
    fn find_team(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Option<Team>, Error>> + Send;
    fn teams(&self) -> impl Future<Output = Result<Vec<Team>, Error>> + Send;
    fn team_options(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Vec<Named>, Error>> + Send;
    fn viewer(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn auto_assign(&self) -> impl Future<Output = Result<bool, Error>> + Send;
    fn user(&self, reference: String) -> impl Future<Output = Result<String, Error>> + Send;
    fn states(&self, team_key: String) -> impl Future<Output = Result<Vec<State>, Error>> + Send;
    fn state(
        &self,
        team_key: String,
        reference: String,
    ) -> impl Future<Output = Result<String, Error>> + Send;
    fn label(
        &self,
        team_key: String,
        reference: String,
    ) -> impl Future<Output = Result<Option<String>, Error>> + Send;
    fn label_options(
        &self,
        team_key: String,
        reference: String,
    ) -> impl Future<Output = Result<Vec<Named>, Error>> + Send;
    fn labels(&self, team_key: String) -> impl Future<Output = Result<Vec<Label>, Error>> + Send;
    fn project(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Option<String>, Error>> + Send;
    fn project_options(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Vec<Named>, Error>> + Send;
    fn projects(&self, team_key: String) -> impl Future<Output = Result<Vec<Named>, Error>> + Send;
    fn milestone(
        &self,
        project_id: String,
        reference: String,
    ) -> impl Future<Output = Result<String, Error>> + Send;
    fn cycle(
        &self,
        team_id: String,
        reference: String,
    ) -> impl Future<Output = Result<String, Error>> + Send;
    fn parent_id(&self, reference: String) -> impl Future<Output = Result<String, Error>> + Send;
    fn parent_metadata(
        &self,
        id: String,
    ) -> impl Future<Output = Result<Option<Parent>, Error>> + Send;
    fn issue_project(
        &self,
        id: String,
    ) -> impl Future<Output = Result<Option<String>, Error>> + Send;
    fn create(
        &self,
        input: super::create::Input,
    ) -> impl Future<Output = Result<Created, Error>> + Send;
    fn update(
        &self,
        id: String,
        input: IssueUpdateInput,
    ) -> impl Future<Output = Result<Updated, Error>> + Send;
}
/// The questions issue creation asks, so tests can answer them.
pub trait Ui {
    /// The checked, trimmed answer to `text`.
    fn text(&mut self, text: Text<'_>) -> Result<String, Error>;
    /// The id of the picked option; the list starts on the one at `default`.
    fn choose(&mut self, message: &str, options: &[Named], default: usize)
    -> Result<String, Error>;
    /// The ids of the picked options, in list order.
    fn checkbox(&mut self, message: &str, options: &[Named]) -> Result<Vec<String>, Error>;
    fn output(&mut self, text: &str) -> Result<(), Error>;
    fn error(&mut self, text: &str) -> Result<(), Error>;
    fn discover_editor(&mut self) -> Result<Option<String>, Error>;
    fn optional_editor(&mut self) -> Result<Option<String>, Error>;
}
