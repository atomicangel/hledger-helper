use std::sync::Arc;
use tracing_subscriber::EnvFilter;

mod config;
mod hledger;
mod file_handler;
mod static_handler;
mod api;

use config::Config;
use static_handler::create_router as create_app_router;

/// Parse config path from command-line arguments
/// - If -c or --config is passed, use that path
/// - Otherwise, look for config.yaml in the same directory as the binary
fn parse_config_path(args: &[String]) -> String {
    for i in 0..args.len() {
        if args[i] == "-c" || args[i] == "--config" {
            if i + 1 < args.len() {
                return args[i + 1].clone();
            }
        }
    }
    
    // Default to config.yaml in the current working directory
    std::env::current_dir()
        .map(|dir| dir.join("config.yaml").to_string_lossy().to_string())
        .expect("Failed to get current directory")
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let config_path = parse_config_path(&args);

    // Load configuration. A load failure is fatal: print the readable
    // error (not a panic dump) and exit, so the operator knows what to
    // fix -- e.g. a missing transfer_journal definition.
    let config = match Config::load(&config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };


    // The transfer journal is a structural file: create the defined path
    // (with parent directories) if it does not exist yet. A missing key
    // already failed load above; this only covers a defined-but-missing
    // file, and a creation failure is fatal.
    if let Err(e) = config.ensure_transfer_journal() {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    let server_config = config.server.clone();
    
    // Create the router with static file serving
    let app = create_app_router(Arc::new(config));

    // Start the server
    let addr = format!("{}:{}", server_config.bind_address, server_config.port);
    tracing::info!("Starting server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");
    
    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
