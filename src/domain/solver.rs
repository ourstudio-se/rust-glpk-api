use crate::domain::validate::SolveInputError;
use crate::models::{ApiSolution, SolverDirection, SparseLEIntegerPolyhedron};
use std::collections::HashMap;

/// Common interface for LP/ILP solvers
pub trait Solver: Send + Sync {
    /// Solve one or more linear programming problems
    ///
    /// # Arguments
    /// * `polyhedron` - The constraint polyhedron (Ax <= b with variable bounds)
    /// * `objectives` - List of objective functions to optimize
    /// * `direction` - Maximize or Minimize
    /// * `use_presolve` - Enable/disable presolve optimization
    /// * `time_limit` - Optional time limit in seconds (None = no limit)
    ///
    /// # Returns
    /// A vector of solutions, one for each objective function
    fn solve(
        &self,
        polyhedron: SparseLEIntegerPolyhedron,
        objectives: Vec<HashMap<String, f64>>,
        direction: SolverDirection,
        use_presolve: bool,
        time_limit: Option<f64>,
    ) -> Result<Vec<ApiSolution>, SolveInputError>;

    /// Get the solver name for logging/debugging
    fn name(&self) -> &str;
}
