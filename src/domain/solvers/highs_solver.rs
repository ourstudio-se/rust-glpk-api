use std::collections::HashMap;
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveInputError};
use crate::models::{ApiSolution, SparseLEIntegerPolyhedron, SolverDirection, Status};
use crate::convert::to_glpk_polyhedron;

use ::highs::{ColProblem, Sense, HighsModelStatus};

/// HiGHS solver implementation
pub struct HighsSolver;

impl HighsSolver {
    pub fn new() -> Self {
        HighsSolver
    }

    /// Convert HiGHS status to our API status
    fn convert_status(model_status: HighsModelStatus) -> Status {
        match model_status {
            HighsModelStatus::Optimal => Status::Optimal,
            HighsModelStatus::Infeasible => Status::Infeasible,
            HighsModelStatus::UnboundedOrInfeasible => Status::Unbounded,
            HighsModelStatus::Unbounded => Status::Unbounded,
            _ => Status::Undefined,
        }
    }
}

impl Solver for HighsSolver {
    fn solve(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        objectives: &[HashMap<String, f64>],
        direction: SolverDirection,
    ) -> Result<Vec<ApiSolution>, SolveInputError> {
        // Use GLPK polyhedron for validation
        let glpk_polyhedron = to_glpk_polyhedron(polyhedron);
        validate_objectives_owned(&glpk_polyhedron.variables, objectives)?;

        let sense = match direction {
            SolverDirection::Maximize => Sense::Maximise,
            SolverDirection::Minimize => Sense::Minimise,
        };

        let mut solutions = Vec::new();

        // Solve each objective separately
        for objective in objectives {
            // Create a new HiGHS problem for each objective
            let mut problem = ColProblem::new();

            // First, add all constraint rows
            let n_rows = polyhedron.a.shape.nrows;
            let mut rows = Vec::new();
            for row_idx in 0..n_rows {
                let rhs = polyhedron.b.get(row_idx).copied().unwrap_or(0) as f64;
                let row = problem.add_row(..=rhs);
                rows.push(row);
            }

            // Build sparse matrix data: for each column, collect its row entries
            let n_cols = polyhedron.a.shape.ncols;
            let mut col_data: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n_cols];

            for i in 0..polyhedron.a.rows.len() {
                let row = polyhedron.a.rows[i] as usize;
                let col = polyhedron.a.cols[i] as usize;
                let val = polyhedron.a.vals[i] as f64;

                if col < n_cols && row < n_rows {
                    col_data[col].push((row, val));
                }
            }

            // Add variables (columns) with their constraints
            let mut var_indices: Vec<usize> = Vec::new();
            for (col_idx, var) in polyhedron.variables.iter().enumerate() {
                let obj_coeff = objective.get(&var.id).copied().unwrap_or(0.0);
                let (lower, upper) = var.bound;

                // Get constraint coefficients for this column
                let row_factors: Vec<_> = col_data.get(col_idx)
                    .map(|entries| {
                        entries.iter()
                            .map(|(row_idx, val)| (rows[*row_idx], *val))
                            .collect()
                    })
                    .unwrap_or_default();

                problem.add_integer_column(
                    obj_coeff,
                    lower as f64..=upper as f64,
                    &row_factors,
                );
                var_indices.push(col_idx);
            }

            // Solve the problem with presolve disabled
            let mut model = problem.optimise(sense);
            model.set_option("presolve", "off");
            let solved = model.solve();

            // Extract solution
            let model_status = solved.status();
            let status = Self::convert_status(model_status);

            let solution_values = solved.get_solution();

            // Map solution back to variable names
            let mut solution_map: HashMap<String, i32> = HashMap::new();
            for (col_idx, var) in polyhedron.variables.iter().enumerate() {
                let value = solution_values.columns().get(col_idx).copied().unwrap_or(0.0);
                solution_map.insert(var.id.clone(), value.round() as i32);
            }

            // Calculate objective value
            let objective_value: f64 = solution_map.iter()
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
        "HiGHS"
    }
}
