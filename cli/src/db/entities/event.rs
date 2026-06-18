use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    /// Unique ID of the model
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Unique ID of the run
    pub run_id: i64,
    /// Unique ID of the step, inside the run
    pub step_id: Option<i64>,
    /// Type of the event
    ///
    /// TODO: make this a discriminated union if possible
    pub event_type: String,
    /// Optional explanatory message about the event
    pub message: Option<String>,
    /// Optional additional metadata abou the event
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
