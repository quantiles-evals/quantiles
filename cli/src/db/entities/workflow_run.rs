use crate::db::summaries::RunStatus;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "workflow_runs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub workflow_id: Option<i64>,
    pub workflow_name: String,
    pub status: RunStatus,
    pub input: Option<String>,
    pub output: Option<String>,
    pub started_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::workflow::Entity",
        from = "Column::WorkflowId",
        to = "super::workflow::Column::Id"
    )]
    Workflow,
    #[sea_orm(has_many = "super::step::Entity")]
    Steps,
    #[sea_orm(has_many = "super::event::Entity")]
    Events,
}

impl Related<super::workflow::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Workflow.def()
    }
}

impl Related<super::step::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Steps.def()
    }
}

impl Related<super::event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Events.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
