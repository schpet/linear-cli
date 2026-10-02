use std::ffi::OsString;

use cynic::QueryBuilder;
use linear_cli::commands::initiative_view::{markdown, render_json};
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::initiative_view::{
    DetailVariables, GetInitiativeByNameForView, GetInitiativeBySlugForView, GetInitiativeDetails,
    NameVariables, ResolveInitiativeBySlug, SlugVariables, UrlSlugVariables,
};
use serde_json::{Value, json};

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c038-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("frozen case")).expect("case JSON")
}
fn compact(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != ',')
        .collect()
}

#[test]
fn all_four_documents_match_frozen_ast_and_variable_shape() {
    let uuid = "00000000-0000-4000-9000-000000000038";
    fn shape<T: serde::Serialize>(request: GraphQlRequest<T>) -> (String, Value) {
        let variables = serde_json::to_value(&request).unwrap()["variables"].clone();
        (request.query, variables)
    }
    let cases = [
        (
            "c038-uuid-json",
            "details",
            shape(GraphQlRequest::with_variables(GetInitiativeDetails::build(
                DetailVariables {
                    id: uuid.to_owned(),
                },
            ))),
        ),
        (
            "c038-slug-hit",
            "slug",
            shape(GraphQlRequest::with_variables(
                GetInitiativeBySlugForView::build(SlugVariables {
                    slug_id: "INIT-38".to_owned(),
                }),
            )),
        ),
        (
            "c038-name-case-hit",
            "name",
            shape(GraphQlRequest::with_variables(
                GetInitiativeByNameForView::build(NameVariables {
                    name: "qUaRtErLy nOrTh".to_owned(),
                }),
            )),
        ),
        (
            "c038-url-hit",
            "url-slug",
            shape(GraphQlRequest::with_variables(
                ResolveInitiativeBySlug::build(UrlSlugVariables {
                    slug_id: "000000000038".to_owned(),
                    include_archived: Some(false),
                }),
            )),
        ),
    ];
    for (case, step, (query, variables)) in cases {
        let frozen = frozen(case);
        let source = frozen["graphql"]["groups"][0]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == step)
            .expect("source step");
        assert_eq!(
            compact(&query),
            compact(source["operation"]["document"].as_str().unwrap()),
            "{case}"
        );
        assert_eq!(variables, source["operation"]["variables"], "{case}");
        if case == "c038-uuid-json" {
            assert!(!query.contains("first:"));
            assert!(!query.contains("after:"));
            assert!(!query.contains("pageInfo"));
        }
    }
}

#[test]
fn json_and_pipe_markdown_keep_source_selection_and_sections() {
    let source = frozen("c038-uuid-json");
    let body = &source["graphql"]["groups"][0]["steps"][0]["response"]["data"];
    let response: GetInitiativeDetails =
        parse_response(json!({"data":body}).to_string().as_bytes()).unwrap();
    let detail = response.initiative.unwrap();
    assert_eq!(
        String::from_utf8(render_json(&detail).unwrap()).unwrap(),
        source["expected"]["stdout"]["utf8"].as_str().unwrap()
    );
    let now = "2026-09-29T00:00:00Z".parse().unwrap();
    let pipe = markdown(&detail, now, false);
    assert!(pipe.starts_with("# 🎯 Initiative 38\n\n**Slug:** INI-38\n**URL:**"));
    assert!(pipe.contains("\n**Status:** Active\n**Health:** onTrack\n**Owner:** Alice Display\n**Target Date:** 2999-01-01"));
    assert!(pipe.ends_with("\n\n## Projects (1)\n\n- **Project 38** (Started)"));
    let terminal = markdown(&detail, now, true);
    assert!(!terminal.contains("**Status:**"));
}

#[test]
fn frozen_pipe_documents_preserve_icon_owner_and_project_grouping_bytes() {
    let now = "2026-09-29T00:00:00Z".parse().unwrap();
    for id in [
        "c038-pipe-no-projects",
        "c038-pipe-icon-title",
        "c038-pipe-owner-name-fallback",
        "c038-pipe-known-project-groups",
    ] {
        let source = frozen(id);
        let body = &source["graphql"]["groups"][0]["steps"][0]["response"]["data"];
        let response: GetInitiativeDetails =
            parse_response(json!({ "data": body }).to_string().as_bytes()).unwrap();
        let detail = response.initiative.unwrap();
        let actual = format!("{}\n", markdown(&detail, now, false));
        assert_eq!(
            actual,
            source["expected"]["stdout"]["utf8"].as_str().unwrap(),
            "{id}"
        );
    }
}

#[test]
fn typed_detail_rejects_missing_required_id_and_unknown_enum_is_explicit() {
    let source = frozen("c038-uuid-json");
    let mut body = source["graphql"]["groups"][0]["steps"][0]["response"]["data"].clone();
    body["initiative"].as_object_mut().unwrap().remove("id");
    assert!(
        parse_response::<GetInitiativeDetails>(json!({"data":body}).to_string().as_bytes())
            .is_err()
    );
    let mut body = source["graphql"]["groups"][0]["steps"][0]["response"]["data"].clone();
    body["initiative"]["status"] = json!("paused");
    let decoded: GetInitiativeDetails =
        parse_response(json!({"data":body}).to_string().as_bytes()).unwrap();
    assert!(matches!(decoded.initiative.unwrap().status,
        linear_cli::graphql::operations::initiatives::InitiativeStatus::Unknown(ref v) if v == "paused"));
}

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn empty_reference_is_native_usage_and_help_is_native() {
    let error = crate::parse(&args(&["initiative", "view", ""])).unwrap_err();
    assert!(error.to_string().contains("expected a nonempty value"));
    let error = crate::parse(&args(&["initiative", "view", "--help"])).unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
}
