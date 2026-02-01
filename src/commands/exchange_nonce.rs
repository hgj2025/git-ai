//! Exchange install nonce for credentials (auto-login from web install page)
//!
//! This command is called by the install script to exchange a nonce for
//! OAuth credentials. It reads INSTALL_NONCE and API_BASE from environment
//! variables and stores credentials in ~/.git-ai/internal/credentials.

use crate::auth::client::OAuthClient;
use crate::auth::CredentialStore;

/// Handle the exchange-nonce command (internal - called by install scripts)
pub fn handle_exchange_nonce(_args: &[String]) {
    // Read from environment variables (injected by install script)
    let nonce = std::env::var("INSTALL_NONCE").ok().filter(|s| !s.is_empty());
    let api_base = std::env::var("API_BASE").ok().filter(|s| !s.is_empty());
    let install_page_url = std::env::var("INSTALL_PAGE_URL").ok().filter(|s| !s.is_empty());

    // If no nonce provided, silently exit (not an error - just means no auto-login)
    let Some(nonce) = nonce else {
        return;
    };

    let Some(api_base) = api_base else {
        eprintln!("\x1b[33mWarning: INSTALL_NONCE set but API_BASE missing\x1b[0m");
        return;
    };

    // Perform the exchange
    if let Err(e) = exchange_nonce(&nonce, &api_base, install_page_url.as_deref()) {
        eprintln!("{}", e);
        // Don't exit with error - install should continue even if login fails
    }
}

fn exchange_nonce(nonce: &str, api_base: &str, install_page_url: Option<&str>) -> Result<(), String> {
    eprintln!("Exchanging install nonce for credentials...");

    // Create OAuth client with custom base URL
    let client = OAuthClient::with_base_url(api_base)
        .map_err(|e| format_error(&e, install_page_url))?;

    // Exchange the nonce for credentials
    let credentials = client
        .exchange_install_nonce(nonce)
        .map_err(|e| format_error(&e, install_page_url))?;

    // Store credentials
    let store = CredentialStore::new();
    store
        .store(&credentials)
        .map_err(|e| format!("\x1b[33mWarning: Failed to store credentials: {}\x1b[0m", e))?;

    eprintln!("\x1b[32mSuccessfully logged in\x1b[0m");
    Ok(())
}

fn format_error(_detail: &str, install_page_url: Option<&str>) -> String {
    if let Some(url) = install_page_url {
        format!(
            "\x1b[33mAutomatic login expired. Visit the link below to get a fresh install command:\n  {}\x1b[0m",
            url
        )
    } else {
        "\x1b[33mAutomatic login expired. Visit your organization's install page for a fresh install command.\x1b[0m".to_string()
    }
}
