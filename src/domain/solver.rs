use std::collections::HashMap;
use crate::models::{ApiSolution, SparseLEIntegerPolyhedron, SolverDirection};
use crate::domain::validate::SolveInputError;

/// Common interface for LP/ILP solvers
pub trait Solver: Send + Sync {
    /// Solve one or more linear programming problems
    ///
    /// # Arguments
    /// * `polyhedron` - The constraint polyhedron (Ax <= b with variable bounds)
    /// * `objectives` - List of objective functions to optimize
    /// * `direction` - Maximize or Minimize
    ///
    /// # Returns
    /// A vector of solutions, one for each objective function
    fn solve(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        objectives: &[HashMap<String, f64>],
        direction: SolverDirection,
    ) -> Result<Vec<ApiSolution>, SolveInputError>;

    /// Get the solver name for logging/debugging
    fn name(&self) -> &str;
}
