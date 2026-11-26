use crate::domain::outcome::SolveOutcome;
use crate::models::{SolverDirection, SparseLEIntegerPolyhedron};
use std::collections::HashMap;

pub use crate::domain::validate::{SolveError, SolveErrorType};

/// Common interface for LP/ILP solvers
pub trait Solver: Send + Sync {
    /// Solve one or more linear programming problems
    ///
    /// # Arguments
    /// * `polyhedron` - The constraint polyhedron (Ax <= b with variable bounds)
    /// * `objectives` - List of objective functions to optimize
    /// * `direction` - Maximize or Minimize
    /// * `use_presolve` - Enable/disable presolve optimization
    ///
    /// # Returns
    /// One `SolveOutcome` per objective function, in the same order.
    /// `Err` is only returned when the request as a whole cannot be solved
    /// (e.g. the model cannot be built or the time limit is exceeded).
    fn solve(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        objectives: &[HashMap<String, f64>],
        direction: SolverDirection,
        use_presolve: bool,
    ) -> Result<Vec<SolveOutcome>, SolveError>;

    /// Get the solver name for logging/debugging
    fn name(&self) -> &str;
}
