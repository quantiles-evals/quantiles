use std::net::SocketAddr;
use std::path::Path;

use anyhow::{Context, Result};

use qt::db;
use qt::server::{ServerConfig, run_server};

pub async fn serve(addr: &str) -> Result<()> {
    let addr = addr
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid server address `{addr}`"))?;
    let db_path = db::workspace_db_path(Path::new("."));

    println!("Serving Quantiles HTTP API on http://{addr}");
    run_server(ServerConfig { addr, db_path }).await
}
