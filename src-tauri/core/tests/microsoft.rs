//! Цепочка входа Microsoft с локальным сервером вместо Microsoft, Xbox и Mojang.

mod common;

use common::{bind_port, Routes, TestServer};
use fenix_core::errors::LauncherError;
use fenix_core::microsoft::{
    poll_device_login, refresh_microsoft_tokens, request_device_code, sign_in_to_minecraft,
    DeviceLogin, MsEndpoints,
};
use serde_json::json;
use std::time::Duration;

const CLIENT_ID: &str = "test-client";
const DEAD_URL: &str = "http://127.0.0.1:1";

fn route(routes: &mut Routes, path: &str, status: u16, body: serde_json::Value) {
    routes.insert(path.to_string(), (status, body.to_string().into_bytes()));
}

fn working_routes() -> Routes {
    let mut routes = Routes::new();
    route(
        &mut routes,
        "/ms/devicecode",
        200,
        json!({
            "device_code": "device",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://microsoft.com/link",
            "interval": 0,
            "expires_in": 60
        }),
    );
    route(
        &mut routes,
        "/ms/token",
        200,
        json!({
            "access_token": "ms-access",
            "refresh_token": "ms-refresh-rotated"
        }),
    );
    route(
        &mut routes,
        "/xbl",
        200,
        json!({
            "Token": "xbl-token",
            "DisplayClaims": { "xui": [{ "uhs": "hash" }] }
        }),
    );
    route(
        &mut routes,
        "/xsts",
        200,
        json!({
            "Token": "xsts-token",
            "DisplayClaims": { "xui": [{ "uhs": "hash", "xid": "2535" }] }
        }),
    );
    route(
        &mut routes,
        "/mc/authentication/login_with_xbox",
        200,
        json!({
            "access_token": "mc-token",
            "expires_in": 86400
        }),
    );
    route(
        &mut routes,
        "/mc/minecraft/profile",
        200,
        json!({
            "id": "0123456789abcdef0123456789abcdef",
            "name": "RealPlayer"
        }),
    );
    routes
}

fn endpoints(base: &str) -> MsEndpoints {
    MsEndpoints {
        authority: format!("{base}/ms"),
        xbox_user: format!("{base}/xbl"),
        xsts: format!("{base}/xsts"),
        minecraft: format!("{base}/mc"),
    }
}

async fn start(routes: Routes) -> (TestServer, MsEndpoints) {
    let port = bind_port().await;
    let ends = endpoints(&port.base);
    (port.serve(routes), ends)
}

fn device() -> DeviceLogin {
    DeviceLogin {
        device_code: "device".to_string(),
        user_code: "ABCD-EFGH".to_string(),
        verification_uri: "https://microsoft.com/link".to_string(),
        interval: Duration::from_millis(1),
        expires_in: Duration::from_secs(5),
    }
}

#[tokio::test]
async fn full_login_returns_the_real_profile() {
    let (server, ends) = start(working_routes()).await;
    let client = reqwest::Client::new();

    let login = request_device_code(&client, &ends, CLIENT_ID)
        .await
        .unwrap();
    assert_eq!(login.user_code, "ABCD-EFGH");

    let tokens = poll_device_login(&client, &ends, CLIENT_ID, &login, || true)
        .await
        .unwrap()
        .unwrap();
    let account = sign_in_to_minecraft(&client, &ends, &tokens.access_token)
        .await
        .unwrap();

    assert_eq!(account.username, "RealPlayer");
    assert_eq!(account.uuid, "0123456789abcdef0123456789abcdef");
    assert_eq!(account.access_token, "mc-token");
    assert_eq!(account.xuid, "2535");
    server.stop().await;
}

#[tokio::test]
async fn account_without_the_game_is_reported() {
    let mut routes = working_routes();
    route(
        &mut routes,
        "/mc/minecraft/profile",
        404,
        json!({ "error": "NOT_FOUND" }),
    );
    let (server, ends) = start(routes).await;

    let result = sign_in_to_minecraft(&reqwest::Client::new(), &ends, "t").await;

    assert_eq!(result.err(), Some(LauncherError::MicrosoftNoGame));
    server.stop().await;
}

#[tokio::test]
async fn unapproved_application_is_explained() {
    let mut routes = working_routes();
    route(
        &mut routes,
        "/mc/authentication/login_with_xbox",
        403,
        json!({}),
    );
    let (server, ends) = start(routes).await;

    let result = sign_in_to_minecraft(&reqwest::Client::new(), &ends, "t").await;

    match result {
        Err(LauncherError::Microsoft(reason)) => assert!(reason.contains("одобрено"), "{reason}"),
        _ => panic!("expected an explanation"),
    }
    server.stop().await;
}

#[tokio::test]
async fn account_without_xbox_gets_a_readable_error() {
    let mut routes = working_routes();
    route(
        &mut routes,
        "/xsts",
        401,
        json!({ "XErr": 2148916233u64, "Message": "" }),
    );
    let (server, ends) = start(routes).await;

    let result = sign_in_to_minecraft(&reqwest::Client::new(), &ends, "t").await;

    match result {
        Err(LauncherError::Microsoft(reason)) => assert!(reason.contains("Xbox"), "{reason}"),
        _ => panic!("expected an Xbox error"),
    }
    server.stop().await;
}

#[tokio::test]
async fn refresh_rotates_tokens_and_detects_an_expired_session() {
    let (server, ends) = start(working_routes()).await;
    let tokens = refresh_microsoft_tokens(&reqwest::Client::new(), &ends, CLIENT_ID, "old")
        .await
        .unwrap();
    assert_eq!(tokens.refresh_token, "ms-refresh-rotated");
    server.stop().await;

    let mut routes = Routes::new();
    route(
        &mut routes,
        "/ms/token",
        400,
        json!({ "error": "invalid_grant" }),
    );
    let (server, ends) = start(routes).await;
    let result = refresh_microsoft_tokens(&reqwest::Client::new(), &ends, CLIENT_ID, "old").await;
    assert_eq!(result.err(), Some(LauncherError::MicrosoftSessionExpired));
    server.stop().await;
}

#[tokio::test]
async fn missing_connection_is_not_reported_as_a_lost_session() {
    let ends = endpoints(DEAD_URL);
    let client = reqwest::Client::new();

    let refresh = refresh_microsoft_tokens(&client, &ends, CLIENT_ID, "old").await;
    let sign_in = sign_in_to_minecraft(&client, &ends, "t").await;

    assert!(matches!(
        refresh.err(),
        Some(LauncherError::NeedsInternet { .. })
    ));
    assert!(matches!(
        sign_in.err(),
        Some(LauncherError::NeedsInternet { .. })
    ));
}

#[tokio::test]
async fn cancelled_login_stops_waiting() {
    let mut routes = Routes::new();
    route(
        &mut routes,
        "/ms/token",
        400,
        json!({ "error": "authorization_pending" }),
    );
    let (server, ends) = start(routes).await;

    let result = poll_device_login(&reqwest::Client::new(), &ends, CLIENT_ID, &device(), || {
        false
    })
    .await
    .unwrap();

    assert!(result.is_none());
    server.stop().await;
}

#[tokio::test]
async fn expired_code_ends_the_login() {
    let mut routes = Routes::new();
    route(
        &mut routes,
        "/ms/token",
        400,
        json!({ "error": "authorization_pending" }),
    );
    let (server, ends) = start(routes).await;
    let short = DeviceLogin {
        expires_in: Duration::from_millis(50),
        ..device()
    };

    let result =
        poll_device_login(&reqwest::Client::new(), &ends, CLIENT_ID, &short, || true).await;

    assert!(matches!(result, Err(LauncherError::Microsoft(_))));
    server.stop().await;
}
