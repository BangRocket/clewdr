use clewdr::config::{CodexAuth, CodexAuthStatus};
use clewdr::services::codex_auth_actor::CodexAuthActorHandle;

fn dummy_auth(id: &str) -> CodexAuth {
    CodexAuth {
        id: id.into(),
        label: None,
        id_token: "x.y.z".into(),
        access_token: "at".into(),
        refresh_token: "rt".into(),
        access_expires_at: i64::MAX,
        account_id: "acct".into(),
        plan: None,
        status: CodexAuthStatus::Valid,
        last_used_at: None,
    }
}

#[tokio::test]
async fn dispatch_returns_valid_credential() {
    let handle = CodexAuthActorHandle::start_with(vec![dummy_auth("a"), dummy_auth("b")])
        .await
        .expect("start");
    let auth = handle.request().await.expect("dispatch");
    assert!(auth.id == "a" || auth.id == "b");
}

#[tokio::test]
async fn dispatch_skips_banned() {
    let mut a = dummy_auth("a");
    a.status = CodexAuthStatus::Banned;
    let b = dummy_auth("b");
    let handle = CodexAuthActorHandle::start_with(vec![a, b]).await.unwrap();
    let auth = handle.request().await.expect("dispatch");
    assert_eq!(auth.id, "b");
}

#[tokio::test]
async fn dispatch_fails_when_pool_empty() {
    let handle = CodexAuthActorHandle::start_with(vec![]).await.unwrap();
    assert!(handle.request().await.is_err());
}

#[tokio::test]
async fn submit_duplicate_id_is_rejected() {
    let handle = CodexAuthActorHandle::start_with(vec![dummy_auth("a")])
        .await
        .unwrap();
    assert!(handle.submit(dummy_auth("a")).await.is_err());
    assert!(handle.submit(dummy_auth("b")).await.is_ok());
}

#[tokio::test]
async fn delete_removes_from_pool() {
    let handle = CodexAuthActorHandle::start_with(vec![dummy_auth("a"), dummy_auth("b")])
        .await
        .unwrap();
    handle.delete("a".into()).await.expect("delete");
    let list = handle.list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "b");
}
