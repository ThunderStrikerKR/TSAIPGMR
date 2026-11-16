mod auth;
mod store;
mod tools;

use std::{env, error::Error, net::SocketAddr, path::PathBuf};

use axum::{Router, middleware};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let bind: SocketAddr = env::var("MCP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8787".into())
        .parse()?;
    if !bind.ip().is_loopback() {
        return Err(
            "MCP_BIND must use a loopback address. Terminate TLS at the test proxy.".into(),
        );
    }

    let project = env::var("MCP_PROJECT").unwrap_or_else(|_| "synthetic-init".into());
    if !tools::valid_id(&project) {
        return Err("MCP_PROJECT must be a 1–64 character identifier".into());
    }
    let auth = auth::Auth::from_env(project)?;
    let path =
        PathBuf::from(env::var_os("MCP_DB_PATH").unwrap_or_else(|| "data/results.sqlite3".into()));
    let store = store::Store::open(path)?;

    let mut hosts = vec![
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
        "::1".to_owned(),
    ];
    if let Ok(host) = env::var("MCP_ALLOWED_HOST") {
        hosts.push(host);
    }
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(16 * 1024)
        .with_allowed_hosts(hosts)
        .enforce_origin_validation();
    let service: StreamableHttpService<_, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(tools::SyntheticTools::new(store.clone())),
        Default::default(),
        config,
    );
    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(auth, auth::authorize));

    let listener = TcpListener::bind(bind).await?;
    println!("{}", listener.local_addr()?);
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
