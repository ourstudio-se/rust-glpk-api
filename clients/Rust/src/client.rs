use crate::error::{GlpkError, Result};
use crate::types::{SolveRequest, SolveResponse};
use reqwest::{Client, Url};

/// HTTP client for interacting with the GLPK REST API
#[derive(Debug, Clone)]
pub struct GlpkClient {
    client: Client,
    base_url: Url,
    api_key: Option<String>,
}

impl GlpkClient {
    /// Create a new GLPK API client
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the GLPK API (e.g., "http://localhost:9000")
    ///
    /// # Example
    ///
    /// ```no_run
    /// use glpk_api_sdk::GlpkClient;
    ///
    /// let client = GlpkClient::new("http://localhost:9000").unwrap();
    /// ```
    pub fn new(base_url: impl AsRef<str>) -> Result<Self> {
        let base_url = Url::parse(base_url.as_ref())
            .map_err(|e| GlpkError::InvalidUrl(e.to_string()))?;

        Ok(Self {
            client: Client::new(),
            base_url,
            api_key: None,
        })
    }

    /// Create a new GLPK API client with custom reqwest client
    ///
    /// This allows you to configure timeouts, proxies, etc.
    pub fn with_client(base_url: impl AsRef<str>, client: Client) -> Result<Self> {
        let base_url = Url::parse(base_url.as_ref())
            .map_err(|e| GlpkError::InvalidUrl(e.to_string()))?;

        Ok(Self {
            client,
            base_url,
            api_key: None,
        })
    }

    /// Set the API key for authentication
    ///
    /// Use this when the API is running in protected mode (PROTECT=true)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use glpk_api_sdk::GlpkClient;
    ///
    /// let client = GlpkClient::new("http://localhost:9000")
    ///     .unwrap()
    ///     .with_api_key("your-api-key");
    /// ```
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Check the health of the API server
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use glpk_api_sdk::GlpkClient;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = GlpkClient::new("http://localhost:9000")?;
    /// let is_healthy = client.health_check().await?;
    /// println!("Server healthy: {}", is_healthy);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health_check(&self) -> Result<bool> {
        let url = self.base_url.join("/health")
            .map_err(|e| GlpkError::InvalidUrl(e.to_string()))?;

        let response = self.client.get(url).send().await?;
        Ok(response.status().is_success())
    }

    /// Solve one or more linear programming problems
    ///
    /// # Arguments
    ///
    /// * `request` - The solve request containing the polyhedron, objectives, and direction
    ///
    /// # Returns
    ///
    /// A response containing one solution for each objective function
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use glpk_api_sdk::{GlpkClient, SolveRequestBuilder, Variable, SolverDirection};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = GlpkClient::new("http://localhost:9000")?;
    ///
    /// let request = SolveRequestBuilder::new()
    ///     .add_variable(Variable::new("x1", 0, 100))
    ///     .add_variable(Variable::new("x2", 0, 100))
    ///     .add_constraint(vec![1, 1], vec![0, 1], vec![2, 3], 10)
    ///     .add_objective([("x1", 1.0), ("x2", 2.0)].into())
    ///     .direction(SolverDirection::Maximize)
    ///     .build()?;
    ///
    /// let response = client.solve(request).await?;
    ///
    /// for solution in &response.solutions {
    ///     println!("Status: {:?}", solution.status);
    ///     println!("Objective: {}", solution.objective);
    ///     println!("Solution: {:?}", solution.solution);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn solve(&self, request: SolveRequest) -> Result<SolveResponse> {
        let url = self.base_url.join("/solve")
            .map_err(|e| GlpkError::InvalidUrl(e.to_string()))?;

        let mut req_builder = self.client.post(url).json(&request);

        // Add API key header if set
        if let Some(ref api_key) = self.api_key {
            req_builder = req_builder.header("X-API-Key", api_key);
        }

        let response = req_builder.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(match status.as_u16() {
                401 | 403 => GlpkError::AuthenticationFailed,
                _ => GlpkError::ApiError(error_text),
            });
        }

        let solve_response: SolveResponse = response
            .json()
            .await
            .map_err(|e| GlpkError::ParseError(e.to_string()))?;

        Ok(solve_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = GlpkClient::new("http://localhost:9000");
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_api_key() {
        let client = GlpkClient::new("http://localhost:9000")
            .unwrap()
            .with_api_key("test-key");
        assert_eq!(client.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_invalid_url() {
        let client = GlpkClient::new("not a valid url");
        assert!(client.is_err());
    }
}
