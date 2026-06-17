from typing import Annotated, Literal

from pydantic import BaseModel, Field, TypeAdapter


class EmptyResponse(BaseModel):
  pass


class CreateRunRequest(BaseModel):
  workflow_name: str
  input: str | None = None


class CreateRunResponse(BaseModel):
  run_id: int


class RunResponse(BaseModel):
  id: int
  workflow_name: str
  status: str
  input: str | None = None
  output: str | None = None
  started_at: str
  finished_at: str | None = None
  error: str | None = None


class CompleteRunRequest(BaseModel):
  output: str | None = None


class FailRunRequest(BaseModel):
  error: str


class SetRunOutputRequest(BaseModel):
  output: str


class EmitMetricRequest(BaseModel):
  metric_name: str
  metric_value: float
  unit: str | None = None


class BeginStepRequest(BaseModel):
  run_id: int
  step_key: str
  input_hash: str


class StepDecisionRun(BaseModel):
  decision: Literal["run"]
  step_id: int


class StepDecisionReuse(BaseModel):
  decision: Literal["reuse"]
  output: str


type StepDecisionResponse = Annotated[
  StepDecisionRun | StepDecisionReuse,
  Field(discriminator="decision"),
]

step_decision_adapter: TypeAdapter[StepDecisionResponse] = TypeAdapter(StepDecisionResponse)


class CompleteStepRequest(BaseModel):
  step_id: int
  output: str


class FailStepRequest(BaseModel):
  step_id: int
  error: str
