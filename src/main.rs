extern crate glpk_sys as glpk;
use libc;

use libc::c_int;
use std::collections::HashMap;
use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use serde::{Deserialize, Serialize};

type Bound = (i32, i32);
type ID = String;
type Objective = HashMap<ID, i32>;
type Interpretation = HashMap<ID, i64>;

// 1    GLP_FR      Free variable (−∞ < x < ∞) 
// 2    GLP_LO      Variable with lower bound (x ≥ l) 
// 3    GLP_UP      Variable with upper bound (x ≤ u) 
// 4    GLP_DB      Double-bounded variable (l ≤ x ≤ u) 
// 5    GLP_FX      Fixed variable (x = l = u) 

// Value	Symbol	Meaning	Example
// 1	GLP_FR	Free (no bounds)	−∞<x<∞
// 2	GLP_LO	Lower bound only	x≥l
// 3	GLP_UP	Upper bound only	𝑥≤𝑢
// 4	GLP_DB	Double bounded (both bounds)	l≤x≤u
// 5	GLP_FX	Fixed (both bounds are equal)	x=l=u

// Variable kind
// 1 - continuous, 2 - integer, 3 - binary

// Solution status
// GLP_UNDEF   1  /* Solution is undefined */
// GLP_FEAS    2  /* Solution is feasible */
// GLP_INFEAS  3  /* No feasible solution exists */
// GLP_NOFEAS  4  /* No feasible solution exists (dual) */
// GLP_OPT     5  /* Optimal solution found */
// GLP_UNBND   6  /* Problem is unbounded */

#[derive(Serialize, Deserialize)]
enum Status {
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
struct Variable {
    id: ID,
    bound: Bound,
}

#[derive(Serialize, Deserialize)]
struct IntegerSparseMatrix {
    rows: Vec<i32>,
    cols: Vec<i32>,
    vals: Vec<i32>,
}

#[derive(Serialize, Deserialize)]
struct SparseIntegerPolyhedron {
    A: IntegerSparseMatrix,
    b: Vec<Bound>,
    variables: Vec<Variable>,
}

#[derive(Serialize, Deserialize)]
struct Solution {
    status: Status,
    objective: i32,
    interpretation: Interpretation,
}

/// Solves a set of integer linear programming (ILP) problems defined by the given polytope and objectives.
/// The function ignores objective variables not present in the polytope. All other variables not provided in objective but present in polytope will be set to 0.
///
/// # Arguments
///
/// * `polytope` - A reference to a `Polytope` struct that defines the constraints and variables of the ILP.
/// * `objectives` - A vector of `Objective` structs that define the objective functions to be optimized.
/// * `term_out` - A boolean flag indicating whether to enable terminal output for the GLPK solver.
///
/// # Returns
///
/// A vector of `Solution` structs, each representing the result of solving the ILP for one of the objectives.
///
/// # Panics
///
/// This function will panic if:
/// * The lengths of the rows, columns, and values in the constraint matrix `A` are not equal.
/// * The number of variables in the polytope does not match the maximum column index in the constraint matrix.
///
/// # Examples
///
/// ```
/// let polytope = Polytope {
///     A: IntegerSparseMatrix { rows: vec![...], cols: vec![...], vals: vec![...] },
///     b: vec![...],
///     variables: vec![...],
/// };
/// let objectives = vec![Objective { ... }, Objective { ... }];
/// let solutions = solve_ilps(&polytope, objectives, true);
/// for solution in solutions {
///     println!("{:?}", solution);
/// }
/// ``
fn solve_ilps(polytope: &SparseIntegerPolyhedron, objectives: Vec<Objective>, maximize: bool, term_out: bool) -> Vec<Solution> {

    let mut solutions: Vec<Solution> = Vec::new();

    // Check if rows, columns and values are the same lenght. Else panic
    if (polytope.A.rows.len() != polytope.A.cols.len()) || (polytope.A.rows.len() != polytope.A.vals.len()) || (polytope.A.cols.len() != polytope.A.vals.len()) {
        panic!("Rows, columns and values must have the same length, got ({},{},{})", polytope.A.rows.len(), polytope.A.cols.len(), polytope.A.vals.len());
    }

    // If the polytope is empty, return an empty space status solution
    if polytope.A.rows.is_empty() || polytope.A.cols.is_empty() {
        for _ in 0..objectives.len() {
            solutions.push(Solution { status: Status::EmptySpace, objective: 0, interpretation: Interpretation::new() });
        }
        return solutions;
    }

    // Check that the max number of columns is equal to the number of provided variables
    let n_cols = (*polytope.A.cols.iter().max().unwrap() + 1) as usize;
    if polytope.variables.len() != n_cols {
        panic!("The number of variables must be equal to the maximum column index in the constraint matrix, got ({},{})", polytope.variables.len(), n_cols);
    }

    let n_rows = (*polytope.A.rows.iter().max().unwrap() + 1) as usize;
    if n_rows != polytope.b.len() {
        panic!("The number of rows in the constraint matrix must be equal to the number of elements in the b vector, got ({},{})", n_rows, polytope.b.len());
    }

    unsafe {

        // Enable or disable terminal output
        glpk::glp_term_out(term_out as c_int);

        // Create the problem
        let direction = if maximize { 2 } else { 1 };
        let lp = glpk::glp_create_prob();
        glpk::glp_set_obj_dir(lp, direction);

        // Add constraints (rows)
        glpk::glp_add_rows(lp, polytope.b.len() as i32);
        for (i, &b) in polytope.b.iter().enumerate() {
            glpk::glp_set_row_bnds(lp, (i + 1) as i32, 3, b.0 as f64, b.1 as f64);
        }

        // Add variables
        glpk::glp_add_cols(lp, polytope.variables.len() as i32);
        for (i, var) in polytope.variables.iter().enumerate() {
            
            // Set col bounds
            if var.bound.0 == var.bound.1 {
                glpk::glp_set_col_bnds(lp, (i + 1) as i32, 5, var.bound.0 as f64, var.bound.1 as f64);
            } else {
                glpk::glp_set_col_bnds(lp, (i + 1) as i32, 4, var.bound.0 as f64, var.bound.1 as f64);
            }

            // Set col kind - either integer (3) or binary (2) in this case
            if var.bound.0 == 0 && var.bound.1 == 1 {
                glpk::glp_set_col_kind(lp, (i + 1) as i32, 2);
            } else {
                glpk::glp_set_col_kind(lp, (i + 1) as i32, 3);
            };
        }

        // Set the constraint matrix
        let rows: Vec<i32> = std::iter::once(0).chain(polytope.A.rows.iter().map(|x| *x + 1)).collect();
        let cols: Vec<i32> = std::iter::once(0).chain(polytope.A.cols.iter().map(|x| *x + 1)).collect();
        let vals_f64: Vec<f64> = std::iter::once(0.0).chain(polytope.A.vals.iter().map(|x| *x as f64)).collect();

        // ne: The number of non-zero elements in the constraint matrix (not the number of rows or columns).
        // ia: An array of row indices (1-based) for each non-zero element.
        // ja: An array of column indices (1-based) for each non-zero element.
        // ar: An array of values corresponding to each non-zero element.
        glpk::glp_load_matrix(
            lp, 
            (vals_f64.len()-1) as i32, 
            rows.as_ptr(), 
            cols.as_ptr(), 
            vals_f64.as_ptr()
        );

        // Solve for multiple objectives
        for obj in objectives.iter() {

            // Setup empty solution
            let mut solution = Solution { status: Status::Undefined, objective: 0, interpretation: Interpretation::new() };

            // Update the objective function
            for (j, v) in polytope.variables.iter().enumerate() {
                let coef = obj.get(&v.id).unwrap_or(&0);
                glpk::glp_set_obj_coef(lp, (j + 1) as i32, *coef as f64);
            }

            // Solve the LP relaxation with warm start
            let mut simplex_params = glpk::glp_smcp::default();
            glpk::glp_init_smcp(&mut simplex_params);
            simplex_params.presolve = 0;
            simplex_params.msg_lev = 1;
            let simplex_ret = glpk::glp_simplex(lp, &mut simplex_params);
            if simplex_ret != 0 {
                solution.status = Status::SimplexFailed;
                continue;
            }

            // **Now solve the integer problem**
            let mut mip_params = glpk::glp_iocp::default();
            glpk::glp_init_iocp(&mut mip_params);
            mip_params.presolve = 1; 
            let mip_ret = glpk::glp_intopt(lp, &mut mip_params);

            if mip_ret != 0 {
                solution.status = Status::MIPFailed;
                continue;
            }

            let status = glpk::glp_mip_status(lp);
            match status {
                1 => solution.status = Status::Undefined,
                2 => {
                    solution.status = Status::Feasible;
                    solution.objective = glpk::glp_mip_obj_val(lp) as i32;
                    for (j, var) in polytope.variables.iter().enumerate() {
                        let x = glpk::glp_mip_col_val(lp, (j + 1) as i32);
                        solution.interpretation.insert(var.id.clone(), x as i64);
                    }
                },
                3 => {
                    solution.status = Status::Infeasible;
                }
                4 => {
                    solution.status = Status::NoFeasible;
                }
                5 => {
                    solution.status = Status::Optimal;
                    solution.objective = glpk::glp_mip_obj_val(lp) as i32;
                    for (j, var) in polytope.variables.iter().enumerate() {
                        let x = glpk::glp_mip_col_val(lp, (j + 1) as i32);
                        solution.interpretation.insert(var.id.clone(), x as i64);
                    }
                },
                6 => {
                    solution.status = Status::Unbounded;
                },
                x => {
                    panic!("Unknown status when solving ({})", x);
                }
            }
            solutions.push(solution);
        }

        // Clean up
        glpk::glp_delete_prob(lp);

        return solutions;
    }
}

#[derive(Deserialize)]
struct SolveRequest {
    polytope: SparseIntegerPolyhedron,
    objectives: Vec<Objective>,
    maximize: bool,
    term_out: bool,
}

/// POST /solve
async fn solve(req: web::Json<SolveRequest>) -> impl Responder {
    let solutions = solve_ilps(
        &req.polytope,
        req.objectives.clone(),
        req.maximize,
        req.term_out,
    );
    HttpResponse::Ok().json(solutions)
}

/// GET /health
async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server on http://127.0.0.1:8080");
    HttpServer::new(|| {
        App::new()
            .route("/solve", web::post().to(solve))
            .route("/health", web::get().to(health_check))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
