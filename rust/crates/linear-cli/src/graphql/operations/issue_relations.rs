//! First-page issue relation selections. Relation types remain arbitrary strings.
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct IssueVariables {
    pub issue_id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssueVariables"
)]
pub struct ListIssueRelations {
    #[arguments(id: $issue_id)]
    pub issue: ListedIssue,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct ListedIssue {
    pub identifier: String,
    pub title: String,
    pub relations: Outgoing,
    pub inverse_relations: Incoming,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationConnection")]
pub struct Outgoing {
    pub nodes: Vec<OutgoingRelation>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelation")]
pub struct OutgoingRelation {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    pub relation_type: String,
    pub related_issue: DisplayIssue,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationConnection")]
pub struct Incoming {
    pub nodes: Vec<IncomingRelation>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelation")]
pub struct IncomingRelation {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    pub relation_type: String,
    pub issue: DisplayIssue,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct DisplayIssue {
    pub identifier: String,
    pub title: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssueVariables"
)]
pub struct FindIssueRelation {
    #[arguments(id: $issue_id)]
    pub issue: FindIssue,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct FindIssue {
    pub relations: FoundRelations,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationConnection")]
pub struct FoundRelations {
    pub nodes: Vec<FoundRelation>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelation")]
pub struct FoundRelation {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    pub relation_type: String,
    pub related_issue: crate::graphql::operations::issue_id::IssueId,
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "IssueRelationType",
    rename_all = "lowercase"
)]
pub enum ApiRelationType {
    Blocks,
    Duplicate,
    Related,
    Similar,
}

impl ApiRelationType {
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::Duplicate => "duplicate",
            Self::Related => "related",
            Self::Similar => "similar",
        }
    }
}
#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationCreateInput")]
pub struct RelationInput {
    pub issue_id: String,
    pub related_issue_id: String,
    #[cynic(rename = "type")]
    pub relation_type: ApiRelationType,
}
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateVariables {
    pub input: RelationInput,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateVariables"
)]
pub struct CreateIssueRelation {
    #[arguments(input: $input)]
    pub issue_relation_create: CreatedPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationPayload")]
pub struct CreatedPayload {
    pub success: bool,
}
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DeleteVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "DeleteVariables"
)]
pub struct DeleteIssueRelation {
    #[arguments(id: $id)]
    pub issue_relation_delete: DeletedPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct DeletedPayload {
    pub success: bool,
}
