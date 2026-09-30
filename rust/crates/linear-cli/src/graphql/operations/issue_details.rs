//! Full GetIssueDetails selection used by the script title and URL commands.
use crate::graphql::{
    scalars::{DateTime, JsonObject},
    schema,
};

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct Variables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "Variables")]
pub struct GetIssueDetails {
    #[arguments(id: $id)]
    pub issue: IssueDetails,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueDetails {
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub url: String,
    pub branch_name: String,
    pub state: State,
    pub assignee: Option<Assignee>,
    pub priority: f64,
    pub project: Option<Project>,
    pub project_milestone: Option<Milestone>,
    pub cycle: Option<Cycle>,
    pub team: Team,
    #[arguments(first: 50)]
    pub labels: Labels,
    pub parent: Option<RelatedIssue>,
    #[arguments(first: 250)]
    pub children: Children,
    #[arguments(first: 50)]
    pub attachments: Attachments,
    #[arguments(first: 50)]
    pub documents: Documents,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
pub struct State {
    pub name: String,
    pub color: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct Assignee {
    pub name: String,
    pub display_name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct Project {
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
pub struct Milestone {
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
pub struct Cycle {
    pub id: cynic::Id,
    pub number: f64,
    pub name: Option<String>,
    pub is_active: bool,
    pub is_next: bool,
    pub is_previous: bool,
    pub is_future: bool,
    pub is_past: bool,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct Team {
    pub active_cycle: Option<ActiveCycle>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
pub struct ActiveCycle {
    pub number: f64,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabelConnection")]
pub struct Labels {
    pub nodes: Vec<Label>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabel")]
pub struct Label {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct RelatedIssue {
    pub identifier: String,
    pub title: String,
    pub state: State,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueConnection")]
pub struct Children {
    pub nodes: Vec<RelatedIssue>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "AttachmentConnection")]
pub struct Attachments {
    pub nodes: Vec<Attachment>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Attachment")]
pub struct Attachment {
    pub id: cynic::Id,
    pub title: String,
    pub url: String,
    pub subtitle: Option<String>,
    pub source_type: Option<String>,
    pub metadata: JsonObject,
    pub created_at: DateTime,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "DocumentConnection")]
pub struct Documents {
    pub nodes: Vec<Document>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Document")]
pub struct Document {
    pub id: cynic::Id,
    pub title: String,
    pub slug_id: String,
    pub url: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}
