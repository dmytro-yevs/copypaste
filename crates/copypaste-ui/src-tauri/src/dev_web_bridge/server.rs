use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Method as HttpMethod, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::cors::{AllowOrigin, CorsLayer};
use uuid::Uuid;

use crate::backend::daemon::DaemonBackend;
use crate::source_app_icon::SourceAppIconCache;

use super::contract::*;
use super::dispatch::call;

async fn health(
    State(state): State<BridgeState>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, Json<Failure>)> {
    if !is_authorized(&headers, &state) {
        return Err(unauthorized());
    }
    Ok(StatusCode::NO_CONTENT)
}

fn cors(origin: HeaderValue) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::exact(origin))
        .allow_methods([HttpMethod::GET, HttpMethod::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

pub async fn run_from_env() -> Result<(), Box<dyn std::error::Error>> {
    let origin = env::var(ENV_ORIGIN).unwrap_or_else(|_| DEFAULT_VITE_ORIGIN.into());
    let origin_header = HeaderValue::from_str(&origin)?;
    let env_file = env::var(ENV_FILE)?;
    let bearer = Uuid::new_v4().simple().to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address: SocketAddr = listener.local_addr()?;
    let url = format!("http://{address}");

    write_vite_env(&env_file, &url, &bearer)?;
    println!("CopyPaste browser bridge: {url}");

    let state = BridgeState {
        backend: DaemonBackend::new(),
        icons: Arc::new(SourceAppIconCache::default()),
        origin: origin_header.clone(),
        bearer,
    };
    let app = Router::new()
        .route("/v1/call", post(call))
        .route("/health", get(health))
        .with_state(state)
        .layer(cors(origin_header));
    axum::serve(listener, app).await?;
    Ok(())
}

fn write_vite_env(path: &str, url: &str, token: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut options = OpenOptions::new();
    options.write(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    writeln!(file, "VITE_COPYPASTE_WEB_BRIDGE_URL={url}")?;
    writeln!(file, "VITE_COPYPASTE_WEB_BRIDGE_TOKEN={token}")?;
    file.sync_all()
}
