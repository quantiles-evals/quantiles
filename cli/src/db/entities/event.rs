use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub run_id: i64,
    pub step_id: Option<i64>,
    pub event_type: String,
    pub message: Option<String>,
    pub data: Option<String>,
    pub created_at: time::OffsetDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::workflow_run::Entity",
        from = "Column::RunId",
        to = "super::workflow_run::Column::Id"
    )]
    WorkflowRun,
    #[sea_orm(
        belongs_to = "super::step::Entity",
        from = "Column::StepId",
        to = "super::step::Column::Id"
    )]
    Step,
}

impl Related<super::workflow_run::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkflowRun.def()
    }
}

impl Related<super::step::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Step.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
