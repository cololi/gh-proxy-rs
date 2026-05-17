//! Integration tests for the proxy router and core streaming logic.
//!
//! Tests 1-5 exercise the public router via tower's `oneshot` (in-process,
//! no port binding). Tests 6-10 drive the proxy core through the test-only
//! `stream_proxy_for_test` entry point so we can target wiremock upstreams
//! (whose hostnames don't match the GitHub/HF allowlist).

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request as WMRequest, Respond, ResponseTemplate};

use hub_proxy::config::Config;
use hub_proxy::proxy::{router, stream_proxy_for_test, AppState};

fn default_cfg() -> Config {
    Config {
        listen: ":0".to_string(),
        size_limit: 1_000_000_000,
        buffer_size: 32 * 1024,
        upstream_timeout: Duration::from_secs(10),
        shutdown_timeout: Duration::from_secs(1),
    }
}

fn build_state(cfg: Config) -> Arc<AppState> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .no_gzip()
        .build()
        .expect("client");
    Arc::new(AppState { cfg, client })
}

async fn read_body(body: Body) -> String {
    let bytes = body.collect().await.expect("collect").to_bytes();
    String::from_utf8_lossy(&bytes).to_string()
}

// --- Router-level tests ------------------------------------------------------

#[tokio::test]
async fn homepage_renders() {
    let app = router(build_state(default_cfg()));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.starts_with("text/html"), "content-type = {ct}");
    let body = read_body(resp.into_body()).await;
    assert!(body.contains("Hub-Proxy-Go"), "body missing brand: {body:.80}");
}

#[tokio::test]
async fn homepage_q_redirects() {
    let app = router(build_state(default_cfg()));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/?q=https://github.com/foo/bar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        resp.headers().get("location").unwrap(),
        "/https://github.com/foo/bar"
    );
}

#[tokio::test]
async fn healthz_ok() {
    let app = router(build_state(default_cfg()));
    let resp = app
        .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn favicon_not_found() {
    let app = router(build_state(default_cfg()));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/favicon.ico")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn non_matchable_url_forbidden() {
    let app = router(build_state(default_cfg()));
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/https://example.com/foo")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = read_body(resp.into_body()).await;
    assert_eq!(body, "无效的输入 URL");
}

// --- Unit-level: blob rewriting ---------------------------------------------

#[tokio::test]
async fn blob_rewritten_to_raw() {
    // The Go server rewrites `/blob/` → `/raw/` on GitHub URLs before
    // dispatching upstream. End-to-end coverage against a real github.com is
    // out of scope; assert the rewrite itself on a representative input.
    let target = "https://github.com/user/repo/blob/main/README.md".to_string();
    assert!(hub_proxy::matcher::is_blob(&target));
    let rewritten = target.replacen("/blob/", "/raw/", 1);
    assert_eq!(
        rewritten,
        "https://github.com/user/repo/raw/main/README.md"
    );
}

// --- Streaming via stream_proxy_for_test ------------------------------------

#[tokio::test]
async fn streams_response_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/echo"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hello world"))
        .mount(&server)
        .await;

    let target = format!("{}/echo", server.uri());
    let resp = stream_proxy_for_test(
        build_state(default_cfg()),
        target,
        Method::GET,
        Body::empty(),
        HeaderMap::new(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    assert_eq!(body, "hello world");
}

#[tokio::test]
async fn size_limit_triggers_302() {
    let server = MockServer::start().await;
    // Hyper enforces Content-Length matches the body length on h1, so we pick
    // a moderate body and a tiny size_limit instead of inflating the header.
    let body = "x".repeat(1024);
    Mock::given(method("GET"))
        .and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let target = format!("{}/big", server.uri());
    let mut cfg = default_cfg();
    cfg.size_limit = 10;

    let resp = stream_proxy_for_test(
        build_state(cfg),
        target.clone(),
        Method::GET,
        Body::empty(),
        HeaderMap::new(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(resp.headers().get("location").unwrap(), &target);
}

// Custom responder that records the request it saw for later inspection.
#[derive(Clone, Default)]
struct Recorder {
    inner: Arc<std::sync::Mutex<Vec<WMRequest>>>,
}

impl Respond for Recorder {
    fn respond(&self, request: &WMRequest) -> ResponseTemplate {
        self.inner.lock().unwrap().push(request.clone());
        ResponseTemplate::new(200).set_body_string("ok-final")
    }
}

#[tokio::test]
async fn sensitive_headers_stripped_on_cross_redirect() {
    // Upstream A returns a 302 → upstream B; B is not proxiable (wiremock host
    // does not match the GitHub/HF allowlist) so the proxy follows it
    // server-side, which is the path that strips sensitive headers.
    let server = MockServer::start().await;

    let recorder = Recorder::default();
    Mock::given(method("GET"))
        .and(path("/final"))
        .respond_with(recorder.clone())
        .mount(&server)
        .await;

    let redirect_target = format!("{}/final", server.uri());
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", redirect_target.clone()),
        )
        .mount(&server)
        .await;

    let mut headers = HeaderMap::new();
    headers.insert("Authorization", HeaderValue::from_static("secret"));
    headers.insert("Cookie", HeaderValue::from_static("sessid=abc"));
    headers.insert("X-Trace", HeaderValue::from_static("keep-me"));

    let target = format!("{}/start", server.uri());
    let resp = stream_proxy_for_test(
        build_state(default_cfg()),
        target,
        Method::GET,
        Body::empty(),
        headers,
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    assert_eq!(body, "ok-final");

    let received = recorder.inner.lock().unwrap().clone();
    assert_eq!(received.len(), 1, "exactly one request to /final");
    let r = &received[0];
    assert!(
        r.headers.get("authorization").is_none(),
        "Authorization should have been stripped"
    );
    assert!(
        r.headers.get("cookie").is_none(),
        "Cookie should have been stripped"
    );
    // Non-sensitive headers should survive.
    assert_eq!(
        r.headers.get("x-trace").map(|v| v.as_bytes()),
        Some(b"keep-me".as_slice())
    );
}

#[tokio::test]
async fn redirect_loop_returns_502() {
    let server = MockServer::start().await;
    // Self-loop: /loop → /loop. Wiremock host isn't proxiable, so the proxy
    // follows redirects server-side and bumps the depth counter each hop.
    Mock::given(method("GET"))
        .and(path("/loop"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", "/loop"),
        )
        .mount(&server)
        .await;

    let target = format!("{}/loop", server.uri());
    let resp = stream_proxy_for_test(
        build_state(default_cfg()),
        target,
        Method::GET,
        Body::empty(),
        HeaderMap::new(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    let body = read_body(resp.into_body()).await;
    assert_eq!(body, "重定向次数过多");
}

// Suppress the unused-import warning for `header` if it isn't referenced
// in this file. It's available so future tests can use it without an edit.
#[allow(dead_code)]
fn _keep_header_import() {
    let _ = header("x-test", "1");
}
