use std::collections::HashMap;
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveInputError};
use crate::models::{ApiSolution, SparseLEIntegerPolyhedron, SolverDirection};
use crate::convert::{to_glpk_polyhedron, to_borrowed_objective};

use glpk_rust::{solve_ilps as glpk_solve_ilps, Solution};

const NO_TERMINAL_OUTPUT: bool = false;

/// GLPK solver implementation
pub struct GlpkSolver;

impl GlpkSolver {
    pub fn new() -> Self {
        GlpkSolver
    }
}

impl Solver for GlpkSolver {
    fn solve(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        objectives: &[HashMap<String, f64>],
        direction: SolverDirection,
        _use_presolve: bool,
    ) -> Result<Vec<ApiSolution>, SolveInputError> {
        let glpk_polyhedron = to_glpk_polyhedron(polyhedron);

        // Validate objectives against variables
        validate_objectives_owned(&glpk_polyhedron.variables, objectives)?;

        // Convert to borrowed objectives for GLPK
        let borrowed_objectives: Vec<HashMap<&str, f64>> = objectives
            .iter()
            .map(|obj| to_borrowed_objective(obj))
            .collect();

        let maximize = direction == SolverDirection::Maximize;

        // Solver expects &mut
        let mut mut_polyhedron = glpk_polyhedron;

        // Call the GLPK library solver
        let lib_solutions: Vec<Solution> = glpk_solve_ilps(
            &mut mut_polyhedron,
            borrowed_objectives,
            maximize,
            NO_TERMINAL_OUTPUT,
        );

        // Convert GLPK solutions to API solutions
        let api_solutions: Vec<ApiSolution> = lib_solutions
            .into_iter()
            .map(|s| s.into())
            .collect();

        Ok(api_solutions)
    }

    fn name(&self) -> &str {
        "GLPK"
    }
}
