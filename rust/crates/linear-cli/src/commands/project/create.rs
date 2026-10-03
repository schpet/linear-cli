//! `project create`: fields from flags or prompts, then one mutation, then
//! the optional initiative link.
use chrono::NaiveDate;

use super::common;
use crate::cli::project::{ProjectCreate, Status};
use crate::cli::values::{self, Priority};
use crate::client::LinearClient;
use crate::commands::outcome;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::project::ProjectStatusType;
use crate::graphql::operations::project::{
    AddProjectToInitiative, CreateProject, CreateProjectVariables, CreatedProject,
    InitiativeLinkInput, LinkVariables, ProjectCreateInput,
};
use crate::graphql::scalars::TimelessDate;
use crate::platform::prompt::{Choice, Prompter, Text};
use crate::platform::style;
use crate::refs::{self, initiative::InitiativeReference, team::ResolvedTeam};

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
    let description = common::description(fields)?;
    let content = common::content(fields)?;
    common::plain_references(
        fields.lead.iter().chain(&args.member),
        "an email, username, display name, or @me",
    )?;
    common::plain_references(&args.label, "a project label name")?;
    common::plain_references(&args.template, "a template name or UUID")?;
    let scope = ctx.scope()?;
    let initiative = match args.initiative.as_deref() {
        Some(original) => Some((original, InitiativeReference::parse(original, &scope)?)),
        None => None,
    };
    let optional = ctx.optional_prompts(args.interactive)?;
    let configured_team = configured_team_key(ctx.options());
    let missing = fields.name.is_none() || (args.team.is_empty() && configured_team.is_none());
    let interactive = ctx.interactive() && (missing || optional);
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
        prompt(
            ctx,
            &ctx.prompter()?,
            &mut draft,
            fields.description_file.is_some(),
            optional,
        )?;
    }
    let name = draft
        .name
        .ok_or_else(|| Error::new("Project name is required").with_hint("Pass --name."))?;
    let teams = if draft.teams.is_empty() {
        vec![configured_team.ok_or_else(|| {
            Error::new("At least one team is required")
                .with_hint("Pass --team, or run `linear config` to set a default team.")
        })?]
    } else {
        draft.teams
    };
    let teams = common::prepare_teams(&teams, &scope)?;
    let client = ctx.client()?;
    let (project, linked) = ctx.spin(!args.json, async {
        let team_ids: Vec<_> = common::teams(client, &teams)
            .await?
            .into_iter()
            .map(|team| team.id)
            .collect();
        let initiative_id = match &initiative {
            Some((original, reference)) => Some((
                *original,
                refs::initiative::resolve(client, reference, refs::initiative::Archived::Exclude)
                    .await?,
            )),
            None => None,
        };
        let template_id = match &args.template {
            Some(template) => Some(common::template(client, template, &team_ids).await?),
            None => None,
        };
        let lead_id = match &draft.lead {
            Some(lead) => Some(refs::user::resolve(client, lead, "Lead").await?),
            None => None,
        };
        let status_id = match draft.status {
            Some(StatusChoice::Kind(status)) => Some(common::status_id(client, status).await?),
            Some(StatusChoice::Id(id)) => Some(id),
            None => None,
        };
        let label_ids: Vec<_> = common::labels(client, &args.label)
            .await?
            .into_iter()
            .map(|label| label.id)
            .collect();
        let mut member_ids = Vec::new();
        for member in &args.member {
            member_ids.push(refs::user::resolve(client, member, "User").await?);
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
        ctx.print(crate::commands::json::render(&project))?;
    } else {
        ctx.print(outcome::done(
            "Created",
            "project",
            &project.name,
            Some(&project.url),
        ))?;
    }
    let Some((initiative, linked)) = linked else {
        return Ok(());
    };
    match linked {
        Ok(()) if args.json => Ok(()),
        Ok(()) => ctx.print(outcome::done(
            "Added",
            "project to initiative",
            initiative,
            None,
        )),
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

async fn submit(client: &LinearClient, input: ProjectCreateInput) -> Result<CreatedProject> {
    let result: CreateProject = client
        .mutate(CreateProjectVariables { input })
        .await
        .map_err(|failure| failure.into_create_error("project"))?;
    let payload = result.project_create;
    match payload.project {
        Some(project) if payload.success => Ok(project),
        Some(_) | None => Err(Error::new("Linear did not create the project")),
    }
}

async fn link(
    client: &LinearClient,
    project: &CreatedProject,
    initiative_id: String,
) -> Result<()> {
    let result: AddProjectToInitiative = client
        .mutate(LinkVariables {
            input: InitiativeLinkInput {
                initiative_id,
                project_id: project.id.inner().to_owned(),
            },
        })
        .await?;
    if result.initiative_to_project_create.success {
        Ok(())
    } else {
        Err(Error::new("Linear rejected the link"))
    }
}

/// The team picker's choices (by key) and the configured team's position.
fn team_choices(
    teams: Vec<ResolvedTeam>,
    configured: Option<String>,
) -> Result<(Vec<Choice<String>>, usize)> {
    if teams.is_empty() {
        return Err(refs::team::none_accessible());
    }
    let start = teams
        .iter()
        .position(|team| Some(&team.key) == configured.as_ref())
        .unwrap_or(0);
    let choices = teams
        .into_iter()
        .map(|team| Choice::new(format!("{} ({})", team.name, team.key), team.key))
        .collect();
    Ok((choices, start))
}

/// Asks for the name and team when they are missing (a configured team
/// counts), and with `optional` for every other field the flags left out.
fn prompt(
    ctx: &Ctx,
    prompter: &Prompter<'_>,
    draft: &mut Draft,
    description_file: bool,
    optional: bool,
) -> Result<()> {
    if draft.name.is_none() {
        draft.name = Some(prompter.text(Text::new("Project name:").required())?);
    }
    let default_team = configured_team_key(ctx.options());
    if draft.teams.is_empty() && (optional || default_team.is_none()) {
        let teams = ctx.spin(true, refs::team::fetch_all(ctx.client()?))?;
        let (choices, start) = team_choices(teams, default_team)?;
        draft.teams = vec![prompter.select_from("Team:", choices, start)?];
    }
    if !optional {
        return Ok(());
    }
    if draft.description.is_none() && !description_file {
        let description = prompter.text(Text::new("Description (optional):"))?;
        draft.description = (!description.is_empty()).then_some(description);
    }
    if draft.status.is_none() {
        let statuses = ctx.spin(true, common::statuses(ctx.client()?))?;
        if !statuses.is_empty() {
            let start = statuses
                .iter()
                .position(|status| status.status_type == ProjectStatusType::Planned)
                .unwrap_or(0);
            let choices = statuses
                .into_iter()
                .map(|status| Choice::new(status.name, status.id.into_inner()))
                .collect();
            let status = prompter.select_from("Status:", choices, start)?;
            draft.status = Some(StatusChoice::Id(status));
        }
    }
    if draft.lead.is_none() {
        let plain = |lead: &str| {
            refs::reject_linear_url(lead, "an email, username, display name, or @me")
                .map_err(|error| error.message().to_owned())
        };
        let lead = prompter.text(
            Text::new("Lead (username, email, or @me - press Enter to skip):").with_check(&plain),
        )?;
        draft.lead = (!lead.is_empty()).then_some(lead);
    }
    for (field, message) in [
        (
            &mut draft.start_date,
            "Start date (YYYY-MM-DD - press Enter to skip):",
        ),
        (
            &mut draft.target_date,
            "Target date (YYYY-MM-DD - press Enter to skip):",
        ),
    ] {
        if field.is_none() {
            *field = prompter.parsed(Text::new(message), &values::date)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::team_choices;
    use crate::refs::team::ResolvedTeam;

    #[test]
    fn no_accessible_teams_is_an_error_not_an_empty_picker() {
        let Err(error) = team_choices(Vec::new(), Some("ENG".to_owned())) else {
            panic!("an empty team list must not reach the picker");
        };
        assert_eq!(
            error.message(),
            "This workspace has no teams you can access"
        );
        assert!(error.hint().is_some());
    }

    #[test]
    fn the_picker_starts_on_the_configured_team() {
        let team = |key: &str| ResolvedTeam {
            id: format!("id-{key}"),
            key: key.to_owned(),
            name: key.to_owned(),
        };
        let (choices, start) =
            team_choices(vec![team("A"), team("ENG")], Some("ENG".to_owned())).expect("teams");
        assert_eq!(choices.len(), 2);
        assert_eq!(start, 1);
    }
}
