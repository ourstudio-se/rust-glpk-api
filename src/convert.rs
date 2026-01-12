use crate::models::{
    ObjectiveOwned, 
    SparseLEIntegerPolyhedron,
    ApiIntegerSparseMatrix,
};
use std::collections::HashMap;

use glpk_rust::{
    Bound,
    IntegerSparseMatrix as GlpkMatrix,
    SparseLEIntegerPolyhedron as GlpkPoly, 
    Variable as GlpkVar,
};

pub fn to_many_borrowed_objectives(objectives: &Vec<ObjectiveOwned>) -> Vec<HashMap<&str, f64>> {
    let mut borrowed_objectives: Vec<HashMap<&str, f64>> = Vec::with_capacity(objectives.len());
    for obj in objectives {
        let borrowed_obj = to_borrowed_objective(obj);
        borrowed_objectives.push(borrowed_obj);
    }
    return borrowed_objectives;
}

pub fn to_borrowed_objective(obj: &ObjectiveOwned) -> HashMap<&str, f64> {
    let mut borrowed_obj: HashMap<&str, f64> = HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        borrowed_obj.insert(k.as_str(), *v);
    }
    return borrowed_obj;
}

/// Convert an API LE polyhedron to a GLPK LE polyhedron by building borrowed variables.
pub fn to_glpk_polyhedron<'a>(le: &'a SparseLEIntegerPolyhedron) -> GlpkPoly<'a> {
    let a = api_matrix_to_glpk(&le.A);
    let b: Vec<Bound> = le.b
        .iter()
        .map(|&v| (0, v))
        .collect();

    let variables: Vec<GlpkVar<'a>> = le
        .variables
        .iter()
        .map(|v| GlpkVar {
            id: v.id.as_str(),  // borrow directly from ApiVariable
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

fn api_matrix_to_glpk(m: &ApiIntegerSparseMatrix) -> GlpkMatrix {
    GlpkMatrix {
        rows: m.rows.clone(),
        cols: m.cols.clone(),
        vals: m.vals.clone(),
    }
}
