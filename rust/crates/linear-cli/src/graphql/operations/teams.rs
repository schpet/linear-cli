//! `GetTeams`: the paginated connection used by `team list`.
//!
//! `filter` and `after` are omitted from the variables object when `None`,
//! including `after` on the first page. Only the raw `api --paginate` path sends `after: null`.

use serde::Serialize;

use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

/// Variables for [`GetTeams`]. `first` is the document's nullable `Int`;
/// `team list` sends `100`, and an unset `first` is omitted rather than sent
/// as `null`.
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetTeamsVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<TeamFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// A subset of the schema's `TeamFilter` input object.
///
/// No command sends a team filter yet; unset keys are omitted. Extend field
/// by field as commands need them.
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamFilter")]
pub struct TeamFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<StringComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub key: Option<StringComparator>,
}

/// A subset of the schema's `StringComparator` input object.
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "StringComparator")]
pub struct StringComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub contains_ignore_case: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetTeamsVariables"
)]
pub struct GetTeams {
    #[arguments(filter: $filter, first: $first, after: $after)]
    pub teams: TeamConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct TeamConnection {
    pub nodes: Vec<Team>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: cynic::Id,
    pub name: String,
    pub key: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub cycles_enabled: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub archived_at: Option<DateTime>,
    pub organization: Organization,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct Organization {
    pub id: cynic::Id,
    pub name: String,
}
