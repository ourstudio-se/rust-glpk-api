use std::collections::{HashMap, HashSet};

use glpk_rust::{
    solve_ilps as glpk_solve_ilps,
    SparseLEIntegerPolyhedron as GlpkPoly, Solution, Variable
};

pub struct SolveInputError{
    pub details: String,
}

pub fn solve_inner(
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
    let solutions = glpk_solve_ilps(
        &mut mut_polyhedron, 
        objectives, 
        maximize,
         false,
        );
        
    return Ok(solutions);
}

fn validate_objectives(
    variables: &Vec<Variable>,
    objectives: &Vec<HashMap<&str, f64>>,
) -> Result<(), SolveInputError> {
    let variable_ids: HashSet<&str> = variables
        .iter()
        .map(|v| v.id)
        .collect();


    for objective in objectives {
        for (objective_variable, _) in objective {
            if !variable_ids.contains(objective_variable) {
                return Err(
                    SolveInputError{ 
                        details: format!(
                            "Objective contains missing variable {}", 
                            objective_variable,
                        ),
                    },
                );
            }
        }
    }

    Ok(())
}