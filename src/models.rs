use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use glpk_rust::Bound;

// ---------- API response types (decoupled from the lib) ----------

#[derive(Serialize, Deserialize)]
pub enum Status {
    Undefined = 1,
    Feasible = 2,
    Infeasible = 3,
    NoFeasible = 4,
    Optimal = 5,
    Unbounded = 6,
    SimplexFailed = 7,
    MIPFailed = 8,
    EmptySpace = 9,
}

#[derive(Serialize, Deserialize)]
pub struct ApiSolution {
    pub status: Status,
    pub objective: i32, // matches glpk_rust’s current output
    pub solution: HashMap<String, i32>,
    pub error: Option<String>,
}

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

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SparseLEIntegerPolyhedron {
    pub A: ApiIntegerSparseMatrix,
    pub b: Vec<i32>, // LE right-hand side
    pub variables: Vec<ApiVariable>,
}
