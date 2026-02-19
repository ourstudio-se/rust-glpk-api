use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_void;
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveInputError};
use crate::models::{ApiSolution, SparseLEIntegerPolyhedron, SolverDirection, Status};
use crate::convert::to_glpk_polyhedron;

use highs_sys::*;

/// HiGHS solver implementation using highs-sys for direct memory control.
///
/// This implementation ensures proper memory cleanup by:
/// - Creating a single HiGHS instance per solve() call
/// - Using RAII (HighsGuard) to guarantee Highs_destroy() is called
/// - Reusing the model across multiple objectives (only updating objective coefficients)
/// - All HiGHS resources are freed when solve() returns
pub struct HighsSolver;

impl HighsSolver {
    pub fn new() -> Self {
        HighsSolver
    }

    /// Convert HiGHS status to our API status
    fn convert_status(status: i32) -> Status {
        const HIGHS_MODEL_STATUS_OPTIMAL: i32 = 7;
        const HIGHS_MODEL_STATUS_INFEASIBLE: i32 = 8;
        const HIGHS_MODEL_STATUS_UNBOUNDED: i32 = 10;
        const HIGHS_MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE: i32 = 9;

        match status {
            HIGHS_MODEL_STATUS_OPTIMAL => Status::Optimal,
            HIGHS_MODEL_STATUS_INFEASIBLE => Status::Infeasible,
            HIGHS_MODEL_STATUS_UNBOUNDED | HIGHS_MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE => Status::Unbounded,
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
        use_presolve: bool,
    ) -> Result<Vec<ApiSolution>, SolveInputError> {
        // Use GLPK polyhedron for validation
        let glpk_polyhedron = to_glpk_polyhedron(polyhedron);
        validate_objectives_owned(&glpk_polyhedron.variables, objectives)?;

        let n_rows = polyhedron.a.shape.nrows as i32;
        let n_cols = polyhedron.variables.len() as i32;

        // Create HiGHS instance
        let highs_ptr = unsafe { Highs_create() };
        if highs_ptr.is_null() {
            return Err(SolveInputError {
                details: "Failed to create HiGHS instance".to_string(),
            });
        }

        // Ensure cleanup on drop using RAII
        let _highs_guard = HighsGuard(highs_ptr);

        // Set options
        unsafe {
            let presolve_str = if use_presolve {
                CString::new("on").unwrap()
            } else {
                CString::new("off").unwrap()
            };
            let option_name = CString::new("presolve").unwrap();
            Highs_setStringOptionValue(highs_ptr, option_name.as_ptr(), presolve_str.as_ptr());

            // Disable output
            let output_flag = CString::new("output_flag").unwrap();
            Highs_setBoolOptionValue(highs_ptr, output_flag.as_ptr(), 0);
        }

        // Set optimization sense (minimize = 1, maximize = -1)
        let sense = match direction {
            SolverDirection::Minimize => 1,
            SolverDirection::Maximize => -1,
        };
        unsafe {
            Highs_changeObjectiveSense(highs_ptr, sense);
        }

        // Prepare row bounds (Ax <= b means -inf <= Ax <= b)
        let row_lower = vec![f64::NEG_INFINITY; n_rows as usize];
        let row_upper: Vec<f64> = polyhedron.b.iter().map(|&b| b as f64).collect();

        // Add rows FIRST (before columns reference them)
        unsafe {
            Highs_addRows(
                highs_ptr,
                n_rows,
                row_lower.as_ptr(),
                row_upper.as_ptr(),
                0, // num_nz = 0, will add via columns
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            );
        }

        // Build sparse constraint matrix in CSC (Column Sparse Compressed) format
        // For each column, we need to know which rows it appears in
        let mut col_start: Vec<i32> = Vec::with_capacity((n_cols + 1) as usize);
        let mut col_index: Vec<i32> = Vec::new();
        let mut col_value: Vec<f64> = Vec::new();

        // Build column-wise sparse matrix
        for col_idx in 0..n_cols as usize {
            col_start.push(col_index.len() as i32);

            // Find all entries for this column
            for i in 0..polyhedron.a.rows.len() {
                if polyhedron.a.cols[i] as usize == col_idx {
                    col_index.push(polyhedron.a.rows[i] as i32);
                    col_value.push(polyhedron.a.vals[i] as f64);
                }
            }
        }
        col_start.push(col_index.len() as i32); // Final element

        // Prepare column bounds and costs
        let col_costs = vec![0.0; n_cols as usize];
        let col_lower: Vec<f64> = polyhedron.variables.iter().map(|v| v.bound.0 as f64).collect();
        let col_upper: Vec<f64> = polyhedron.variables.iter().map(|v| v.bound.1 as f64).collect();

        // Add columns with constraints (rows must exist first)
        unsafe {
            Highs_addCols(
                highs_ptr,
                n_cols,
                col_costs.as_ptr(),
                col_lower.as_ptr(),
                col_upper.as_ptr(),
                col_index.len() as i32,
                col_start.as_ptr(),
                col_index.as_ptr(),
                col_value.as_ptr(),
            );
        }

        // Set integrality for all columns
        for col_idx in 0..n_cols {
            unsafe {
                Highs_changeColIntegrality(highs_ptr, col_idx, 1); // 1 = integer
            }
        }

        let mut solutions = Vec::with_capacity(objectives.len());

        // Solve each objective by updating objective coefficients
        for objective in objectives {
            // Update objective coefficients
            for (col_idx, var) in polyhedron.variables.iter().enumerate() {
                let obj_coeff = objective.get(&var.id).copied().unwrap_or(0.0);
                unsafe {
                    Highs_changeColCost(highs_ptr, col_idx as i32, obj_coeff);
                }
            }

            // Solve
            let status = unsafe { Highs_run(highs_ptr) };
            if status != 0 {
                solutions.push(ApiSolution {
                    status: Status::Undefined,
                    objective: 0,
                    solution: HashMap::new(),
                    error: Some(format!("HiGHS solve failed with status {}", status)),
                });
                continue;
            }

            // Get model status
            let model_status = unsafe { Highs_getModelStatus(highs_ptr) };
            let api_status = Self::convert_status(model_status);

            // Extract solution
            let mut solution_values = vec![0.0; n_cols as usize];
            unsafe {
                Highs_getSolution(highs_ptr, solution_values.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            }

            // Map solution back to variable names
            let mut solution_map: HashMap<String, i32> = HashMap::new();
            for (col_idx, var) in polyhedron.variables.iter().enumerate() {
                let value = solution_values[col_idx];
                let rounded_value = value.round() as i32;
                solution_map.insert(var.id.clone(), rounded_value);
            }

            // Calculate objective value
            let objective_value: f64 = solution_map.iter()
                .filter_map(|(var_id, &val)| {
                    objective.get(var_id).map(|coeff| coeff * (val as f64))
                })
                .sum();

            solutions.push(ApiSolution {
                status: api_status,
                objective: objective_value.round() as i32,
                solution: solution_map,
                error: None,
            });
        }

        // HiGHS instance will be destroyed by HighsGuard drop
        Ok(solutions)
    }

    fn name(&self) -> &str {
        "HiGHS"
    }
}

/// RAII guard to ensure HiGHS instance is properly destroyed
struct HighsGuard(*mut c_void);

impl Drop for HighsGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                Highs_destroy(self.0);
            }
        }
    }
}
