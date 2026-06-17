use std::path::Path;

use anyhow::Result;

use qt::db;

pub async fn init() -> Result<()> {
    let db_path = db::init_workspace(Path::new(".")).await?;

    println!("Initialized Quantiles workspace at {}", db_path.display());
    Ok(())
}
