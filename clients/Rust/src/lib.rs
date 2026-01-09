//! # GLPK API Client
//!
//! A Rust client SDK for interacting with the GLPK REST API for solving linear programming problems.
//!
//! ## Example
//!
//! ```no_run
//! use glpk_api_sdk::{GlpkClient, SolveRequestBuilder, Variable, SolverDirection};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = GlpkClient::new("http://localhost:9000")?;
//!
//!     let request = SolveRequestBuilder::new()
//!         .add_variable(Variable::new("x1", 0, 1))
//!         .add_variable(Variable::new("x2", 0, 1))
//!         .add_variable(Variable::new("x3", 0, 1))
//!         .add_constraint(vec![1, 1, 0], vec![0, 1, 0], vec![1, 1, 0], 1)
//!         .add_constraint(vec![1, 1, 0], vec![0, 2, 0], vec![1, 1, 0], 1)
//!         .add_constraint(vec![1, 1, 0], vec![1, 2, 0], vec![1, 1, 0], 1)
//!         .add_objective([("x3", 1.0)].into())
//!         .direction(SolverDirection::Maximize)
//!         .build()?;
//!
//!     let response = client.solve(request).await?;
//!     println!("Solutions: {:?}", response.solutions);
//!     Ok(())
//! }
//! ```

pub mod types;
pub mod client;
pub mod builder;
pub mod error;

pub use client::GlpkClient;
pub use types::{
    SolveRequest, SolveResponse, Variable, IntegerSparseMatrix, Shape,
    SparseLEIntegerPolyhedron, SolverDirection, Solution, Status,
};
pub use builder::SolveRequestBuilder;
pub use error::{GlpkError, Result};
