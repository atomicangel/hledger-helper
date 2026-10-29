#[test]
fn test_config_loads() {
    let config = hledger_helper::config::Config::load("config.example.yaml")
        .expect("Failed to load config");
    
    assert_eq!(config.nas_path, "./demo_journals");
    assert_eq!(config.current_year, 2026);
}

#[test]
fn test_top_journal_configured() {
    let config = hledger_helper::config::Config::load("config.example.yaml")
        .expect("Failed to load config");
    
    assert_eq!(config.top_journal, "current.journal");
}

#[test]
fn test_server_configured() {
    let config = hledger_helper::config::Config::load("config.example.yaml")
        .expect("Failed to load config");
    
    assert_eq!(config.server.bind_address, "0.0.0.0");
    assert_eq!(config.server.port, 8080);
}
