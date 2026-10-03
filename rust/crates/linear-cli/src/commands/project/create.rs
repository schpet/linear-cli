//! `project create`: fields from flags or prompts, then one mutation, then
//! the optional initiative link.
use std::io::Stdin;

use chrono::NaiveDate;
use cynic::MutationBuilder;

use crate::cli::project::{ProjectCreate, Status};
use crate::cli::values::Priority;
use crate::commands::project::write;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_write::{
    AddProjectToInitiative, CreateProject, CreateProjectVariables, CreatedProject,
    InitiativeLinkInput, LinkVariables, ProjectCreateInput,
};
use crate::graphql::operations::projects::ProjectStatusType;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::transport::GraphQlTransport;
use crate::platform::output::StdoutWriter;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};
use crate::platform::prompt_text::TextOptions;
use crate::platform::style;
use crate::refs::{self, prepare_initiative_lookup, resolve_initiative_with_transport};

pub fn run(ctx: &Ctx, args: &ProjectCreate) -> Result<()> {
    create(ctx, args).context("Failed to create project")
}

/// The fields the prompts can fill in.
struct Draft {
    name: Option<String>,
    description: Option<String>,
    teams: Vec<String>,
    status: Option<StatusChoice>,
    lead: Option<String>,
    start_date: Option<NaiveDate>,
    target_date: Option<NaiveDate>,
}

enum StatusChoice {
    /// The first status of this kind (from `--status`).
    Kind(Status),
    /// A status picked at the prompt.
    Id(String),
}

fn create(ctx: &Ctx, args: &ProjectCreate) -> Result<()> {
    let fields = &args.fields;
    let description = write::description(fields)?;
    let content = write::content(fields)?;
    write::plain_references(
        fields.lead.iter().chain(&args.member),
        "an email, username, display name, or @me",
    )?;
    write::plain_references(&args.label, "a project label name")?;
    write::plain_references(&args.template, "a template name or UUID")?;
    let scope = ctx.scope()?;
    let initiative = match args.initiative.as_deref() {
        Some(original) => Some((original, prepare_initiative_lookup(original, &scope)?)),
        None => None,
    };
    let interactive =
        ctx.stdout_tty() && (args.interactive || (fields.name.is_none() && args.team.is_empty()));
    if args.interactive && !interactive {
        return Err(Error::new("Interactive mode needs a terminal")
            .with_hint("Pass --name and --team instead of --interactive."));
    }
    let mut draft = Draft {
        name: fields.name.clone(),
        description,
        teams: args.team.clone(),
        status: fields.status.map(StatusChoice::Kind),
        lead: fields.lead.clone(),
        start_date: fields.start_date,
        target_date: fields.target_date,
    };
    if interactive {
        ctx.print("\nCreate a new project\n\n")?;
        let mut session = ctx.prompts()?;
        let result = prompt(
            ctx,
            &mut session,
            &mut draft,
            fields.description_file.is_some(),
        );
        session.close()?;
        result?;
    }
    let name = draft
        .name
        .ok_or_else(|| Error::new("Project name is required").with_hint("Pass --name."))?;
    let teams = if draft.teams.is_empty() {
        vec![configured_team_key(ctx.options()).ok_or_else(|| {
            Error::new("At least one team is required")
                .with_hint("Pass --team, or run `linear config` to set a default team.")
        })?]
    } else {
        draft.teams
    };
    let teams = write::prepare_teams(&teams, &scope)?;
    let client = ctx.client()?;
    let (project, linked) = ctx.spin(!args.json, async {
        let team_ids: Vec<_> = write::teams(client, &teams)
            .await?
            .into_iter()
            .map(|team| team.id)
            .collect();
        let initiative_id = match &initiative {
            Some((original, reference)) => Some((
                *original,
                resolve_initiative_with_transport(reference, original, client).await?,
            )),
            None => None,
        };
        let template_id = match &args.template {
            Some(template) => Some(write::template(client, template, &team_ids).await?),
            None => None,
        };
        let lead_id = match &draft.lead {
            Some(lead) => Some(write::user(client, lead, "Lead").await?),
            None => None,
        };
        let status_id = match draft.status {
            Some(StatusChoice::Kind(status)) => Some(write::status_id(client, status).await?),
            Some(StatusChoice::Id(id)) => Some(id),
            None => None,
        };
        let label_ids: Vec<_> = write::labels(client, &args.label)
            .await?
            .into_iter()
            .map(|label| label.id)
            .collect();
        let mut member_ids = Vec::new();
        for member in &args.member {
            member_ids.push(write::user(client, member, "User").await?);
        }
        let input = ProjectCreateInput {
            name,
            team_ids,
            description: draft.description,
            content,
            lead_id,
            status_id,
            start_date: draft.start_date.map(TimelessDate::from),
            target_date: draft.target_date.map(TimelessDate::from),
            priority: fields.priority.map(Priority::number),
            label_ids: (!label_ids.is_empty()).then_some(label_ids),
            member_ids: (!member_ids.is_empty()).then_some(member_ids),
            icon: args.icon.clone(),
            color: args.color.clone(),
            template_id,
        };
        let project = submit(client, input).await?;
        // The project exists now: a failed link is reported, not fatal to the output.
        let linked = match initiative_id {
            Some((original, id)) => Some((original, link(client, &project, id).await)),
            None => None,
        };
        Ok::<_, Error>((project, linked))
    })?;
    if args.json {
        let mut output = serde_json::to_vec_pretty(&serde_json::json!({
            "success": true,
            "project": project,
        }))
        .expect("project JSON always serializes");
        output.push(b'\n');
        ctx.print(output)?;
    } else {
        ctx.print(format!(
            "✓ Created project: {}\n  Slug: {}\n  URL: {}\n",
            project.name, project.slug_id, project.url
        ))?;
    }
    let Some((initiative, linked)) = linked else {
        return Ok(());
    };
    match linked {
        Ok(()) if args.json => Ok(()),
        Ok(()) => ctx.print(format!("✓ Added to initiative: {initiative}\n")),
        Err(error) => {
            let color = ctx.terminal().stderr_color();
            ctx.eprint(format!(
                "{}\n{}\n",
                style::red(
                    &format!("✗ Failed to add the project to initiative {initiative}: {error}"),
                    color
                ),
                style::gray(
                    &format!(
                        "Run `linear project update {} --add-initiative {initiative}` to retry.",
                        project.slug_id
                    ),
                    color
                ),
            ))?;
            Err(Error::reported())
        }
    }
}

async fn submit(client: &GraphQlTransport, input: ProjectCreateInput) -> Result<CreatedProject> {
    let request =
        GraphQlRequest::with_variables(CreateProject::build(CreateProjectVariables { input }));
    let result: CreateProject = client.execute(&request).await?;
    let payload = result.project_create;
    match payload.project {
        Some(project) if payload.success => Ok(project),
        Some(_) | None => Err(Error::new("Linear did not create the project")),
    }
}

async fn link(
    client: &GraphQlTransport,
    project: &CreatedProject,
    initiative_id: String,
) -> Result<()> {
    let request = GraphQlRequest::with_variables(AddProjectToInitiative::build(LinkVariables {
        input: InitiativeLinkInput {
            initiative_id,
            project_id: project.id.inner().to_owned(),
        },
    }));
    let result: AddProjectToInitiative = client.execute(&request).await?;
    if result.initiative_to_project_create.success {
        Ok(())
    } else {
        Err(Error::new("Linear rejected the link"))
    }
}

/// Asks for every field the flags left out.
fn prompt(
    ctx: &Ctx,
    session: &mut PromptSession<Stdin, StdoutWriter<'_>>,
    draft: &mut Draft,
    description_file: bool,
) -> Result<()> {
    let text = |required| TextOptions {
        required,
        default: None,
    };
    if draft.name.is_none() {
        draft.name = Some(answer(
            session.text_with_options("Project name:", text(true))?,
        )?);
    }
    if draft.description.is_none() && !description_file {
        let description =
            answer(session.text_with_options("Description (optional):", text(false))?)?;
        draft.description = (!description.is_empty()).then_some(description);
    }
    if draft.teams.is_empty() {
        session.suspend()?;
        let teams = ctx.block_on(refs::fetch_all_teams_with_transport(ctx.client()?));
        session.resume()?;
        let options: Vec<_> = teams?
            .into_iter()
            .map(|team| PlainOption {
                label: format!("{} ({})", team.name, team.key),
                value: team.key.clone(),
                script_token: team.key,
            })
            .collect();
        let default_team = configured_team_key(ctx.options());
        let default_index = options
            .iter()
            .position(|option| Some(&option.value) == default_team.as_ref())
            .unwrap_or(0);
        draft.teams = vec![answer(session.select(&PlainSelect {
            message: "Team:",
            options: &options,
            default_index,
            default_hint: None,
        })?)?];
    }
    if draft.status.is_none() {
        session.suspend()?;
        let statuses = ctx.block_on(write::statuses(ctx.client()?));
        session.resume()?;
        let statuses = statuses?;
        if !statuses.is_empty() {
            let default_index = statuses
                .iter()
                .position(|status| status.status_type == ProjectStatusType::Planned)
                .unwrap_or(0);
            let options: Vec<_> = statuses
                .into_iter()
                .map(|status| PlainOption {
                    label: status.name.clone(),
                    value: status.id.into_inner(),
                    script_token: status.name,
                })
                .collect();
            draft.status = Some(StatusChoice::Id(answer(session.select(&PlainSelect {
                message: "Status:",
                options: &options,
                default_index,
                default_hint: None,
            })?)?));
        }
    }
    if draft.lead.is_none() {
        let lead = answer(session.text_with_options(
            "Lead (username, email, or @me - press Enter to skip):",
            text(false),
        )?)?;
        if !lead.is_empty() {
            refs::reject_linear_url(&lead, "an email, username, display name, or @me")?;
            draft.lead = Some(lead);
        }
    }
    for (date, message) in [
        (
            &mut draft.start_date,
            "Start date (YYYY-MM-DD - press Enter to skip):",
        ),
        (
            &mut draft.target_date,
            "Target date (YYYY-MM-DD - press Enter to skip):",
        ),
    ] {
        if date.is_none() {
            let answer = answer(session.text_with_options(message, text(false))?)?;
            if !answer.is_empty() {
                *date = Some(NaiveDate::parse_from_str(&answer, "%Y-%m-%d").map_err(|_| {
                    Error::new(format!("Invalid date {answer:?}"))
                        .with_hint("Enter dates like 2025-01-31.")
                })?);
            }
        }
    }
    Ok(())
}

fn answer<T>(outcome: PromptOutcome<T>) -> Result<T> {
    match outcome {
        PromptOutcome::Submitted(answer) => Ok(answer),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => {
            Err(Error::new("Input ended before every question was answered"))
        }
    }
}
