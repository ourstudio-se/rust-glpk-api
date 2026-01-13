use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Variable bounds (lower_bound, upper_bound)
pub type Bound = (i32, i32);

/// A variable in the linear programming problem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    /// Variable identifier
    pub id: String,
    /// Variable bounds (lower, upper)
    pub bound: Bound,
}

impl Variable {
    /// Create a new variable with the given id and bounds
    pub fn new(id: impl Into<String>, lower: i32, upper: i32) -> Self {
        Self {
            id: id.into(),
            bound: (lower, upper),
        }
    }
}

/// Matrix shape specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    /// Number of rows
    pub nrows: usize,
    /// Number of columns
    pub ncols: usize,
}

/// Sparse matrix representation using coordinate format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegerSparseMatrix {
    /// Row indices (0-based)
    pub rows: Vec<i32>,
    /// Column indices (0-based)
    pub cols: Vec<i32>,
    /// Values at the specified positions
    pub vals: Vec<i32>,
    /// Matrix dimensions
    pub shape: Shape,
}

impl IntegerSparseMatrix {
    /// Create a new sparse matrix
    pub fn new(rows: Vec<i32>, cols: Vec<i32>, vals: Vec<i32>, nrows: usize, ncols: usize) -> Self {
        Self {
            rows,
            cols,
            vals,
            shape: Shape { nrows, ncols },
        }
    }
}

/// A polyhedron defined by linear constraints Ax ≤ b
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseLEIntegerPolyhedron {
    /// Constraint coefficient matrix
    #[serde(rename = "A")]
    pub a: IntegerSparseMatrix,
    /// Right-hand side constraint values
    pub b: Vec<i32>,
    /// Decision variables
    pub variables: Vec<Variable>,
}

/// Direction for optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SolverDirection {
    /// Maximize the objective function
    Maximize,
    /// Minimize the objective function
    Minimize,
}

/// Objective function as a mapping from variable names to coefficients
pub type Objective = HashMap<String, f64>;

/// Request to solve one or more linear programming problems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveRequest {
    /// The constraint polyhedron
    pub polyhedron: SparseLEIntegerPolyhedron,
    /// One or more objective functions to optimize
    pub objectives: Vec<Objective>,
    /// Whether to maximize or minimize
    pub direction: SolverDirection,
}

/// Solution status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Solution status is undefined
    Undefined = 1,
    /// Solution is feasible
    Feasible = 2,
    /// Problem is infeasible
    Infeasible = 3,
    /// No feasible solution exists
    NoFeasible = 4,
    /// Optimal solution found
    Optimal = 5,
    /// Problem is unbounded
    Unbounded = 6,
    /// Simplex method failed
    SimplexFailed = 7,
    /// Mixed-integer programming failed
    MIPFailed = 8,
    /// Search space is empty
    EmptySpace = 9,
}

/// A single solution for one objective function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    /// Solution status
    pub status: Status,
    /// Objective value achieved
    pub objective: i32,
    /// Variable assignments
    pub solution: HashMap<String, i64>,
    /// Error message, if any
    pub error: Option<String>,
}

/// Response from the solve endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveResponse {
    /// One solution per objective function
    pub solutions: Vec<Solution>,
}
