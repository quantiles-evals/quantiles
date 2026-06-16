use anyhow::Result;
use comfy_table::{Cell, ContentArrangement, Table, presets::NOTHING};

use qt::{
    db,
    time::{format_duration, format_utc},
};

#[derive(serde::Serialize)]
struct RunJSON {
    id: i64,
    eval: String,
    status: String,
    samples: i64,
    created: String,
    duration_sec: Option<String>,
}

#[derive(serde::Serialize)]
struct ListRunsJSON {
    runs: Vec<RunJSON>,
}

pub async fn list(json: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = db::resolve_workspace_root(&cwd, true).await?;
    let db = db::open_workspace(&root).await?;
    let runs = db::list_runs(&db).await?;

    if runs.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&ListRunsJSON { runs: vec![] })?
            );
        } else {
            println!("No eval runs found.");
        }
        return Ok(());
    }

    if json {
        let runs_json: Vec<RunJSON> = runs
            .into_iter()
            .map(|run| {
                let duration_sec = run.finished_at.map(|finished| {
                    let dur = finished - run.started_at;
                    format!("{:.3}", dur.as_seconds_f64())
                });

                RunJSON {
                    id: run.id,
                    eval: run.workflow_name,
                    status: run.status.to_string(),
                    samples: run.steps_count,
                    created: format_utc(run.started_at),
                    duration_sec,
                }
            })
            .collect();

        let output = serde_json::json!(ListRunsJSON { runs: runs_json });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        let mut table = Table::new();
        table
            .load_preset(NOTHING)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                "ID", "EVAL", "STATUS", "SAMPLES", "CREATED", "DURATION",
            ]);

        for run in runs {
            let duration = run.finished_at.map_or("-".to_string(), |finished| {
                format_duration(finished - run.started_at)
            });

            table.add_row(vec![
                Cell::new(run.id),
                Cell::new(run.workflow_name),
                Cell::new(run.status),
                Cell::new(run.steps_count),
                Cell::new(format_utc(run.started_at)),
                Cell::new(duration),
            ]);
        }

        println!("{table}");
    }

    Ok(())
}
