use super::existing_body;
use crate::client::{
    client_to,
    server::{Reply, Server},
};
use serde_json::{Value, json};

const COMMENT_ID: &str = "comment-1";

fn check_lookup(server: Server) {
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    let request = requests.first().expect("one comment lookup");
    let body: Value = serde_json::from_slice(&request.body).expect("GraphQL request");
    assert_eq!(body["operationName"], "GetComment");
    assert_eq!(body["variables"], json!({ "id": COMMENT_ID }));
}

#[tokio::test(flavor = "current_thread")]
async fn missing_comments_fail_before_the_editor_opens() {
    for response in [
        json!({"data": {"comment": null}}),
        json!({"data": null, "errors": [{"message": "Entity not found: Comment"}]}),
    ] {
        let server = Server::start(vec![Reply::json(&response)]);
        let failure = existing_body(&client_to(&server), COMMENT_ID)
            .await
            .expect_err("missing comment");
        assert_eq!(failure.message(), "Comment not found: comment-1");
        check_lookup(server);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn existing_comment_bodies_are_preserved() {
    for (comment, expected) in [
        (json!({}), ""),
        (json!({"body": null}), ""),
        (json!({"body": ""}), ""),
        (
            json!({"body": "  # café\n\nOriginal *body*\n"}),
            "  # café\n\nOriginal *body*\n",
        ),
    ] {
        let server = Server::start(vec![Reply::json(&json!({"data": {"comment": comment}}))]);
        assert_eq!(
            existing_body(&client_to(&server), COMMENT_ID)
                .await
                .expect("existing comment"),
            expected
        );
        check_lookup(server);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn unrelated_comment_lookup_errors_are_preserved() {
    let response = json!({"data": null, "errors": [{"message": "Internal failure", "extensions": {"userPresentableMessage": "Friendly lookup failure"}}]});
    let server = Server::start(vec![Reply::json(&response)]);
    let failure = existing_body(&client_to(&server), COMMENT_ID)
        .await
        .expect_err("lookup failure");
    assert_eq!(failure.message(), "Friendly lookup failure");
    check_lookup(server);
}
