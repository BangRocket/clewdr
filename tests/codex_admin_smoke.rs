use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use clewdr::api::codex::{
    AddCodexAuthBody, api_codex_add, api_codex_delete, api_codex_list,
};
use clewdr::services::codex_auth_actor::CodexAuthActorHandle;

const AUTH_JSON: &str = r#"{
  "tokens": {
    "id_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhY2N0X2FiYyIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhYmMtMTIzIiwiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIn19.sig",
    "access_token": "at",
    "refresh_token": "rt-unique-x9"
  }
}"#;

#[tokio::test]
async fn admin_add_then_list_then_delete() {
    let handle = CodexAuthActorHandle::start_with(vec![]).await.unwrap();

    let (status, body) = api_codex_add(
        State(handle.clone()),
        axum::Json(AddCodexAuthBody {
            auth_json: AUTH_JSON.to_string(),
            label: Some("personal".into()),
        }),
    )
    .await
    .expect("add");
    assert_eq!(status, StatusCode::CREATED);
    let id = body.0.id.clone();
    assert!(!id.is_empty());

    let list = api_codex_list(State(handle.clone())).await.expect("list");
    assert_eq!(list.0.len(), 1);
    assert_eq!(list.0[0].label.as_deref(), Some("personal"));

    let status = api_codex_delete(State(handle.clone()), Path(id))
        .await
        .expect("delete");
    assert_eq!(status, StatusCode::NO_CONTENT);

    let list = api_codex_list(State(handle)).await.expect("list");
    assert_eq!(list.0.len(), 0);
}

#[tokio::test]
async fn add_with_malformed_auth_json_is_400() {
    let handle = CodexAuthActorHandle::start_with(vec![]).await.unwrap();

    let result = api_codex_add(
        State(handle),
        axum::Json(AddCodexAuthBody {
            auth_json: "not valid json".into(),
            label: None,
        }),
    )
    .await;

    let err = match result {
        Ok(_) => panic!("expected malformed auth.json to fail"),
        Err(e) => e,
    };
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_unknown_id_is_404() {
    let handle = CodexAuthActorHandle::start_with(vec![]).await.unwrap();

    let result = api_codex_delete(State(handle), Path("does-not-exist".into())).await;

    let err = match result {
        Ok(_) => panic!("expected delete of unknown id to fail"),
        Err(e) => e,
    };
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
