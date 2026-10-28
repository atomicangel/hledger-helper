use axum::{response::Json, extract::State};
use std::sync::Arc;

use crate::config::{Config, Category};

/// Get available budget categories from hledger accounts
pub async fn get_budget_categories(
    State(config): State<Arc<Config>>,
) -> Json<Vec<Category>> {
    let categories = config.get_categories();
    Json(categories)
}


