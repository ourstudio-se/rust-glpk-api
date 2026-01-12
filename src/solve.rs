use actix_web::{HttpResponse};
use std::collections::HashMap;

use crate::{
    api_le_to_glpk_le, 
    validate_solve_request, 
    ApiSolution, 
    SolveRequest, 
    SolverDirection,
};

use glpk_rust::{
    solve_ilps as glpk_solve_ilps,
    Bound, IntegerSparseMatrix as GlpkMatrix,
    SparseLEIntegerPolyhedron as GlpkPoly, Status as GlpkStatus, Variable as GlpkVar, Solution
};

/// POST /solve
pub fn solve2(req: &SolveRequest) -> Result<Vec<ApiSolution>, HttpResponse> {
    match validate_solve_request(&req) {
        Ok(_) => (),
        Err(response) => return Err(response),
    }

    // Keep owned IDs alive while GLPK borrows &str from them
    let id_storage: Vec<String> = req
        .polyhedron
        .variables
        .iter()
        .map(|v| v.id.clone())
        .collect();

    // Build a quick intern map (&str -> &str) so we can map objective keys to the same &strs as variables
    let mut intern: HashMap<&str, &str> = HashMap::with_capacity(id_storage.len());
    for s in &id_storage {
        intern.insert(s.as_str(), s.as_str());
    }

    // Build a borrowed LE polyhedron for the solver
    let glpk_polyhedron = api_le_to_glpk_le(&req.polyhedron, &id_storage);

    // Convert objectives from HashMap<String, f64> → HashMap<&str, f64>
    // and ignore objective vars not in the polytope (as per your spec).
    let mut borrowed_objectives: Vec<HashMap<&str, f64>> = Vec::with_capacity(req.objectives.len());
    for obj in &req.objectives {
        let mut bobj: HashMap<&str, f64> = HashMap::with_capacity(obj.len());
        for (k, v) in obj {
            if let Some(&interned) = intern.get(k.as_str()) {
                bobj.insert(interned, *v);
            }
            // else: silently ignore unknown var (per your comment)
        }
        borrowed_objectives.push(bobj);
    }

    let maximize = req.direction == SolverDirection::Maximize;

    let solve_result = solve_inner(
        glpk_polyhedron, 
        borrowed_objectives, 
        maximize,
    );

    let lib_solutions: Vec<Solution>;
    match solve_result {
        Ok(solutions) => lib_solutions = solutions,
        Err(_) => return Err(HttpResponse::InternalServerError().json("Something went wrong")),
    }

    // Map library solutions → API solutions with owned Strings
    let api_solutions: Vec<ApiSolution> = lib_solutions
        .into_iter()
        .map(|s| ApiSolution {
            status: s.status.into(),
            objective: s.objective,
            solution: s
                .solution
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            error: s.error,
        })
        .collect();

    return Ok(api_solutions);
}

enum SolveError {
    InvalidInput,
}

/// POST /solve
fn solve_inner(
    polyhedron: GlpkPoly,
    objectives: Vec<HashMap<&str, f64>>,
    maximize: bool,
) -> Result<Vec<Solution>, SolveError> {
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