use axum::{Json, extract::State};
use std::sync::Arc;
use crate::config::Config;

pub async fn get_expenses(State(config): State<Arc<Config>>) -> Json<Vec<crate::config::Category>> {
    Json(config.get_expenses_from_hledger())
}
