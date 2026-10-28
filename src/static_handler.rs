use axum::{Router, response::{Html, Response}, routing::{get, post}, extract::Path};
use axum::http::{StatusCode, header};
use rust_embed::Embed;
use std::sync::Arc;

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
        .route("/api/accounts", get(api::get_accounts))
        .route("/api/payees", get(api::get_payees))
        .route("/api/payment-sources", get(api::get_payment_sources))
        .route("/api/categories", get(api::get_budget_categories))
        .route("/api/expenses", get(api::get_expenses))
        .route("/api/transfer-accounts", get(api::get_transfer_accounts))
        .route("/api/expenses-totals", get(api::get_expense_totals))
        .route("/api/safe-to-spend", get(api::get_safe_to_spend))
        .route("/api/transactions", post(api::create_transaction))
        .route("/api/transfers", post(api::create_transfer))
        .route("/*filename", get(static_file_handler))
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
    serde_json::json!({
        "nas_path": config.nas_path,
        "top_journal": config.top_journal,
        "current_year": config.current_year,
        "budget_locations": config.budget_locations,
        "liability_locations": config.liability_locations,
        "credit_locations": config.credit_locations,
        "safe_to_spend_account": config.safe_to_spend_account,
        "transfer_journal": config.transfer_journal,
        "payment_sources": config.payment_sources,
        "server": {
            "bind_address": config.server.bind_address,
            "port": config.server.port
        }
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
