//! Users, referenced by `@me` (or `self`), email, display name or part of a
//! name.
use crate::cli::values::UserRef;
use crate::client::LinearClient;
use crate::error::{Error, Result};
use crate::graphql::operations::user::{
    AgentUser, GetViewerId, ListAgentUsers, ListAgentUsersVariables, LookupUser, LookupUserNode,
    LookupUserVariables,
};
use crate::graphql::pagination::{self, Page};

/// The ID of the user `user` names. `role` names the user's part in the
/// not-found error, like "Owner".
pub async fn resolve(client: &LinearClient, user: &UserRef, role: &str) -> Result<String> {
    let input = match user {
        UserRef::Me => {
            let data: GetViewerId = client.query(()).await?;
            return Ok(data.viewer.id.into_inner());
        }
        UserRef::Query(input) => input.as_str(),
    };
    let data: LookupUser = client
        .query(LookupUserVariables {
            input: input.to_owned(),
        })
        .await?;
    select(&data.users.nodes, input, role).map(|user| user.id.inner().to_owned())
}

/// The ID of the agent `user` names, for an issue's delegate. Only agent (app)
/// users can be delegates, so only they are searched; `@me` is the viewer
/// when the viewer is an agent.
pub async fn resolve_agent(client: &LinearClient, user: &UserRef) -> Result<String> {
    let agents = pagination::collect(None, |after, first| async move {
        let data: ListAgentUsers = client
            .query(ListAgentUsersVariables { first, after })
            .await?;
        Ok(Page {
            nodes: data.users.nodes,
            page_info: data.users.page_info,
        })
    })
    .await?;
    select_agent(&agents, user).map(|agent| agent.id.inner().to_owned())
}

/// For a UUID, the agent with that ID. Otherwise an exact email, else an
/// exact display name, else an exact name, else a partial name match, each
/// tier unique like [`select`].
fn select_agent<'a>(agents: &'a [AgentUser], user: &UserRef) -> Result<&'a AgentUser> {
    let input = match user {
        UserRef::Me => {
            return agents.iter().find(|agent| agent.is_me).ok_or_else(|| {
                Error::invalid("Cannot delegate to @me: you are not an agent user").with_hint(
                    format!(
                        "Use --assignee @me to assign yourself. {}",
                        agent_list(agents)
                    ),
                )
            });
        }
        UserRef::Query(input) => input.as_str(),
    };
    let wanted = input.to_lowercase();
    let by_id: [&dyn Fn(&AgentUser) -> bool; 1] = [&|agent| agent.id.inner() == wanted];
    let by_name: [&dyn Fn(&AgentUser) -> bool; 4] = [
        &|agent| agent.email.to_lowercase() == wanted,
        &|agent| agent.display_name.to_lowercase() == wanted,
        &|agent| agent.name.to_lowercase() == wanted,
        &|agent| agent.name.to_lowercase().contains(&wanted),
    ];
    let tiers: &[&dyn Fn(&AgentUser) -> bool] = if super::is_linear_uuid(input) {
        &by_id
    } else {
        &by_name
    };
    for tier in tiers {
        let matches: Vec<&AgentUser> = agents.iter().filter(|agent| tier(agent)).collect();
        match matches.as_slice() {
            [] => continue,
            [agent] => return Ok(agent),
            several => {
                return Err(super::ambiguous(
                    "Delegate",
                    input,
                    several.iter().map(|agent| {
                        format!(
                            "{} ({}, {}, {})",
                            agent.name,
                            agent.display_name,
                            agent.email,
                            agent.id.inner()
                        )
                    }),
                )
                .with_hint("Pass the agent's email or ID instead."));
            }
        }
    }
    Err(Error::not_found("Delegate", input).with_hint(format!(
        "Delegates are agent users; use --assignee for people. {}",
        agent_list(agents)
    )))
}

/// The workspace's agents, for a hint.
fn agent_list(agents: &[AgentUser]) -> String {
    if agents.is_empty() {
        return "This workspace has no agent users.".to_owned();
    }
    let names: Vec<String> = agents
        .iter()
        .map(|agent| format!("{} ({})", agent.name, agent.display_name))
        .collect();
    format!("Agents in this workspace: {}.", names.join(", "))
}

/// A user given as a flag, looked up together with the other flags, or one
/// already found at a prompt.
pub enum UserChoice {
    Given(UserRef),
    Found(String),
}

impl UserChoice {
    pub async fn id(self, client: &LinearClient, role: &str) -> Result<String> {
        match self {
            Self::Given(user) => resolve(client, &user, role).await,
            Self::Found(id) => Ok(id),
        }
    }
}

/// An exact email match, else an exact display name, else a partial name
/// match. Each tier must be unique; several matches at the first tier that
/// has any are ambiguous.
fn select<'a>(users: &'a [LookupUserNode], input: &str, role: &str) -> Result<&'a LookupUserNode> {
    let wanted = input.to_lowercase();
    let tiers: [&dyn Fn(&LookupUserNode) -> bool; 3] = [
        &|user| user.email.to_lowercase() == wanted,
        &|user| user.display_name.to_lowercase() == wanted,
        &|_| true,
    ];
    for tier in tiers {
        let matches: Vec<&LookupUserNode> = users.iter().filter(|user| tier(user)).collect();
        match matches.as_slice() {
            [] => continue,
            [user] => return Ok(user),
            several => {
                return Err(super::ambiguous(
                    role,
                    input,
                    several.iter().map(|user| {
                        format!("{} ({}, {})", user.name, user.display_name, user.email)
                    }),
                )
                .with_hint("Pass the user's email address instead."));
            }
        }
    }
    Err(Error::not_found(role, input))
}

#[cfg(test)]
mod tests {
    use super::select;
    use crate::graphql::operations::user::LookupUserNode;

    fn user(id: &str, name: &str, display_name: &str, email: &str) -> LookupUserNode {
        LookupUserNode {
            id: cynic::Id::new(id),
            email: email.to_owned(),
            display_name: display_name.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn an_exact_email_or_display_name_beats_partial_name_matches() {
        let users = [
            user("1", "Sam Lee", "sam", "sam@example.com"),
            user("2", "Samantha Ray", "samr", "samantha@example.com"),
        ];
        assert_eq!(
            select(&users, "SAM", "User").expect("display").id.inner(),
            "1"
        );
        assert_eq!(
            select(&users, "samantha@example.com", "User")
                .expect("email")
                .id
                .inner(),
            "2"
        );
    }

    #[test]
    fn several_matches_in_the_deciding_tier_are_ambiguous() {
        let users = [
            user("1", "Sam Lee", "sam", "lee@example.com"),
            user("2", "Sam Ray", "sam", "ray@example.com"),
        ];
        let error = select(&users, "sam", "Assignee").expect_err("two display names");
        assert_eq!(
            error.message(),
            "Assignee \"sam\" is ambiguous; it matches:\n  Sam Lee (sam, lee@example.com)\n  Sam Ray (sam, ray@example.com)"
        );
        assert_eq!(error.hint(), Some("Pass the user's email address instead."));

        let partial = [
            user("1", "Sam Lee", "slee", "lee@example.com"),
            user("2", "Sam Ray", "sray", "ray@example.com"),
        ];
        select(&partial, "Sam", "User").expect_err("two partial matches");
    }

    #[test]
    fn no_match_is_not_found_under_the_role() {
        let error = select(&[], "nobody", "Lead").expect_err("none");
        assert_eq!(error.message(), "Lead not found: nobody");
    }
}
