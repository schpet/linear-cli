//! `issue title` and `issue url`.
use cynic::QueryBuilder;

use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::issue_details::{GetIssueDetails, IssueDetails, Variables};

#[derive(Clone, Copy)]
pub enum Field {
    Title,
    Url,
}

pub fn run(ctx: &Ctx, issue_id: Option<&str>, field: Field) -> Result<()> {
    let context = match field {
        Field::Title => "Failed to get issue title",
        Field::Url => "Failed to get issue URL",
    };
    print(ctx, issue_id, field).context(context)
}

fn print(ctx: &Ctx, issue_id: Option<&str>, field: Field) -> Result<()> {
    let identifier = super::require(ctx, issue_id)?;
    let client = ctx.client()?;
    let details = ctx.spin(true, fetch(client, identifier))?;
    let value = match field {
        Field::Title => details.title,
        Field::Url => details.url,
    };
    ctx.print(format!("{value}\n"))
}

pub fn request(id: String) -> GraphQlRequest<Variables> {
    GraphQlRequest::with_variables(GetIssueDetails::build(Variables { id }))
}

pub async fn fetch(client: &LinearClient, id: String) -> Result<IssueDetails> {
    let result: GetIssueDetails = client.execute(&request(id)).await?;
    Ok(result.issue)
}
