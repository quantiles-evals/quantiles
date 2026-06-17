use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct QuantilesClient {
    base_url: String,
    http: Client,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunResponse {
    pub run_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum StepDecisionResponse {
    Run { step_id: i64 },
    Reuse { output: String },
}

#[derive(Debug, Serialize)]
struct CreateRunRequest<'a> {
    workflow_name: &'a str,
    input: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct BeginStepRequest<'a> {
    run_id: i64,
    step_key: &'a str,
    input_hash: &'a str,
}

#[derive(Debug, Serialize)]
struct CompleteStepRequest<'a> {
    step_id: i64,
    output: &'a str,
}

#[derive(Debug, Serialize)]
struct FailStepRequest<'a> {
    step_id: i64,
    error: &'a str,
}

#[derive(Debug, Serialize)]
struct FailRunRequest<'a> {
    error: &'a str,
}

impl QuantilesClient {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: Client::new(),
        }
    }

    /// Check whether the server is reachable.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the server returns an error
    /// status.
    pub async fn health(&self) -> Result<()> {
        self.http
            .get(self.url("/health"))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    /// Create an eval run.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, the server returns an error
    /// status, or the response cannot be decoded.
    pub async fn create_run(&self, workflow_name: &str, input: Option<&str>) -> Result<i64> {
        let response = self
            .http
            .post(self.url("/runs"))
            .json(&CreateRunRequest {
                workflow_name,
                input,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<CreateRunResponse>()
            .await?;

        Ok(response.run_id)
    }

    /// Update the output of an eval run without changing status.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the server returns an error
    /// status.
    pub async fn set_run_output(&self, run_id: i64, output: &str) -> Result<()> {
        #[derive(Debug, Serialize)]
        struct SetRunOutputRequest<'a> {
            output: &'a str,
        }
        self.http
            .post(self.url(&format!("/runs/{run_id}/output")))
            .json(&SetRunOutputRequest { output })
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    /// Mark an eval run as completed.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the server returns an error
    /// status.
    pub async fn complete_run(&self, run_id: i64) -> Result<()> {
        self.http
            .post(self.url(&format!("/runs/{run_id}/complete")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    /// Mark an eval run as failed.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the server returns an error
    /// status.
    pub async fn fail_run(&self, run_id: i64, error: &str) -> Result<()> {
        self.http
            .post(self.url(&format!("/runs/{run_id}/fail")))
            .json(&FailRunRequest { error })
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    /// Run a durable step through the HTTP API.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, the server returns an error
    /// status, the response cannot be decoded, or the execution closure fails.
    pub async fn run_step<F>(
        &self,
        run_id: i64,
        step_key: &str,
        input_hash: &str,
        execute: F,
    ) -> Result<String>
    where
        F: FnOnce() -> Result<String>,
    {
        match self.begin_step(run_id, step_key, input_hash).await? {
            StepDecisionResponse::Reuse { output } => Ok(output),
            StepDecisionResponse::Run { step_id } => self.execute_step(step_id, execute).await,
        }
    }

    async fn begin_step(
        &self,
        run_id: i64,
        step_key: &str,
        input_hash: &str,
    ) -> Result<StepDecisionResponse> {
        Ok(self
            .http
            .post(self.url("/steps/begin"))
            .json(&BeginStepRequest {
                run_id,
                step_key,
                input_hash,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<StepDecisionResponse>()
            .await?)
    }

    async fn complete_step(&self, step_id: i64, output: &str) -> Result<()> {
        self.http
            .post(self.url("/steps/complete"))
            .json(&CompleteStepRequest { step_id, output })
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn fail_step(&self, step_id: i64, error: &str) -> Result<()> {
        self.http
            .post(self.url("/steps/fail"))
            .json(&FailStepRequest { step_id, error })
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn execute_step<F>(&self, step_id: i64, execute: F) -> Result<String>
    where
        F: FnOnce() -> Result<String>,
    {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(execute))
            .map_err(|_panic| anyhow::anyhow!("step panicked"))?;

        let output = match result {
            Ok(output) => output,
            Err(err) => {
                let message = format!("{err:#}");
                self.fail_step(step_id, &message).await?;
                return Err(err);
            }
        };

        self.complete_step(step_id, &output).await?;
        Ok(output)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}
