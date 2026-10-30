use axum::{
    Router,
    extract::{ConnectInfo, Path, Request},
    middleware::Next,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use axum::http::{HeaderValue, StatusCode, header};
use rust_embed::Embed;
use std::sync::Arc;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::api;

#[derive(Embed)]
#[folder = "src/static/"]
struct StaticAssets;

pub fn create_router(config: Arc<Config>) -> Router {
    Router::new()
        .route("/", get(root_handler))
        .route("/manifest.json", get(manifest_handler))
        .route("/sw.js", get(service_worker_handler))
        .route("/config.json", get(config_handler))
        .route("/api/categories", get(api::get_budget_categories))
        .route("/api/expenses", get(api::get_expenses))
        .route("/api/expenses-totals", get(api::get_expense_totals))
        .route("/api/safe-to-spend", get(api::get_safe_to_spend))
        .route("/api/transactions", post(api::create_transaction))
        .route("/api/transfers", post(api::create_transfer))
        .route("/*filename", get(static_file_handler))
        .layer(axum::middleware::from_fn(security_middleware))
        .with_state(config)
}

async fn root_handler() -> Html<String> {
    let asset = StaticAssets::get("index.html").unwrap();
    Html(String::from_utf8_lossy(asset.data.as_ref()).to_string())
}

async fn manifest_handler() -> Html<String> {
    let asset = StaticAssets::get("manifest.json").unwrap();
    Html(String::from_utf8_lossy(asset.data.as_ref()).to_string())
}

async fn service_worker_handler() -> Html<String> {
    let asset = StaticAssets::get("sw.js").unwrap();
    Html(String::from_utf8_lossy(asset.data.as_ref()).to_string())
}

async fn config_handler(axum::extract::State(config): axum::extract::State<Arc<Config>>) -> String {
    // Only what the frontend actually reads (see app.js loadConfig):
    // location prefixes for dropdown grouping, the safe-to-spend
    // account, and the transfer journal for covers. Everything else
    // the old response carried (nas_path, server bind/port,
    // payment-source journal filenames, top_journal, current_year)
    // is server internals and must not leak to clients.
    serde_json::json!({
        "budget_locations": config.budget_locations,
        "liability_locations": config.liability_locations,
        "expense_locations": config.expense_locations,
        "credit_locations": config.credit_locations,
        "safe_to_spend_account": config.safe_to_spend_account,
        "transfer_journal": config.transfer_journal
    }).to_string()
}

async fn static_file_handler(Path(filename): Path<String>) -> Result<Response, StatusCode> {
    // Try to get the file from embedded assets
    let asset = StaticAssets::get(&filename)
        .ok_or_else(|| StatusCode::NOT_FOUND)?;
    
    let content_type = infer_content_type(&filename);
    let body = asset.data.to_vec();
    
    let response = Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(body))
        .unwrap();
    
    Ok(response)
}

fn infer_content_type(filename: &str) -> &'static str {
    match filename.split('/').last().unwrap_or(filename).split('.').last() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "text/plain; charset=utf-8",
    }
}

/// Per-IP rate-limit window: 300 requests per 60 seconds. Generous for
/// a single-user app (a page load is ~10 requests); it exists to stop
/// floods, since every read spawns an hledger process.
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const RATE_LIMIT_MAX: u32 = 300;

/// Fixed-window counters per client IP. Only entries inside the window
/// are kept (pruned on every request), so the table stays tiny.
static RATE_LIMIT_TABLE: OnceLock<Mutex<HashMap<IpAddr, (Instant, u32)>>> =
    OnceLock::new();

fn rate_limit_table() -> &'static Mutex<HashMap<IpAddr, (Instant, u32)>> {
    RATE_LIMIT_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Security middleware: per-IP rate limiting plus response headers on
/// every response (including 429s and static assets).
async fn security_middleware(req: Request, next: Next) -> Response {
    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    let now = Instant::now();
    let limited = {
        let mut table = rate_limit_table().lock().unwrap_or_else(|e| e.into_inner());
        table.retain(|_, (t, _)| now.duration_since(*t) < RATE_LIMIT_WINDOW);
        let entry = table.entry(ip).or_insert((now, 0));
        entry.1 += 1;
        entry.1 > RATE_LIMIT_MAX
    };
    let mut res = if limited {
        let mut r = (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
        r.headers_mut()
            .insert("retry-after", HeaderValue::from_static("60"));
        r
    } else {
        next.run(req).await
    };
    let headers = res.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; base-uri 'self'; frame-ancestors 'none'",
        ),
    );
    res
}
