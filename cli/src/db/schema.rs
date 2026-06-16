use anyhow::{Context, Result};
use sea_orm::schema::Schema;
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityName, Statement};

use crate::db::entities::{event, step, workflow, workflow_run};

pub(super) async fn apply_schema(db: &DatabaseConnection) -> Result<()> {
    db.execute(Statement::from_string(
        db.get_database_backend(),
        "PRAGMA foreign_keys = ON".to_owned(),
    ))
    .await
    .context("failed to enable foreign keys")?;

    let schema = Schema::new(db.get_database_backend());

    macro_rules! create_table {
        ($mod:ident) => {{
            let mut stmt = schema.create_table_from_entity($mod::Entity);
            stmt.if_not_exists();
            db.execute(db.get_database_backend().build(&stmt))
                .await
                .with_context(|| {
                    format!("failed to create table for {}", $mod::Entity.table_name())
                })?;

            for index in schema.create_index_from_entity($mod::Entity) {
                db.execute(db.get_database_backend().build(&index))
                    .await
                    .with_context(|| {
                        format!("failed to create index for {}", $mod::Entity.table_name())
                    })?;
            }
        }};
    }

    create_table!(workflow);
    create_table!(workflow_run);
    create_table!(step);
    create_table!(event);

    Ok(())
}
