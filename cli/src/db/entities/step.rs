use crate::db::summaries::StepStatus;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "steps")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub run_id: i64,
    pub step_key: String,
    pub input_hash: String,
    pub status: StepStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub started_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::workflow_run::Entity",
        from = "Column::RunId",
        to = "super::workflow_run::Column::Id"
    )]
    WorkflowRun,
    #[sea_orm(has_many = "super::event::Entity")]
    Events,
}

impl Related<super::workflow_run::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkflowRun.def()
    }
}

impl Related<super::event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Events.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
