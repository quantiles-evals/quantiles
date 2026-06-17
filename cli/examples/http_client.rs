use std::net::SocketAddr;

use anyhow::{Result, bail};
use qt::client::QuantilesClient;
use qt::db;
use qt::server::{ServerConfig, run_server};

#[tokio::main]
async fn main() -> Result<()> {
    let root = std::env::temp_dir().join("quantiles-http-client-example");
    let db_path = db::init_workspace(&root).await?;
    let addr = "127.0.0.1:8765".parse::<SocketAddr>()?;

    println!("starting server on {addr:?}");
    tokio::spawn(async move {
        run_server(ServerConfig { addr, db_path })
            .await
            .expect("server failed");
    });

    let base_url = format!("http://{addr}");
    let client = QuantilesClient::new(base_url);
    wait_for_server(&client).await?;
    println!("server started");

    let run_id = client
        .create_run("http-example", Some("{\"dataset\":\"smoke\"}"))
        .await?;
    println!("created run (run_id={run_id})");
    let first = client
        .run_step(run_id, "call-model", "input-hash-1", || {
            Ok("model output".to_owned())
        })
        .await?;
    println!("ran first step (response='{first}')");
    let second = client
        .run_step(run_id, "call-model", "input-hash-1", || {
            Ok("this should not be used".to_owned())
        })
        .await?;
    client.set_run_output(run_id, &second).await?;
    client.complete_run(run_id).await?;

    println!("ran second step (response='{second}')");
    println!("eval done!");

    Ok(())
}

async fn wait_for_server(client: &QuantilesClient) -> Result<()> {
    for _ in 0..50 {
        if client.health().await.is_ok() {
            return Ok(());
        }

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    bail!("server did not become ready");
}
