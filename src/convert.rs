use crate::models::{
    ApiIntegerSparseMatrix, ApiSolution, ObjectiveOwned, SparseLEIntegerPolyhedron, Status,
};
use std::collections::HashMap;

use glpk_rust::{
    Bound, IntegerSparseMatrix as GlpkMatrix, Solution, SparseLEIntegerPolyhedron as GlpkPoly,
    Status as GlpkStatus, Variable as GlpkVar,
};

pub fn to_many_borrowed_objectives(objectives: &Vec<ObjectiveOwned>) -> Vec<HashMap<&str, f64>> {
    let mut borrowed_objectives: Vec<HashMap<&str, f64>> = Vec::with_capacity(objectives.len());
    for obj in objectives {
        let borrowed_obj = to_borrowed_objective(obj);
        borrowed_objectives.push(borrowed_obj);
    }
    borrowed_objectives
}

pub fn to_borrowed_objective(obj: &ObjectiveOwned) -> HashMap<&str, f64> {
    let mut borrowed_obj: HashMap<&str, f64> = HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        borrowed_obj.insert(k.as_str(), *v);
    }
    borrowed_obj
}

/// Convert an API LE polyhedron to a GLPK LE polyhedron by building borrowed variables.
pub fn to_glpk_polyhedron<'a>(le: &'a SparseLEIntegerPolyhedron) -> GlpkPoly<'a> {
    let a = to_glpk_matrix(&le.A);
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

impl From<GlpkStatus> for Status {
    fn from(s: GlpkStatus) -> Self {
        // Assumes your crate uses the same variant names
        match s {
            GlpkStatus::Undefined => Status::Undefined,
            GlpkStatus::Feasible => Status::Feasible,
            GlpkStatus::Infeasible => Status::Infeasible,
            GlpkStatus::NoFeasible => Status::NoFeasible,
            GlpkStatus::Optimal => Status::Optimal,
            GlpkStatus::Unbounded => Status::Unbounded,
            GlpkStatus::SimplexFailed => Status::SimplexFailed,
            GlpkStatus::MIPFailed => Status::MIPFailed,
            GlpkStatus::EmptySpace => Status::EmptySpace,
        }
    }
}

impl From<Solution> for ApiSolution {
    fn from(s: Solution) -> Self {
        ApiSolution {
            status: s.status.into(),
            objective: s.objective,
            solution: s
                .solution
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            error: s.error,
        }
    }
}
