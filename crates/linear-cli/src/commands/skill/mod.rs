//! `linear skill`: the guide that teaches a coding agent to use this CLI, and
//! the small skill file that points an agent at it.
//!
//! The guide ships in the binary, so what an agent reads always matches the
//! commands it can run. The installed skill is a stub that says to run
//! `linear skill show`; it changes only if that instruction does. None of
//! this needs configuration, credentials or the network; `app` runs it
//! before any of those load.
use std::collections::BTreeSet;

use crate::cli;
use crate::cli::skill::{Agent, SkillCommand, SkillRemove, SkillSetup, SkillStatus};
use crate::commands::json;
use crate::config::RealFileSource;
use crate::error::{Error, Result};
use crate::platform::output::{self, Stdout};

mod apply;
mod guide;
mod plan;
mod report;
mod slots;

#[cfg(test)]
mod tests;

use apply::{Files, RealFiles};
use plan::{Change, State, Step, Stubs};
use report::Action;
use slots::{Layout, Scope, SkillEnv};

pub fn run(command: &SkillCommand) -> Result<()> {
    match command {
        SkillCommand::Show(_) => show(),
        SkillCommand::Setup(args) => {
            let (stdout, stderr, result) = setup(
                args,
                &SkillEnv::capture()?,
                Stubs::SHIPPED,
                &RealFiles,
                &stamp(),
            )?;
            print(&stdout, &stderr)?;
            result
        }
        SkillCommand::Status(args) => {
            let (stdout, stderr, result) = status(args, &SkillEnv::capture()?, Stubs::SHIPPED)?;
            print(&stdout, &stderr)?;
            result
        }
        SkillCommand::Remove(args) => {
            let (stdout, stderr, result) =
                remove(args, &SkillEnv::capture()?, Stubs::SHIPPED, &RealFiles)?;
            print(&stdout, &stderr)?;
            result
        }
    }
}

/// A command's report for stdout, its notes for stderr, and whether it
/// succeeded. The report is printed either way.
type Outcome = (Vec<u8>, String, Result<()>);

fn print(stdout: &[u8], stderr: &str) -> Result<()> {
    let out = Stdout::new();
    out.write(stdout)?;
    out.flush()?;
    output::eprint(stderr.as_bytes())
}

/// Prints the guide as raw Markdown: no pager, color or wrapping, so an agent
/// reads all of it in one call.
fn show() -> Result<()> {
    let stdout = Stdout::new();
    stdout.write(guide::render(&cli::command()).as_bytes())?;
    stdout.flush()
}

/// The name of this run's backup directory: the time, to the second.
fn stamp() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

fn layout(env: &SkillEnv, project: bool) -> Result<Layout> {
    if project {
        Layout::project(env, &RealFileSource)
    } else {
        Layout::user(env)
    }
}

fn setup(
    args: &SkillSetup,
    env: &SkillEnv,
    stubs: Stubs,
    files: &impl Files,
    stamp: &str,
) -> Result<Outcome> {
    let layout = layout(env, args.project)?;
    let agents = layout.setup_agents(&args.agent);
    let mut steps = plan::setup(&layout, &agents, args.force, stubs, stamp);
    if !args.dry_run {
        apply::apply(&mut steps, stubs, files);
    }
    let stdout = if args.json {
        json::render(&report::changes_json(
            &layout,
            Action::Setup,
            args.dry_run,
            &steps,
        ))
    } else {
        report::changes_text(&steps, args.dry_run, env.home.as_deref()).into_bytes()
    };

    let mut notes = Vec::new();
    if args.dry_run {
        notes.push("Dry run: nothing was changed.".to_owned());
    }
    if layout.scope == Scope::User && args.agent.is_empty() && !layout.claude_present {
        notes.push(
            "Claude Code's directory was not found, so its skill was not installed. \
             `linear skill setup --agent claude` installs it anyway."
                .to_owned(),
        );
    }
    if let Some(note) = twice(&steps) {
        notes.push(note);
    }
    let installed = steps.iter().any(|step| {
        matches!(
            step.change,
            Change::Created | Change::Updated | Change::Replaced
        )
    });
    if installed && !args.dry_run {
        if !env.linear_on_path() {
            notes.push(
                "linear is not on PATH. The skill tells agents to run `linear skill show`, \
                 so they need it there."
                    .to_owned(),
            );
        }
        if layout.scope == Scope::Project && agents.contains(&Agent::Pi) {
            notes.push("Pi reads a project's skills only after the project is trusted.".to_owned());
        }
    }
    Ok((stdout, note_lines(&notes), setup_result(&steps, args)))
}

/// A note for a machine where one agent reads two of the directories the
/// skill is in, as when an agent's own skills directory is a symlink to
/// another's: that agent will list the skill twice.
fn twice(steps: &[Step]) -> Option<String> {
    let holding: Vec<&Step> = steps
        .iter()
        .filter(|step| {
            step.slot.location == slots::Location::Canonical
                && matches!(
                    step.change,
                    Change::Created | Change::Updated | Change::Unchanged | Change::Replaced
                )
        })
        .collect();
    let agents: Vec<Agent> = Agent::ALL
        .into_iter()
        .filter(|agent| {
            holding
                .iter()
                .filter(|step| step.slot.agents.contains(agent))
                .count()
                > 1
        })
        .collect();
    (!agents.is_empty()).then(|| {
        format!(
            "{} read more than one of these directories on this machine, so the skill will \
             show up twice there. `--agent` installs into one of them.",
            plan::titles(&agents)
        )
    })
}

fn setup_result(steps: &[Step], args: &SkillSetup) -> Result<()> {
    if let Some(failed) = steps.iter().find(|step| step.change == Change::Failed) {
        return Err(Error::new(format!(
            "Failed to install the skill: {}",
            failed.note.as_deref().unwrap_or("the reason is unknown")
        ))
        .with_hint("Fix the problem and run the command again; it carries on where it stopped"));
    }
    let conflicts = steps
        .iter()
        .filter(|step| step.change == Change::Conflict)
        .count();
    if conflicts == 0 {
        return Ok(());
    }
    let forceable = steps.iter().any(|step| {
        step.change == Change::Conflict
            && step.seen.state == State::Foreign
            && step.slot.location != slots::Location::Nested
    });
    let error = Error::new(format!(
        "{conflicts} linear-cli {} left alone",
        if conflicts == 1 {
            "skill was"
        } else {
            "skills were"
        }
    ));
    Err(if forceable && !args.force {
        error.with_hint(
            "`linear skill setup --force` moves them to a backup directory and installs the \
             skill; add --dry-run to see what it would do",
        )
    } else {
        error.with_hint("Move or remove them by hand, then run the command again")
    })
}

fn status(args: &SkillStatus, env: &SkillEnv, stubs: Stubs) -> Result<Outcome> {
    let layout = layout(env, args.project)?;
    let items = plan::status(&layout, stubs);
    let stdout = if args.json {
        json::render(&report::status_json(&layout, &items))
    } else {
        report::status_text(&items, env.home.as_deref()).into_bytes()
    };
    let project = if args.project { " --project" } else { "" };
    let mut notes = Vec::new();
    // The slots a plain `setup` would write: not Claude Code's on a machine
    // without Claude Code.
    let wanted = layout.setup_agents(&[]);
    let needs_setup = items.iter().any(|item| {
        item.slot.location == slots::Location::Canonical
            && matches!(item.seen.state, State::Missing | State::Outdated)
            && item.slot.agents.iter().any(|agent| wanted.contains(agent))
    });
    if needs_setup {
        notes.push(format!(
            "`linear skill setup{project}` installs or updates the skill."
        ));
    }
    if items.iter().any(|item| item.seen.state == State::Foreign) {
        notes.push(format!(
            "`linear skill setup{project} --force` moves a foreign skill to a backup directory \
             and installs over it."
        ));
    }
    let result = if items
        .iter()
        .any(|item| item.seen.state == State::Unreadable)
    {
        Err(Error::new("A skill directory could not be read"))
    } else {
        Ok(())
    };
    Ok((stdout, note_lines(&notes), result))
}

fn remove(args: &SkillRemove, env: &SkillEnv, stubs: Stubs, files: &impl Files) -> Result<Outcome> {
    let layout = layout(env, args.project)?;
    let agents: BTreeSet<Agent> = if args.agent.is_empty() {
        Agent::ALL.into_iter().collect()
    } else {
        args.agent.iter().copied().collect()
    };
    let mut steps = plan::remove(&layout, &agents, stubs);
    if !args.dry_run {
        apply::apply(&mut steps, stubs, files);
    }
    let stdout = if args.json {
        json::render(&report::changes_json(
            &layout,
            Action::Remove,
            args.dry_run,
            &steps,
        ))
    } else {
        report::changes_text(&steps, args.dry_run, env.home.as_deref()).into_bytes()
    };
    let mut notes = Vec::new();
    if args.dry_run {
        notes.push("Dry run: nothing was changed.".to_owned());
    }
    let result = match steps.iter().find(|step| step.change.failed()) {
        Some(step) => Err(Error::new(format!(
            "Failed to remove the skill at {}: {}",
            step.slot.path.display(),
            step.note.as_deref().unwrap_or("the reason is unknown")
        ))),
        None => Ok(()),
    };
    Ok((stdout, note_lines(&notes), result))
}

fn note_lines(notes: &[String]) -> String {
    notes.iter().map(|note| format!("{note}\n")).collect()
}
