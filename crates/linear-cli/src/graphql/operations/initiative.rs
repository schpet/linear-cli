//! Initiative operations: lists, details, create, update, archive, delete and project links.

use serde::Serialize;

use super::common::DeletePayload;
use super::common::IdVariablesFields;
use super::project::ProjectStatusType;
use super::team::StringComparator;
use super::user::UserRef;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::scalars::Float;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::schema;

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeForArchive {
    #[arguments(id: $id)]
    pub initiative: Option<ArchiveDetail>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct ArchiveDetail {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub archived_at: Option<DateTime>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeForDelete {
    #[arguments(id: $id)]
    pub initiative: Option<DeleteDetail>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct DeleteDetail {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub projects: Option<LinkedProjects>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct LinkedProjects {
    pub nodes: Vec<LinkedProjectId>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct LinkedProjectId {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct ArchiveInitiative {
    #[arguments(id: $id)]
    pub initiative_archive: ArchivePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeArchivePayload")]
pub struct ArchivePayload {
    pub success: bool,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct DeleteInitiative {
    #[arguments(id: $id)]
    pub initiative_delete: DeletePayload,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateInitiativeVariables {
    pub input: InitiativeCreateInput,
}

#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeCreateInput")]
pub struct InitiativeCreateInput {
    pub name: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status: Option<InitiativeStatus>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub target_date: Option<TimelessDate>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateInitiativeVariables"
)]
pub struct CreateInitiative {
    #[arguments(input: $input)]
    pub initiative_create: CreateInitiativePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativePayload")]
pub struct CreateInitiativePayload {
    pub success: bool,
    pub initiative: CreatedInitiative,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct CreatedInitiative {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub url: String,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct LinksVariables {
    pub initiative_id: String,
    pub project_id: String,
    pub after: Option<String>,
}

/// Both names, and one page of the initiatives the project is linked to.
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LinksVariables"
)]
pub struct GetInitiativeProjectLinks {
    #[arguments(id: $initiative_id)]
    pub initiative: Named,
    #[arguments(id: $project_id)]
    pub project: LinkedProject,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct Named {
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "LinksVariables"
)]
pub struct LinkedProject {
    pub name: String,
    #[arguments(first: 100, after: $after)]
    pub initiative_to_projects: Links,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectConnection")]
pub struct Links {
    pub nodes: Vec<Link>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProject")]
pub struct Link {
    pub id: cynic::Id,
    pub initiative: LinkedInitiative,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct LinkedInitiative {
    pub id: cynic::Id,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct AddVariables {
    pub input: InitiativeToProjectCreateInput,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectCreateInput")]
pub struct InitiativeToProjectCreateInput {
    pub initiative_id: String,
    pub project_id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<Float>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "AddVariables"
)]
pub struct AddProjectToInitiative {
    #[arguments(input: $input)]
    pub initiative_to_project_create: AddPayload,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectPayload")]
pub struct AddPayload {
    pub success: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct RemoveProjectFromInitiative {
    #[arguments(id: $id)]
    pub initiative_to_project_delete: DeletePayload,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct InitiativeNameVariables {
    pub name: String,
    pub include_archived: bool,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "InitiativeNameVariables"
)]
pub struct ResolveInitiativeByName {
    #[arguments(filter: { name: { eqIgnoreCase: $name } }, includeArchived: $include_archived)]
    pub initiatives: InitiativeNameConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeNameConnection {
    pub nodes: Vec<InitiativeName>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeName {
    pub id: cynic::Id,
    pub name: String,
    pub slug_id: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct ArchivedLookupVariables {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ArchivedLookupVariables"
)]
pub struct GetInitiativeForUnarchive {
    #[arguments(filter: { id: { eq: $id } }, includeArchived: true)]
    pub initiatives: UnarchiveDetails,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct UnarchiveDetails {
    pub nodes: Vec<UnarchiveDetail>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct UnarchiveDetail {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub archived_at: Option<DateTime>,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct UnarchiveVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UnarchiveVariables"
)]
pub struct UnarchiveInitiative {
    #[arguments(id: $id)]
    pub initiative_unarchive: UnarchivePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeArchivePayload")]
pub struct UnarchivePayload {
    pub success: bool,
    pub entity: Option<UnarchivedInitiative>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct UnarchivedInitiative {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub url: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DetailVariables"
)]
pub struct GetInitiativeForUpdate {
    #[arguments(id: $id)]
    pub initiative: Option<CurrentInitiative>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct CurrentInitiative {
    pub name: String,
    pub description: Option<String>,
    pub status: Option<InitiativeStatus>,
    pub target_date: Option<TimelessDate>,
    pub color: Option<String>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdateInput")]
pub struct InitiativeUpdateInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status: Option<InitiativeStatus>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub target_date: Option<TimelessDate>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UpdateVariables {
    pub id: String,
    pub input: InitiativeUpdateInput,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateVariables"
)]
pub struct UpdateInitiative {
    #[arguments(id: $id, input: $input)]
    pub initiative_update: UpdatedPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativePayload")]
pub struct UpdatedPayload {
    pub success: bool,
    pub initiative: UpdatedInitiative,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct UpdatedInitiative {
    pub name: String,
    pub url: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct DetailVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DetailVariables"
)]
pub struct GetInitiativeDetails {
    // Handled as nullable even though the schema declares non-null.
    #[arguments(id: $id)]
    pub initiative: Option<InitiativeDetails>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeDetails {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: InitiativeStatus,
    pub target_date: Option<TimelessDate>,
    pub health: Option<InitiativeUpdateHealthType>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub url: String,
    pub archived_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub owner: Option<UserRef>,
    pub projects: InitiativeViewProjects,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct InitiativeViewProjects {
    pub nodes: Vec<InitiativeViewProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct InitiativeViewProject {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub status: InitiativeViewProjectStatus,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectStatus")]
pub struct InitiativeViewProjectStatus {
    pub name: String,
    #[cynic(rename = "type")]
    pub status_type: ProjectStatusType,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct UrlSlugVariables {
    pub slug_id: String,
    pub include_archived: bool,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "UrlSlugVariables"
)]
pub struct ResolveInitiativeBySlug {
    #[arguments(filter: { slugId: { eq: $slug_id } }, includeArchived: $include_archived)]
    pub initiatives: InitiativeUrlResults,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeUrlResults {
    pub nodes: Vec<InitiativeUrlNode>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeUrlNode {
    pub id: cynic::Id,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetInitiativesVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<InitiativeFilter>,
    pub include_archived: Option<bool>,
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct InitiativeFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status: Option<StringComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub owner: Option<NullableUserFilter>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct NullableUserFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub id: Option<IDComparator>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IDComparator")]
pub struct IDComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq: Option<cynic::Id>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetInitiativesVariables"
)]
pub struct GetInitiatives {
    // The schema declares this non-null, but a null response is handled
    // rather than treated as a decode failure.
    #[arguments(filter: $filter, includeArchived: $include_archived, first: $first, after: $after)]
    pub initiatives: Option<InitiativeConnection>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct InitiativeConnection {
    pub nodes: Vec<Initiative>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct Initiative {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: InitiativeStatus,
    pub target_date: Option<TimelessDate>,
    pub health: Option<InitiativeUpdateHealthType>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub url: String,
    pub archived_at: Option<DateTime>,
    pub owner: Option<InitiativeOwner>,
    pub projects: InitiativeProjects,
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", non_exhaustive)]
pub enum InitiativeStatus {
    #[cynic(rename = "Active")]
    Active,
    #[cynic(rename = "Canceled")]
    Canceled,
    #[cynic(rename = "Completed")]
    Completed,
    #[cynic(rename = "Planned")]
    Planned,
    #[cynic(rename = "Proposed")]
    Proposed,
    #[cynic(fallback)]
    Unknown(String),
}

impl InitiativeStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "Active",
            Self::Canceled => "Canceled",
            Self::Completed => "Completed",
            Self::Planned => "Planned",
            Self::Proposed => "Proposed",
            Self::Unknown(value) => value,
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            Self::Active => 1,
            Self::Planned => 2,
            Self::Completed => 3,
            Self::Canceled | Self::Proposed | Self::Unknown(_) => 255,
        }
    }
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", rename_all = "camelCase", non_exhaustive)]
pub enum InitiativeUpdateHealthType {
    AtRisk,
    OffTrack,
    OnTrack,
    #[cynic(fallback)]
    Unknown(String),
}

impl InitiativeUpdateHealthType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::AtRisk => "atRisk",
            Self::OffTrack => "offTrack",
            Self::OnTrack => "onTrack",
            Self::Unknown(value) => value,
        }
    }
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct InitiativeOwner {
    pub id: cynic::Id,
    pub display_name: String,
    pub initials: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
#[serde(transparent)]
pub struct InitiativeProjects {
    pub nodes: Vec<InitiativeProject>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct InitiativeProject {
    pub id: cynic::Id,
    pub name: String,
    pub status: InitiativeProjectStatus,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectStatus")]
pub struct InitiativeProjectStatus {
    pub name: String,
}
