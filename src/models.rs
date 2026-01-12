use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use glpk_rust::{
    Bound,
};

// ---------- API (wire) types: owned & serde-friendly ----------

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiVariable {
    pub id: String,
    pub bound: Bound, // (i32, i32) from glpk_rust
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiShape {
    pub nrows: usize,
    pub ncols: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiIntegerSparseMatrix {
    pub rows: Vec<i32>,
    pub cols: Vec<i32>,
    pub vals: Vec<i32>,
    pub shape: ApiShape,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SolverDirection {
    Maximize,
    Minimize,
}

pub type ObjectiveOwned = HashMap<String, f64>;

#[derive(Deserialize)]
pub struct SolveRequest {
    pub polyhedron: SparseLEIntegerPolyhedron,
    pub objectives: Vec<ObjectiveOwned>,
    pub direction: SolverDirection,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SparseLEIntegerPolyhedron {
    pub A: ApiIntegerSparseMatrix,
    pub b: Vec<i32>, // LE right-hand side
    pub variables: Vec<ApiVariable>,
}