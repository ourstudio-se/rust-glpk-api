use std::collections::HashMap;

use crate::domain::validate::{validate_objectives_owned, SolveInputError};

use glpk_rust::{solve_ilps as glpk_solve_ilps, Solution, SparseLEIntegerPolyhedron as GlpkPoly};

const NO_TERMINAL_OUTPUT: bool = false;

pub fn solve(
    polyhedron: GlpkPoly,
    objectives: Vec<HashMap<&str, f64>>,
    maximize: bool,
) -> Result<Vec<Solution>, SolveInputError> {
    // Convert objectives to Vec<HashMap<String, f64>>
    let objectives_owned: Vec<HashMap<String, f64>> = objectives
        .clone()
        .iter()
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect()
        })
        .collect();

    // Validate objectives
    match validate_objectives_owned(&polyhedron.variables, &objectives_owned) {
        Ok(_) => (),
        Err(error) => return Err(error),
    }

    // Solver expects &mut
    let mut mut_polyhedron = polyhedron;

    // Call the library solver
    let solutions = glpk_solve_ilps(
        &mut mut_polyhedron,
        objectives,
        maximize,
        NO_TERMINAL_OUTPUT,
    );

    Ok(solutions)
}
