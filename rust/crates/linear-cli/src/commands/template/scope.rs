//! Choosing a template for `issue create` or `project create`: by ID or name,
//! limited to templates of the right kind that the target teams can use.
use crate::{error::Error, graphql::operations::template::Template};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateScope {
    Issue,
    Project,
}

impl TemplateScope {
    pub fn word(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Project => "project",
        }
    }
    fn article(self) -> &'static str {
        match self {
            Self::Issue => "an",
            Self::Project => "a",
        }
    }
}

fn available(template: &Template, team_ids: &[String]) -> bool {
    template
        .team
        .as_ref()
        .is_none_or(|team| team_ids.iter().any(|id| id == team.id.inner()))
}

fn wrong_type(template: &Template, scope: TemplateScope) -> Error {
    let article = if template
        .template_type
        .chars()
        .next()
        .is_some_and(|c| "aeiouAEIOU".contains(c))
    {
        "an"
    } else {
        "a"
    };
    Error::new(format!(
        "Template \"{}\" is {article} {} template, not {} {} template",
        template.name,
        template.template_type,
        scope.article(),
        scope.word()
    ))
    .with_hint(format!(
        "Run `linear template list --type {}` to see the {} templates.",
        scope.word(),
        scope.word()
    ))
}

/// `keys` names the teams the template belongs to; it is never empty.
fn wrong_team(name: &str, keys: &[String], scope: TemplateScope) -> Error {
    let first = keys.first().expect("a team template has a team key");
    let plural = if keys.len() == 1 { "" } else { "s" };
    Error::new(format!(
        "Template \"{name}\" belongs to team{plural} {} and cannot be applied here",
        keys.join(", ")
    ))
    .with_hint(format!(
        "Pass --team {first}, or pick a workspace template or one from the target team with \
         `linear template list --type {} --team <team>`.",
        scope.word()
    ))
}

pub fn assert_scope(
    template: &Template,
    team_ids: &[String],
    scope: TemplateScope,
) -> Result<(), Error> {
    if template.template_type != scope.word() {
        return Err(wrong_type(template, scope));
    }
    if !available(template, team_ids) {
        let team = template
            .team
            .as_ref()
            .expect("only team templates can be unavailable");
        return Err(wrong_team(
            &template.name,
            std::slice::from_ref(&team.key),
            scope,
        ));
    }
    Ok(())
}

pub fn select(
    reference: &str,
    all: Vec<Template>,
    team_ids: &[String],
    scope: TemplateScope,
) -> Result<Template, Error> {
    let matches: Vec<_> = all
        .iter()
        .filter(|t| t.name.to_lowercase() == reference.to_lowercase())
        .collect();
    let eligible = |t: &Template| t.template_type == scope.word() && available(t, team_ids);
    let candidates: Vec<_> = matches.iter().copied().filter(|t| eligible(t)).collect();
    if candidates.len() > 1 {
        return Err(Error::new(format!(
            "Template name \"{reference}\" is ambiguous: it matches {} templates",
            candidates.len()
        ))
        .with_hint(format!(
            "Pass the template ID instead: {}",
            candidates
                .iter()
                .map(|t| format!(
                    "{} ({}, {})",
                    t.id.inner(),
                    t.template_type,
                    t.team
                        .as_ref()
                        .map_or("Workspace", |team| team.key.as_str())
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if let Some(t) = candidates.first() {
        return Ok((*t).clone());
    }
    if let Some(first) = matches.first() {
        let same: Vec<_> = matches
            .iter()
            .copied()
            .filter(|t| t.template_type == scope.word())
            .collect();
        let mut keys = Vec::new();
        for t in &same {
            if let Some(team) = &t.team
                && !keys.contains(&team.key)
            {
                keys.push(team.key.clone())
            }
        }
        if !keys.is_empty() {
            return Err(wrong_team(
                &same
                    .first()
                    .unwrap_or_else(|| unreachable!("team keys imply a matching template"))
                    .name,
                &keys,
                scope,
            ));
        }
        return Err(wrong_type(first, scope));
    }
    let mut names = Vec::new();
    for template in all.iter().filter(|t| eligible(t)) {
        if !names.contains(&template.name) {
            names.push(template.name.clone())
        }
    }
    names.sort_by(|a, b| crate::platform::collation::compare(a, b));
    let suggestion = if names.is_empty() {
        format!(
            "No {} templates are available here. Run `linear template list` to see every template.",
            scope.word()
        )
    } else {
        format!(
            "Available {} templates: {}. Run `linear template list` to see every template.",
            scope.word(),
            names
                .iter()
                .map(|n| format!("\"{n}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Err(Error::not_found("Template", reference).with_hint(suggestion))
}
