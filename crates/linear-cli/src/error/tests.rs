use super::*;

const ALL: [Failure; 4] = [
    Failure::General,
    Failure::NotFound,
    Failure::Auth,
    Failure::Unavailable,
];

#[test]
fn combining_keeps_the_class_that_needs_attention_first() {
    use Failure::{Auth, General, NotFound, Unavailable};
    for (a, b, expected) in [
        (NotFound, NotFound, NotFound),
        (NotFound, General, General),
        (NotFound, Unavailable, Unavailable),
        (NotFound, Auth, Auth),
        (General, Unavailable, Unavailable),
        (General, Auth, Auth),
        (Unavailable, Auth, Auth),
    ] {
        assert_eq!(a.combine(b), expected, "{a:?} + {b:?}");
        assert_eq!(b.combine(a), expected, "{b:?} + {a:?}");
    }
    for failure in ALL {
        assert_eq!(failure.combine(failure), failure);
    }
}

#[test]
fn folding_starts_from_the_first_failure() {
    assert_eq!(Failure::fold([]), None);
    assert_eq!(
        Failure::fold([Failure::NotFound, Failure::NotFound]),
        Some(Failure::NotFound)
    );
    assert_eq!(
        Failure::fold([Failure::NotFound, Failure::Unavailable, Failure::General]),
        Some(Failure::Unavailable)
    );
}

#[test]
fn the_class_survives_context_hints_and_message_changes() {
    let wrapped =
        || -> Result<()> { Err(Error::not_found("Issue", "ENG-1").with_hint("Check the ID.")) };
    let mut error = wrapped()
        .context("Failed to view issue")
        .expect_err("fails");
    error.push_message("; more");
    assert_eq!(error.failure(), Some(Failure::NotFound));
    assert_eq!(error.exit_code(), 3);
    assert_eq!(Error::auth("No API key configured").exit_code(), 4);
    assert_eq!(Error::reported(Failure::Unavailable).exit_code(), 5);
    assert_eq!(Error::invalid("bad").failure(), None);
}
