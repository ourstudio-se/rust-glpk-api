use std::collections::HashMap;
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveInputError};
use crate::models::{ApiSolution, SparseLEIntegerPolyhedron, SolverDirection, Status};
use crate::convert::to_glpk_polyhedron;

use grb::prelude::*;

/// Gurobi solver implementation
pub struct GurobiSolver;

impl GurobiSolver {
    pub fn new() -> Self {
        GurobiSolver
    }

    /// Convert Gurobi status to our API status
    fn convert_status(status: grb::Status) -> Status {
        match status {
            grb::Status::Optimal => Status::Optimal,
            grb::Status::Infeasible => Status::Infeasible,
            grb::Status::InfOrUnbd | grb::Status::Unbounded => Status::Unbounded,
            _ => Status::Undefined,
        }
    }
}

impl Solver for GurobiSolver {
    fn solve(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        objectives: &[HashMap<String, f64>],
        direction: SolverDirection,
        use_presolve: bool,
    ) -> std::result::Result<Vec<ApiSolution>, SolveInputError> {
        // Use GLPK polyhedron for validation
        let glpk_polyhedron = to_glpk_polyhedron(polyhedron);
        validate_objectives_owned(&glpk_polyhedron.variables, objectives)?;

        let sense = match direction {
            SolverDirection::Maximize => ModelSense::Maximize,
            SolverDirection::Minimize => ModelSense::Minimize,
        };

        let mut solutions = Vec::new();

        // Create Gurobi environment once
        let mut env = Env::new("").map_err(|e| SolveInputError {
            details: format!("Failed to create Gurobi environment: {}", e),
        })?;

        // Disable Gurobi console output for production use
        // Set to 1 to enable verbose logging for debugging
        env.set(param::OutputFlag, 0).map_err(|e| SolveInputError {
            details: format!("Failed to set Gurobi output flag: {}", e),
        })?;

        // Use all available threads for parallel solving
        // Gurobi will automatically use all CPU cores (default is 0 = automatic)
        // You can set to a specific number to limit threads, e.g., env.set(param::Threads, 4)
        env.set(param::Threads, 0).map_err(|e| SolveInputError {
            details: format!("Failed to set Gurobi thread count: {}", e),
        })?;

        // Configure presolve: -1 = auto, 0 = off, 1 = conservative, 2 = aggressive
        env.set(param::Presolve, if use_presolve { -1 } else { 0 }).map_err(|e| SolveInputError {
            details: format!("Failed to set Gurobi presolve: {}", e),
        })?;

        // Solve each objective separately
        for objective in objectives {
            // Create a new Gurobi model for each objective
            let mut model = Model::with_env("optimization", &env).map_err(|e| SolveInputError {
                details: format!("Failed to create Gurobi model: {}", e),
            })?;

            // Add variables
            let mut vars: Vec<Var> = Vec::new();
            for var in polyhedron.variables.iter() {
                let (lower, upper) = var.bound;

                // Use binary variables for [0,1] bounds, integer otherwise
                // Binary variables are optimized more efficiently by Gurobi
                let gurobi_var = if lower == 0 && upper == 1 {
                    add_binvar!(
                        model,
                        name: &var.id
                    ).map_err(|e| SolveInputError {
                        details: format!("Failed to add binary variable: {}", e),
                    })?
                } else {
                    add_intvar!(
                        model,
                        name: &var.id,
                        bounds: lower as f64..upper as f64
                    ).map_err(|e| SolveInputError {
                        details: format!("Failed to add integer variable: {}", e),
                    })?
                };

                vars.push(gurobi_var);
            }

            model.update().map_err(|e| SolveInputError {
                details: format!("Failed to update model after adding variables: {}", e),
            })?;

            // Build sparse matrix structure: for each row, collect its column entries
            let n_rows = polyhedron.a.shape.nrows;
            let n_cols = polyhedron.a.shape.ncols;
            let mut row_data: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n_rows];

            for i in 0..polyhedron.a.rows.len() {
                let row = polyhedron.a.rows[i] as usize;
                let col = polyhedron.a.cols[i] as usize;
                let val = polyhedron.a.vals[i] as f64;

                if row < n_rows && col < n_cols {
                    row_data[row].push((col, val));
                }
            }

            // Add constraints (Ax <= b)
            for (row_idx, entries) in row_data.iter().enumerate() {
                if entries.is_empty() {
                    continue;
                }

                let rhs = polyhedron.b.get(row_idx).copied().unwrap_or(0) as f64;

                // Build linear expression for this constraint
                let expr = entries.iter().fold(
                    Expr::Constant(0.0),
                    |acc, &(col_idx, coeff)| {
                        acc + coeff * vars[col_idx]
                    }
                );

                let constraint_name = format!("c{}", row_idx);
                model.add_constr(&constraint_name, c!(expr <= rhs)).map_err(|e| {
                    SolveInputError {
                        details: format!("Failed to add constraint: {}", e),
                    }
                })?;
            }

            model.update().map_err(|e| SolveInputError {
                details: format!("Failed to update model after adding constraints: {}", e),
            })?;

            // Build objective expression
            let obj_expr = polyhedron.variables.iter().enumerate().fold(
                Expr::Constant(0.0),
                |acc, (idx, var)| {
                    let coeff = objective.get(&var.id).copied().unwrap_or(0.0);
                    if coeff != 0.0 {
                        acc + coeff * vars[idx]
                    } else {
                        acc
                    }
                }
            );

            model.set_objective(obj_expr, sense).map_err(|e| SolveInputError {
                details: format!("Failed to set objective: {}", e),
            })?;

            // Optimize
            model.optimize().map_err(|e| SolveInputError {
                details: format!("Failed to optimize: {}", e),
            })?;

            // Extract solution
            let model_status = model.status().map_err(|e| SolveInputError {
                details: format!("Failed to get model status: {}", e),
            })?;
            let status = Self::convert_status(model_status);

            // Map solution back to variable names
            let mut solution_map: HashMap<String, i32> = HashMap::new();
            for (idx, var) in polyhedron.variables.iter().enumerate() {
                let (lower, upper) = var.bound;

                // Get solution value, or use fixed value if variable was eliminated by presolve
                let value = model.get_obj_attr(attr::X, &vars[idx])
                    .unwrap_or_else(|_| {
                        // If variable is fixed (lower == upper), use the fixed value
                        if lower == upper {
                            lower as f64
                        } else {
                            0.0
                        }
                    });

                solution_map.insert(var.id.clone(), value.round() as i32);
            }

            // Calculate objective value
            let objective_value: f64 = solution_map
                .iter()
                .filter_map(|(var_id, &val)| {
                    objective.get(var_id).map(|coeff| coeff * (val as f64))
                })
                .sum();

            solutions.push(ApiSolution {
                status,
                objective: objective_value.round() as i32,
                solution: solution_map,
                error: None,
            });
        }

        Ok(solutions)
    }

    fn name(&self) -> &str {
        "Gurobi"
    }
}
