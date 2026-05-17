//! HTTP reverse proxy implementation.
//!
//! Ported from `internal/proxy/proxy.go`. The handler logic, header
//! filtering and redirect-following semantics match the Go server byte-for-byte
//! at the wire level (apart from minor framing differences imposed by hyper /
//! reqwest such as automatically-managed Transfer-Encoding).

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::TryStreamExt;
use url::Url;

use crate::config::Config;
use crate::homepage::HOMEPAGE;
use crate::matcher;

/// Maximum number of upstream redirects the proxy will follow before giving up.
/// Matches `maxRedirectDepth` in `proxy.go`.
const MAX_REDIRECT_DEPTH: u32 = 5;

/// Default User-Agent applied when the client request has none. Kept verbatim
/// from the Go server so existing upstream allowlists keep working.
const DEFAULT_USER_AGENT: &str = "Hub-Proxy-Go/1.0";

/// Shared state plumbed through the axum router.
pub struct AppState {
    pub cfg: Config,
    pub client: reqwest::Client,
}

/// Build the axum router with handlers wired to the supplied state.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/favicon.ico", get(favicon))
        .route("/healthz", get(healthz))
        .fallback(fallback)
        .with_state(state)
}

// --- Handlers ----------------------------------------------------------------

async fn index(req: Request<Body>) -> Response {
    if let Some(q) = req.uri().query() {
        // Manual parse of `q=...` — keeps the dependency surface small.
        for pair in q.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            if k == "q" && !v.is_empty() {
                let decoded = percent_decode(v);
                return Response::builder()
                    .status(StatusCode::FOUND)
                    .header("Location", format!("/{decoded}"))
                    .body(Body::empty())
                    .expect("redirect response")
                    .into_response();
            }
        }
    }
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=UTF-8")
        .body(Body::from(HOMEPAGE))
        .expect("homepage response")
        .into_response()
}

async fn favicon() -> Response {
    (StatusCode::NOT_FOUND, "404 page not found").into_response()
}

async fn healthz() -> Response {
    (StatusCode::OK, "ok").into_response()
}

async fn fallback(State(state): State<Arc<AppState>>, req: Request<Body>) -> Response {
    handle_proxy(state, req).await
}

// --- Proxy core --------------------------------------------------------------

async fn handle_proxy(state: Arc<AppState>, req: Request<Body>) -> Response {
    let (parts, body) = req.into_parts();

    // Strip the leading `/` from the path; append `?query` if present.
    let mut target = parts.uri.path().trim_start_matches('/').to_string();
    if let Some(q) = parts.uri.query() {
        target.push('?');
        target.push_str(q);
    }
    let mut target = normalize_url(&target);

    if matcher::match_url(&target).is_none() {
        return (StatusCode::FORBIDDEN, "无效的输入 URL").into_response();
    }

    // Automatic /blob/ → /raw/ rewrite for GitHub / HF blob URLs.
    if matcher::is_blob(&target) {
        target = target.replacen("/blob/", "/raw/", 1);
    }

    stream_proxy(state, target, parts.method, body, parts.headers).await
}

/// Drive the upstream request, following our own redirects (up to
/// `MAX_REDIRECT_DEPTH`). Implemented as a loop rather than recursion to avoid
/// boxing the future across recursive async calls.
async fn stream_proxy(
    state: Arc<AppState>,
    target: String,
    method: Method,
    body: Body,
    headers: HeaderMap,
) -> Response {
    let mut target = target;
    let mut method = method;
    // Body is consumed on the first attempt; subsequent redirect hops always
    // use an empty body + GET (matching the Go server's behaviour).
    let mut body_opt: Option<Body> = Some(body);
    let mut headers = headers;
    let mut depth: u32 = 0;

    loop {
        if depth > MAX_REDIRECT_DEPTH {
            return (StatusCode::BAD_GATEWAY, "重定向次数过多").into_response();
        }

        let parsed = match Url::parse(&target) {
            Ok(u) => u,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("无效的 URL: {e}")).into_response();
            }
        };

        let outbound_method = match reqwest::Method::from_bytes(method.as_str().as_bytes()) {
            Ok(m) => m,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("无效的方法: {e}")).into_response();
            }
        };

        let mut builder = state.client.request(outbound_method.clone(), parsed.clone());

        // Forward headers, skipping Host and hop-by-hop.
        let mut have_user_agent = false;
        for (name, value) in headers.iter() {
            if is_host_header(name) || is_hop_by_hop(name) {
                continue;
            }
            if name == axum::http::header::USER_AGENT {
                have_user_agent = !value.is_empty();
            }
            builder = builder.header(name.as_str(), value.as_bytes());
        }
        if !have_user_agent {
            builder = builder.header("User-Agent", DEFAULT_USER_AGENT);
        }

        // Stream the request body upstream. Only the first hop carries the
        // original body; redirect hops use an empty body.
        if let Some(b) = body_opt.take() {
            let data_stream = b
                .into_data_stream()
                .map_err(std::io::Error::other);
            builder = builder.body(reqwest::Body::wrap_stream(data_stream));
        }

        let resp = match builder.send().await {
            Ok(r) => r,
            Err(e) => {
                return (StatusCode::BAD_GATEWAY, format!("上游服务器错误: {e}")).into_response();
            }
        };

        let status = resp.status();
        let resp_headers = resp.headers().clone();

        // Size-limit guard: if upstream advertises a Content-Length larger than
        // our configured cap, redirect the client to the upstream URL instead
        // of streaming the payload.
        if let Some(cl) = resp_headers.get(reqwest::header::CONTENT_LENGTH) {
            if let Ok(s) = cl.to_str() {
                if let Ok(size) = s.parse::<u64>() {
                    if size > state.cfg.size_limit {
                        drop(resp);
                        return Response::builder()
                            .status(StatusCode::FOUND)
                            .header("Location", target.as_str())
                            .body(Body::empty())
                            .expect("size-limit redirect response")
                            .into_response();
                    }
                }
            }
        }

        // 3xx redirect handling. We resolve relative Locations against the
        // current target; proxiable destinations are bounced back through the
        // client (so the next request lands on us again with the new path),
        // non-proxiable destinations are followed server-side after stripping
        // sensitive headers.
        let code = status.as_u16();
        if (300..400).contains(&code) {
            if let Some(loc) = resp_headers.get(reqwest::header::LOCATION) {
                if let Ok(loc_str) = loc.to_str() {
                    let resolved = if loc_str.starts_with("http") {
                        loc_str.to_string()
                    } else {
                        match Url::parse(&target).and_then(|b| b.join(loc_str)) {
                            Ok(u) => u.to_string(),
                            Err(_) => loc_str.to_string(),
                        }
                    };

                    if matcher::match_url(&resolved).is_some() {
                        return Response::builder()
                            .status(status_to_axum(status))
                            .header("Location", format!("/{resolved}"))
                            .body(Body::empty())
                            .expect("client-bounce redirect response")
                            .into_response();
                    }

                    // Follow server-side after stripping sensitive headers.
                    drop(resp);
                    let cleaned = strip_sensitive(&headers);
                    target = resolved;
                    method = Method::GET;
                    headers = cleaned;
                    body_opt = Some(Body::empty());
                    depth += 1;
                    continue;
                }
            }
        }

        return forward_response(status, resp_headers, resp);
    }
}

/// Build the final downstream response from the upstream one. Hop-by-hop
/// headers are dropped; the response body is streamed bytes-as-they-arrive.
fn forward_response(
    status: reqwest::StatusCode,
    headers: reqwest::header::HeaderMap,
    resp: reqwest::Response,
) -> Response {
    let mut builder = Response::builder().status(status_to_axum(status));
    if let Some(target_headers) = builder.headers_mut() {
        for (name, value) in headers.iter() {
            if is_hop_by_hop_str(name.as_str()) {
                continue;
            }
            if let (Ok(hn), Ok(hv)) = (
                HeaderName::from_bytes(name.as_str().as_bytes()),
                HeaderValue::from_bytes(value.as_bytes()),
            ) {
                target_headers.append(hn, hv);
            }
        }
    }

    let stream = resp
        .bytes_stream()
        .map_err(std::io::Error::other);
    let body = Body::from_stream(stream);

    builder.body(body).expect("forward response").into_response()
}

// --- Helpers -----------------------------------------------------------------

/// Mirror of `normalizeURL` in proxy.go. Promotes single-slash schemes to the
/// canonical double-slash form and defaults missing schemes to https.
pub(crate) fn normalize_url(u: &str) -> String {
    if u.starts_with("https:/") && !u.starts_with("https://") {
        return format!("https://{}", &u[7..]);
    }
    if u.starts_with("http:/") && !u.starts_with("http://") {
        return format!("http://{}", &u[6..]);
    }
    if !u.starts_with("http") {
        return format!("https://{}", u);
    }
    u.to_string()
}

fn is_host_header(name: &HeaderName) -> bool {
    name == axum::http::header::HOST
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    is_hop_by_hop_str(name.as_str())
}

fn is_hop_by_hop_str(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "proxy-connection"
            | "keep-alive"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "proxy-authenticate"
            | "proxy-authorization"
    )
}

fn is_sensitive(name: &HeaderName) -> bool {
    matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "set-cookie"
    )
}

fn strip_sensitive(headers: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in headers.iter() {
        if !is_sensitive(name) {
            out.append(name.clone(), value.clone());
        }
    }
    out
}

/// Convert a reqwest status to an axum status. They share the underlying type
/// in compatible versions; going through `u16` insulates us from skew.
fn status_to_axum(s: reqwest::StatusCode) -> StatusCode {
    StatusCode::from_u16(s.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

/// Minimal percent-decoder for the homepage `q=` query string. Replicates Go's
/// behaviour for typical inputs (only %XX escapes; '+' is left as '+').
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = hex_val(bytes[i + 1]);
            let l = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (h, l) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// --- Test-only support -------------------------------------------------------

/// Test-only wrapper that runs the proxy core without going through the
/// matcher's URL allowlist. Real traffic still passes through `handle_proxy`,
/// which enforces the matcher; this hook lets integration tests drive
/// streaming, redirect handling, and size-limit behaviour against arbitrary
/// wiremock upstreams (whose hostnames are not on the GitHub/HF allowlist).
///
/// The function is `pub` rather than `cfg(test)`-gated so that integration
/// tests in `tests/` — which compile as a separate crate — can call it. The
/// matcher gate in `handle_proxy` is the load-bearing security boundary; this
/// helper merely skips that gate, it does not bypass any other check.
#[doc(hidden)]
pub async fn stream_proxy_for_test(
    state: Arc<AppState>,
    target: String,
    method: Method,
    body: Body,
    headers: HeaderMap,
) -> Response {
    stream_proxy(state, target, method, body, headers).await
}
