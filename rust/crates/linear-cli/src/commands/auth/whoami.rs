//! `auth whoami`: the authenticated user and their workspace.
use cynic::QueryBuilder;

use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::auth_whoami::AuthStatus;

pub fn run(ctx: &Ctx) -> Result<()> {
    whoami(ctx).context("Failed to get user info")
}

fn whoami(ctx: &Ctx) -> Result<()> {
    let client = ctx.client()?;
    let request = GraphQlRequest::without_variables(AuthStatus::build(()));
    let status: AuthStatus = ctx.spin(true, client.execute(&request))?;
    ctx.print(render(&status))
}

fn render(status: &AuthStatus) -> String {
    let viewer = &status.viewer;
    let organization = &viewer.organization;
    let mut output = format!(
        "Workspace: {}\n  Slug: {}\n  URL: https://linear.app/{}\nUser: {}\n",
        organization.name, organization.url_key, organization.url_key, viewer.name
    );
    if viewer.display_name != viewer.name {
        output.push_str(&format!("  Display name: {}\n", viewer.display_name));
    }
    output.push_str(&format!("  Email: {}\n", viewer.email));
    if viewer.admin {
        output.push_str("  Role: admin\n");
    } else if viewer.guest {
        output.push_str("  Role: guest\n");
    }
    output
}
