use std::collections::HashMap;

use crate::domain::validate::{validate_objectives, SolveInputError};

use glpk_rust::{solve_ilps as glpk_solve_ilps, Solution, SparseLEIntegerPolyhedron as GlpkPoly};

pub fn solve(
    polyhedron: GlpkPoly,
    objectives: Vec<HashMap<&str, f64>>,
    maximize: bool,
) -> Result<Vec<Solution>, SolveInputError> {
    match validate_objectives(&polyhedron.variables, &objectives) {
        Ok(_) => (),
        Err(error) => return Err(error),
    }

    // Solver expects &mut
    let mut mut_polyhedron = polyhedron;

    // Call the library solver
    let solutions = glpk_solve_ilps(&mut mut_polyhedron, objectives, maximize, false);

    Ok(solutions)
}
