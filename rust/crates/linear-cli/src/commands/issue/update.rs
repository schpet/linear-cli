//! `issue update`: change an issue's fields.
use super::write::{self as shared, Backend};
use crate::{
    cli::issue::IssueUpdate,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::{edit::Edit, operations::issue_update::IssueUpdateInput, scalars::TimelessDate},
};

pub fn run(ctx: &Ctx, args: &IssueUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update issue")
}

fn update(ctx: &Ctx, args: &IssueUpdate) -> Result<()> {
    let fields = Fields::from(args);
    let description = fields.local()?;
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let backend = super::create::backend(ctx)?;
    let changes = ctx.spin(true, input(&backend, &identifier, &fields, description))?;
    ctx.print(header(&identifier))?;
    let issue = ctx.spin(true, backend.update(identifier, changes))?;
    ctx.print(output(&issue))
}
#[derive(Clone, Debug, Default)]
pub struct Fields {
    pub title: Option<String>,
    pub assignee: Option<String>,
    pub unassign: bool,
    pub due_date: Option<String>,
    pub clear_due_date: bool,
    pub parent: Option<String>,
    pub clear_parent: bool,
    pub priority: Option<f64>,
    pub estimate: Option<f64>,
    pub clear_estimate: bool,
    pub description: Option<String>,
    pub description_file: Option<String>,
    pub labels: Option<Vec<String>>,
    pub add_labels: Option<Vec<String>>,
    pub remove_labels: Option<Vec<String>>,
    pub team: Option<String>,
    pub project: Option<String>,
    pub clear_project: bool,
    pub state: Option<String>,
    pub milestone: Option<String>,
    pub clear_milestone: bool,
    pub cycle: Option<String>,
    pub clear_cycle: bool,
}
impl Fields {
    /// Flag conflicts, checked before reading files, inferring an issue or any request.
    pub fn local(&self) -> Result<Option<String>, Error> {
        let conflicts = [
            (
                self.unassign && self.assignee.is_some(),
                "Cannot specify both --assignee and --unassign",
                "Use --assignee <user> to set an assignee, or --unassign on its own to clear it.",
            ),
            (
                self.clear_cycle && self.cycle.is_some(),
                "Cannot specify both --cycle and --clear-cycle",
                "Use --cycle <cycle> to set a cycle, or --clear-cycle on its own to remove it.",
            ),
            (
                self.clear_due_date && self.due_date.is_some(),
                "Cannot specify both --due-date and --clear-due-date",
                "Use --due-date <date> to set a due date, or --clear-due-date on its own to remove it.",
            ),
            (
                self.clear_estimate && self.estimate.is_some(),
                "Cannot specify both --estimate and --clear-estimate",
                "Use --estimate <points> to set an estimate, or --clear-estimate on its own to remove it.",
            ),
            (
                self.clear_parent && self.parent.is_some(),
                "Cannot specify both --parent and --clear-parent",
                "Use --parent <issue> to set a parent, or --clear-parent on its own to remove it.",
            ),
            (
                self.clear_project && self.project.is_some(),
                "Cannot specify both --project and --clear-project",
                "Use --project <project> to set a project, or --clear-project on its own to remove it.",
            ),
            (
                self.clear_project && self.milestone.is_some(),
                "Cannot specify --milestone while clearing the issue's project",
                "Drop --milestone, or replace it with --clear-milestone to remove both the project and the milestone.",
            ),
            (
                self.clear_milestone && self.milestone.is_some(),
                "Cannot specify both --milestone and --clear-milestone",
                "Use --milestone <milestone> to set a milestone, or --clear-milestone on its own to remove it.",
            ),
            (
                self.labels.is_some()
                    && (self.add_labels.is_some() || self.remove_labels.is_some()),
                "Cannot combine --label with --add-label or --remove-label",
                "--label replaces the issue's entire label set. Use it alone to set the exact set, or use --add-label/--remove-label alone to change it incrementally.",
            ),
            (
                self.team.is_some() && (self.add_labels.is_some() || self.remove_labels.is_some()),
                "Cannot combine --team with --add-label or --remove-label",
                "Move the issue with --team first, then change labels in a second update.",
            ),
        ];
        for (conflict, message, suggestion) in conflicts {
            if conflict {
                return Err(shared::validation(message).with_hint(suggestion));
            }
        }
        shared::description(
            self.description.as_deref(),
            self.description_file.as_deref(),
        )
    }
}
async fn labels<B: Backend>(
    backend: &B,
    team: &str,
    names: Option<&[String]>,
) -> Result<Vec<String>, Error> {
    let mut ids = Vec::new();
    for name in names.unwrap_or_default() {
        let id = backend
            .label(team.to_owned(), name.clone())
            .await?
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                Error::not_found("Issue label", name).with_hint(format!(
                    "Run `linear label list --team {team}` to see available labels."
                ))
            })?;
        if !ids.contains(&id) {
            ids.push(id)
        }
    }
    Ok(ids)
}
pub async fn input<B: Backend>(
    backend: &B,
    issue_id: &str,
    fields: &Fields,
    description: Option<String>,
) -> Result<IssueUpdateInput, Error> {
    // Always resolve team, including a no-flags update. No no-op shortcut.
    let reference = fields
        .team
        .clone()
        .or_else(|| issue_id.rsplit_once('-').map(|(key, _)| key.to_owned()))
        .ok_or_else(|| shared::validation("Could not determine team key from issue ID"))?;
    let team = backend.team(reference).await?;
    let state = match &fields.state {
        Some(value) => Some(backend.state(team.key.clone(), value.clone()).await?),
        None => None,
    };
    let assignee = match &fields.assignee {
        Some(value) => {
            let id = backend.user(value.clone()).await?;
            if id.is_empty() {
                return Err(Error::not_found("User", value));
            }
            Some(id)
        }
        None => None,
    };
    let replacements = labels(backend, &team.key, fields.labels.as_deref()).await?;
    let added = labels(backend, &team.key, fields.add_labels.as_deref()).await?;
    let removed = labels(backend, &team.key, fields.remove_labels.as_deref()).await?;
    if added.iter().any(|id| removed.contains(id)) {
        return Err(
            shared::validation("Cannot add and remove the same label in one update")
                .with_hint("Remove the duplicate label from either --add-label or --remove-label."),
        );
    }
    let project = match &fields.project {
        Some(value) => Some(backend.project(value.clone()).await?.ok_or_else(|| {
            Error::not_found("Project", value).with_hint(
                "Pass a project UUID, slug ID (from `linear project list`), or exact project name.",
            )
        })?),
        None => None,
    };
    let milestone = match &fields.milestone {
        Some(value) if crate::refs::is_linear_uuid(value) => Some(value.clone()),
        Some(value) => {
            let project = match &project {
                Some(id) => Some(id.clone()),
                None => backend.issue_project(issue_id.to_owned()).await?,
            };
            let project=project.ok_or_else(||shared::validation("--milestone requires --project to be set (issue has no existing project)")
                .with_hint("Use --project to specify the project for the milestone, or pass a milestone UUID directly."))?;
            Some(backend.milestone(project, value.clone()).await?)
        }
        None => None,
    };
    let cycle = match &fields.cycle {
        Some(value) => Some(backend.cycle(team.id.clone(), value.clone()).await?),
        None => None,
    };
    // Parent read is deliberately late: after all other resolvers, while input assembles.
    let parent = if fields.clear_parent {
        Edit::Clear
    } else {
        match &fields.parent {
            Some(value) => Edit::Set(backend.parent_id(value.clone()).await?),
            None => Edit::Unchanged,
        }
    };
    Ok(IssueUpdateInput {
        title: Edit::set_or_unchanged(fields.title.clone()),
        assignee_id: shared::edit(fields.unassign, assignee),
        due_date: shared::edit(
            fields.clear_due_date,
            fields.due_date.clone().map(TimelessDate),
        ),
        parent_id: parent,
        priority: Edit::set_or_unchanged(shared::integer(fields.priority, "priority")?),
        estimate: shared::edit(
            fields.clear_estimate,
            shared::integer(fields.estimate, "estimate")?,
        ),
        description: Edit::set_or_unchanged(description),
        label_ids: fields.labels.as_ref().map(|_| replacements),
        added_label_ids: fields.add_labels.as_ref().map(|_| added),
        removed_label_ids: fields.remove_labels.as_ref().map(|_| removed),
        team_id: Edit::Set(team.id),
        project_id: shared::edit(fields.clear_project, project),
        project_milestone_id: shared::edit(fields.clear_milestone, milestone),
        cycle_id: shared::edit(fields.clear_cycle, cycle),
        state_id: Edit::set_or_unchanged(state),
        ..Default::default()
    })
}
pub fn header(issue_id: &str) -> String {
    format!("Updating issue {issue_id}\n\n")
}
pub fn output(issue: &shared::Updated) -> String {
    format!(
        "✓ Updated issue {}: {}\n{}\n",
        issue.identifier, issue.title, issue.url
    )
}

impl From<&crate::cli::issue::IssueUpdate> for Fields {
    fn from(action: &crate::cli::issue::IssueUpdate) -> Self {
        Self {
            title: action.title.clone(),
            assignee: action.assignee.clone(),
            unassign: action.unassign,
            due_date: action.due_date.clone(),
            clear_due_date: action.clear_due_date,
            parent: action.parent.clone(),
            clear_parent: action.clear_parent,
            priority: action.priority,
            estimate: action.estimate,
            clear_estimate: action.clear_estimate,
            description: action.description.clone(),
            description_file: action.description_file.clone(),
            labels: (!action.label.is_empty()).then(|| action.label.clone()),
            add_labels: (!action.add_label.is_empty()).then(|| action.add_label.clone()),
            remove_labels: (!action.remove_label.is_empty()).then(|| action.remove_label.clone()),
            team: action.team.clone(),
            project: action.project.clone(),
            clear_project: action.clear_project,
            state: action.state.clone(),
            milestone: action.milestone.clone(),
            clear_milestone: action.clear_milestone,
            cycle: action.cycle.clone(),
            clear_cycle: action.clear_cycle,
        }
    }
}
