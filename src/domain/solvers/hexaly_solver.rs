use std::collections::HashMap;
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveInputError};
use crate::models::{ApiSolution, SparseLEIntegerPolyhedron, SolverDirection, Status};
use crate::convert::to_glpk_polyhedron;

use hexaly::{HexalyOptimizer, State, Expression};

/// Hexaly (formerly LocalSolver) solver implementation
pub struct HexalySolver;

impl HexalySolver {
    pub fn new() -> Self {
        HexalySolver
    }

    /// Convert Hexaly state to our API status
    /// Note: Hexaly 14.5 uses solution status, not state, for determining optimality
    fn convert_status(state: State) -> Status {
        // Hexaly 14.5 only returns Stopped after solving
        // The actual solution quality is in the solution status
        match state {
            State::Stopped => Status::Optimal, // Assume optimal when stopped
            State::Running => Status::Undefined,
            State::Paused => Status::Undefined,
        }
    }
}

impl Solver for HexalySolver {
    fn solve(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        objectives: &[HashMap<String, f64>],
        direction: SolverDirection,
        _use_presolve: bool,
    ) -> Result<Vec<ApiSolution>, SolveInputError> {
        // Use GLPK polyhedron for validation
        let glpk_polyhedron = to_glpk_polyhedron(polyhedron);
        validate_objectives_owned(&glpk_polyhedron.variables, objectives)?;

        let mut solutions = Vec::new();

        // Solve each objective separately
        for objective in objectives {
            // Create HexalyOptimizer environment
            let ls = HexalyOptimizer::new().map_err(|e| SolveInputError {
                details: format!("Failed to create Hexaly environment: {}", e),
            })?;

            // Configure parameters BEFORE building model
            let param = ls.param();
            param.set_verbosity(0); // Disable console output for performance

            // Get model
            let model = ls.model();

            // Create decision variables and store them
            let mut vars: Vec<Expression> = Vec::new();
            for var in polyhedron.variables.iter() {
                let (lower, upper) = var.bound;
                let expr = model.int_var(lower as i64, upper as i64);
                vars.push(expr);
            }

            // Build sparse matrix structure: for each row, collect its column entries
            let n_rows = polyhedron.a.shape.nrows;
            let n_cols = polyhedron.a.shape.ncols;
            let mut row_data: Vec<Vec<(usize, i32)>> = vec![Vec::new(); n_rows];

            for i in 0..polyhedron.a.rows.len() {
                let row = polyhedron.a.rows[i] as usize;
                let col = polyhedron.a.cols[i] as usize;
                let val = polyhedron.a.vals[i];

                if row < n_rows && col < n_cols {
                    row_data[row].push((col, val));
                }
            }

            // Add constraints (Ax <= b)
            for (row_idx, entries) in row_data.iter().enumerate() {
                if entries.is_empty() {
                    continue;
                }

                let rhs = polyhedron.b.get(row_idx).copied().unwrap_or(0);

                // Build linear expression for this constraint using sum
                let constraint_sum = model.sum();

                for &(col_idx, coeff) in entries {
                    if coeff == 1 {
                        // Just add the variable
                        model.add_operand(&constraint_sum, &vars[col_idx]);
                    } else if coeff == -1 {
                        // For -1, we need to create a product with -1
                        let neg_one = model.scalar(-1);
                        let neg_var = model.prod();
                        model.add_operand(&neg_var, &neg_one);
                        model.add_operand(&neg_var, &vars[col_idx]);
                        model.add_operand(&constraint_sum, &neg_var);
                    } else if coeff != 0 {
                        // For other coefficients, create coeff * var
                        let coeff_expr = model.scalar(coeff as i64);
                        let prod_expr = model.prod();
                        model.add_operand(&prod_expr, &coeff_expr);
                        model.add_operand(&prod_expr, &vars[col_idx]);
                        model.add_operand(&constraint_sum, &prod_expr);
                    }
                }

                // Add constraint: sum <= rhs
                let rhs_expr = model.scalar(rhs as i64);
                let constraint = model.leq(&constraint_sum, &rhs_expr);
                model.add_constraint(constraint);
            }

            // Build objective expression
            let obj_sum = model.sum();

            for (idx, var) in polyhedron.variables.iter().enumerate() {
                let coeff = objective.get(&var.id).copied().unwrap_or(0.0);
                if coeff != 0.0 {
                    let coeff_i64 = coeff.round() as i64;
                    if coeff_i64 == 1 {
                        model.add_operand(&obj_sum, &vars[idx]);
                    } else if coeff_i64 == -1 {
                        let neg_one = model.scalar(-1);
                        let neg_var = model.prod();
                        model.add_operand(&neg_var, &neg_one);
                        model.add_operand(&neg_var, &vars[idx]);
                        model.add_operand(&obj_sum, &neg_var);
                    } else if coeff_i64 != 0 {
                        let coeff_expr = model.scalar(coeff_i64);
                        let prod_expr = model.prod();
                        model.add_operand(&prod_expr, &coeff_expr);
                        model.add_operand(&prod_expr, &vars[idx]);
                        model.add_operand(&obj_sum, &prod_expr);
                    }
                }
            }

            // Set objective direction
            match direction {
                SolverDirection::Maximize => model.maximize(obj_sum),
                SolverDirection::Minimize => model.minimize(obj_sum),
            }

            // Close model (must be done after all constraints/objectives are added)
            model.close();

            // Solve (parameters were already set at the beginning)
            ls.solve();

            // Extract solution
            let state = ls.state();
            let status = Self::convert_status(state);

            // Map solution back to variable names
            let mut solution_map: HashMap<String, i32> = HashMap::new();
            for (idx, var) in polyhedron.variables.iter().enumerate() {
                let value = ls.get_int_value(&vars[idx]) as i32;
                solution_map.insert(var.id.clone(), value);
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
        "Hexaly"
    }
}
