//! `milestone create`: fields from flags or prompts, then one mutation after
//! resolving the project.
use crate::cli::{milestone::MilestoneCreate, values};
use crate::client::LinearClient;
use crate::commands::{confirm, outcome};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::milestone::{
    CreateProjectMilestone, CreateProjectMilestoneVariables, CreatedMilestone,
    ProjectMilestoneCreateInput,
};
use crate::graphql::scalars::TimelessDate;
use crate::platform::prompt::{Prompter, Text};
use crate::refs::{self, project::ProjectReference};

pub fn run(ctx: &Ctx, args: &MilestoneCreate) -> Result<()> {
    create(ctx, args).context("Failed to create milestone")
}

fn create(ctx: &Ctx, args: &MilestoneCreate) -> Result<()> {
    let optional = ctx.optional_prompts(args.interactive)?;
    if args.name.is_none() && !ctx.interactive() {
        return Err(ctx.missing_value("Milestone name is required", "--name"));
    }
    let project = ProjectReference::parse(&args.project, &ctx.scope()?)?;
    let client = ctx.client()?;
    let typed = args.name.is_none() || optional;
    // The project is found before anything is typed for it.
    let (project_id, project_name) = ctx.spin(true, async {
        let id = refs::project::resolve(client, &project).await?;
        let name = if typed && !args.confirm.yes {
            Some(refs::project::name(client, &id).await?)
        } else {
            None
        };
        Ok::<_, Error>((id, name))
    })?;
    let input = if typed {
        prompt(&ctx.prompter()?, args, project_id, optional)?
    } else {
        ProjectMilestoneCreateInput {
            project_id,
            name: args
                .name
                .clone()
                .expect("a name is given unless it is asked for"),
            description: args.description.clone(),
            target_date: args.target_date.map(TimelessDate::from),
        }
    };
    if let Some(project_name) = project_name {
        let question = format!(
            "Create milestone \"{}\" in project \"{project_name}\"?",
            input.name
        );
        if !confirm::proceed(ctx, args.confirm.yes, &question)? {
            return Ok(());
        }
    }
    let milestone = ctx.spin(true, submit(client, input))?;
    ctx.print(render(&milestone))
}

/// Asks for the name when it was not given, and with `optional` for the
/// description and target date the flags left out.
fn prompt(
    prompter: &Prompter<'_>,
    args: &MilestoneCreate,
    project_id: String,
    optional: bool,
) -> Result<ProjectMilestoneCreateInput> {
    let name = match &args.name {
        Some(name) => name.clone(),
        None => prompter.text(Text::new("Milestone name:").required())?,
    };
    let description = match &args.description {
        Some(description) => Some(description.clone()),
        None if optional => {
            let answer = prompter.text(Text::new("Description:"))?;
            (!answer.is_empty()).then_some(answer)
        }
        None => None,
    };
    let target_date = match args.target_date {
        Some(date) => Some(date),
        None if optional => {
            prompter.parsed(Text::new("Target date (YYYY-MM-DD):"), &values::date)?
        }
        None => None,
    };
    Ok(ProjectMilestoneCreateInput {
        project_id,
        name,
        description,
        target_date: target_date.map(TimelessDate::from),
    })
}

/// Sends the mutation once. A failure after the request may have reached
/// Linear says the milestone may already exist; nothing is retried.
async fn submit(
    client: &LinearClient,
    input: ProjectMilestoneCreateInput,
) -> Result<CreatedMilestone> {
    let result: CreateProjectMilestone = client
        .mutate(CreateProjectMilestoneVariables { input })
        .await
        .map_err(|failure| failure.into_create_error("milestone"))?;
    let payload = result.project_milestone_create;
    if !payload.success {
        return Err(Error::new("Linear did not create the milestone"));
    }
    Ok(payload.project_milestone)
}

fn render(milestone: &CreatedMilestone) -> String {
    let mut output = outcome::done("Created", "milestone", &milestone.name, None);
    output.push_str(&format!("  ID: {}\n", milestone.id.inner()));
    if let Some(date) = milestone.target_date {
        output.push_str(&format!("  Target Date: {date}\n"));
    }
    output.push_str(&format!("  Project: {}\n", milestone.project.name));
    output
}
