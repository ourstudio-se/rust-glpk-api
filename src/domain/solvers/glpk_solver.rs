use crate::convert::{to_borrowed_objective, to_glpk_polyhedron};
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveError};
use crate::models::{ApiSolution, SolverDirection, SparseLEIntegerPolyhedron};
use glpk_rust::solve_ilps;
use std::collections::HashMap;

const NO_TERMINAL_OUTPUT: bool = false;

/// GLPK solver implementation
///
/// Note: GLPK does not support model caching due to its mutable API design.
/// The cache_size parameter is accepted for API consistency but has no effect.
pub struct GlpkSolver {
    timeout_ms: Option<u32>,
}

impl GlpkSolver {
    /// Create a new GLPK solver with specified cache size
    /// Note: Cache is not supported for GLPK, parameter ignored
    pub fn with_cache_size(_size: Option<usize>, timeout_ms: Option<u32>) -> Self {
        GlpkSolver { timeout_ms }
    }

    /// Create solver with caching disabled (same as default for GLPK)
    pub fn without_cache(timeout_ms: Option<u32>) -> Self {
        GlpkSolver { timeout_ms }
    }
}

impl Solver for GlpkSolver {
    fn solve(
        &self,
        polyhedron: SparseLEIntegerPolyhedron,
        objectives: Vec<HashMap<String, f64>>,
        direction: SolverDirection,
        _use_presolve: bool,
    ) -> Result<Vec<ApiSolution>, SolveError> {
        let glpk_polyhedron = to_glpk_polyhedron(&polyhedron);

        // Validate objectives against variables
        validate_objectives_owned(&glpk_polyhedron.variables, &objectives)?;

        // Convert to borrowed objectives for GLPK
        let borrowed_objectives: Vec<HashMap<&str, f64>> = objectives
            .iter()
            .map(|obj| to_borrowed_objective(obj))
            .collect();

        let maximize = direction == SolverDirection::Maximize;

        // Solver expects &mut
        let mut mut_polyhedron = glpk_polyhedron;

        // Call the GLPK library solver
        let lib_solutions = solve_ilps(
            &mut mut_polyhedron,
            borrowed_objectives,
            maximize,
            _use_presolve,
            NO_TERMINAL_OUTPUT,
            self.timeout_ms,
        )?;

        // Convert GLPK solutions to API solutions
        let api_solutions: Vec<ApiSolution> = lib_solutions.into_iter().map(|s| s.into()).collect();

        Ok(api_solutions)
    }

    fn name(&self) -> &str {
        "GLPK"
    }
}
