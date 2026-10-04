//! Users, referenced by `@me` (or `self`), email, display name or part of a
//! name.
use crate::cli::values::UserRef;
use crate::client::LinearClient;
use crate::error::{Error, Result};
use crate::graphql::operations::user::{
    GetViewerId, LookupUser, LookupUserNode, LookupUserVariables,
};

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
