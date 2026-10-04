use crate::config::AssignSelf;
use crate::refs::team::ResolvedTeam;
use crate::{
    cli::values::UserRef,
    error::Error,
    graphql::{edit::Edit, operations::issue::IssueUpdateInput},
    platform::prompt::{Choice, Text},
};
use std::future::Future;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub name: String,
    /// Tells this apart from others with the same name in a picker, like a
    /// project's slug ID.
    pub detail: Option<String>,
}

/// The question offering `candidates`, near matches for the `kind` (like
/// `Project`) the user called `original` that did not resolve, and its
/// choices. `None` without candidates.
pub fn suggestions(
    kind: &str,
    original: &str,
    candidates: &[&Named],
) -> Option<(String, Vec<Choice<Option<String>>>)> {
    match candidates {
        [] => None,
        [only] => Some((
            format!("{kind} \"{original}\" not found. Use \"{}\"?", only.name),
            vec![
                Choice::new("Yes", Some(only.id.clone())),
                Choice::new("No", None),
            ],
        )),
        many => Some((
            format!("{kind} \"{original}\" not found. Did you mean one of these?"),
            Named::choices(many)
                .into_iter()
                .chain([Choice::new("None of these", None)])
                .collect(),
        )),
    }
}

impl Named {
    /// Picker choices for `options`, telling apart those that share a name.
    pub fn choices(options: &[&Named]) -> Vec<Choice<Option<String>>> {
        crate::platform::prompt::distinct_choices(
            options
                .iter()
                .map(|option| {
                    (
                        option.name.clone(),
                        option.detail.clone(),
                        Some(option.id.clone()),
                    )
                })
                .collect(),
        )
    }
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
    /// The team the label belongs to, or `None` for a workspace label.
    pub team_key: Option<String>,
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
    pub title: String,
    pub url: String,
}
#[derive(Clone, Debug)]
pub struct Updated {
    pub identifier: String,
    pub title: String,
    pub url: String,
}
#[derive(Clone, Debug)]
pub struct CreateSettings {
    pub default_team: Option<String>,
    pub assign_self: AssignSelf,
    pub ask_project: bool,
}

/// The description from `--description` or `--description-file`; an empty
/// `--description` counts as not given.
pub fn description(
    inline: Option<&str>,
    file: Option<&crate::cli::values::TextSource>,
) -> Result<Option<String>, Error> {
    if inline.is_some_and(|text| !text.is_empty()) && file.is_some() {
        return Err(Error::new(
            "Cannot specify both --description and --description-file",
        ));
    }
    match file {
        None => Ok(inline.map(str::to_owned)),
        Some(source) => crate::commands::text_input::read_source(source).map_err(|error| {
            Error::new(format!("Failed to read description file: {source}"))
                .with_hint(format!("Error: {error}"))
                .with_source(error)
        }),
    }
}
/// The first state of this kind at the lowest position; equal positions,
/// including signed zero, keep their input order.
pub(super) fn lowest_of_kind<'a>(states: &'a [State], kind: &str) -> Option<&'a State> {
    let mut lowest: Option<&State> = None;
    for state in states.iter().filter(|state| state.kind == kind) {
        if lowest.is_none_or(|old| state.position < old.position) {
            lowest = Some(state)
        }
    }
    lowest
}
pub fn default_state(states: &[State]) -> Option<String> {
    lowest_of_kind(states, "unstarted")
        .or_else(|| states.first())
        .map(|state| state.id.clone())
}
pub fn edit<T>(clear: bool, value: Option<T>) -> Edit<T> {
    if clear {
        Edit::Clear
    } else {
        Edit::set_or_unchanged(value)
    }
}
/// The Linear lookups and mutations issue create and update resolve names through.
pub trait Backend: Clone + Send + 'static {
    fn team(&self, reference: String) -> impl Future<Output = Result<ResolvedTeam, Error>> + Send;
    fn find_team(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Option<ResolvedTeam>, Error>> + Send;
    fn teams(&self) -> impl Future<Output = Result<Vec<ResolvedTeam>, Error>> + Send;
    fn team_options(
        &self,
        reference: String,
    ) -> impl Future<Output = Result<Vec<Named>, Error>> + Send;
    fn viewer(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn auto_assign(&self) -> impl Future<Output = Result<bool, Error>> + Send;
    fn user(&self, user: UserRef) -> impl Future<Output = Result<String, Error>> + Send;
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
    /// What `parse` makes of the answer to `text`; `None` when left blank.
    fn parsed<T>(
        &mut self,
        text: Text<'_>,
        parse: &dyn Fn(&str) -> Result<T, String>,
    ) -> Result<Option<T>, Error>;
    /// The picked value; the list starts on the choice at `default`.
    fn choose<T>(
        &mut self,
        message: &str,
        choices: Vec<Choice<T>>,
        default: usize,
    ) -> Result<T, Error>;
    /// The picked values, in list order.
    fn checkbox<T>(&mut self, message: &str, choices: Vec<Choice<T>>) -> Result<Vec<T>, Error>;
    /// Shows `text` beside the questions: a note or progress line, never
    /// part of the command's result.
    fn note(&mut self, text: &str) -> Result<(), Error>;
    fn discover_editor(&mut self) -> Result<Option<String>, Error>;
    fn optional_editor(&mut self) -> Result<Option<String>, Error>;
}

#[cfg(test)]
mod tests {
    use super::{State, default_state};

    #[test]
    fn no_workflow_states_means_no_default() {
        assert_eq!(default_state(&[]), None);
    }

    #[test]
    fn without_unstarted_states_the_first_state_is_the_default() {
        let state = |id: &str, kind: &str, position| State {
            id: id.to_owned(),
            name: id.to_owned(),
            kind: kind.to_owned(),
            position,
        };
        let states = [
            state("started", "started", 9.0),
            state("backlog", "backlog", 1.0),
        ];
        assert_eq!(default_state(&states).as_deref(), Some("started"));
    }
    #[test]
    fn equal_signed_zero_positions_keep_the_first_default_state() {
        for positions in [[-0.0, 0.0], [0.0, -0.0]] {
            let states = ["first", "second"]
                .into_iter()
                .zip(positions)
                .map(|(id, position)| State {
                    id: id.to_owned(),
                    name: id.to_owned(),
                    kind: "unstarted".to_owned(),
                    position,
                })
                .collect::<Vec<_>>();
            assert_eq!(default_state(&states).as_deref(), Some("first"));
        }
    }
}
