//! Details and the mutation for `document delete`.
use crate::graphql::schema;
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct IdVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetDocumentForDelete {
    #[arguments(id: $id)]
    pub document: Option<DocumentDetails>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Document")]
pub struct DocumentDetails {
    pub id: cynic::Id,
    pub title: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct DeleteDocument {
    #[arguments(id: $id)]
    pub document_delete: DeletePayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "DocumentArchivePayload")]
pub struct DeletePayload {
    pub success: bool,
}
