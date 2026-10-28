use axum::{response::Json, extract::State};
use std::sync::Arc;

use crate::config::Config;

/// Payment source for UI buttons
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PaymentSourceResponse {
    pub id: String,
    pub label: String,
    pub journal: String,
}

/// Get available payment sources for UI buttons
pub async fn get_payment_sources(
    State(config): State<Arc<Config>>,
) -> Json<Vec<PaymentSourceResponse>> {
    let sources: Vec<PaymentSourceResponse> = config
        .payment_sources
        .iter()
        .map(|(id, source)| PaymentSourceResponse {
            id: id.clone(),
            label: source.label.clone(),
            journal: source.journal.clone(),
        })
        .collect();

    Json(sources)
}
