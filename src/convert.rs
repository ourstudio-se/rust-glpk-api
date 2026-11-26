use crate::domain::solve_outcome::{self, Outcome};
use crate::models::{
    ApiIntegerSparseMatrix, ApiSolution, ObjectiveOwned, SparseLEIntegerPolyhedron, Status,
};
use std::collections::HashMap;

use glpk_rust::{
    Bound, IntegerSparseMatrix as GlpkMatrix, Solution, SparseLEIntegerPolyhedron as GlpkPoly,
    Variable as GlpkVar,
};

pub fn to_borrowed_objective(obj: &ObjectiveOwned) -> HashMap<&str, f64> {
    obj.iter().map(|(k, v)| (k.as_str(), *v)).collect()
}

/// Convert an API LE polyhedron to a GLPK LE polyhedron by building borrowed variables.
pub fn to_glpk_polyhedron<'a>(le: &'a SparseLEIntegerPolyhedron) -> GlpkPoly<'a> {
    let a = to_glpk_matrix(&le.a);
    let b: Vec<Bound> = le.b.iter().map(|&v| (0, v)).collect();

    let variables: Vec<GlpkVar<'a>> = le
        .variables
        .iter()
        .map(|v| GlpkVar {
            id: v.id.as_str(), // borrow directly from ApiVariable
            bound: v.bound,
        })
        .collect();

    GlpkPoly {
        a,
        b,
        variables,
        double_bound: false,
    }
}

fn to_glpk_matrix(m: &ApiIntegerSparseMatrix) -> GlpkMatrix {
    GlpkMatrix {
        rows: m.rows.clone(),
        cols: m.cols.clone(),
        vals: m.vals.clone(),
    }
}

impl From<Solution> for ApiSolution {
    fn from(s: Solution) -> Self {
        let status = match solve_outcome::glpk(s.status as i32, s.error.as_deref()) {
            Outcome::Optimal => Status::Optimal,
            Outcome::Feasible => Status::Feasible,
            Outcome::Failed(error) => return solve_outcome::failed_solution(error),
        };
        ApiSolution {
            status,
            objective: s.objective as i32, // Match current api contract
            solution: s
                .solution
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            error: None,
        }
    }
}
