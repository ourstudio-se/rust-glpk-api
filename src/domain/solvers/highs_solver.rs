use crate::convert::to_glpk_polyhedron;
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveInputError};
use crate::models::{ApiSolution, SolverDirection, SparseLEIntegerPolyhedron, Status};
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_void;
use std::sync::Arc;

use highs_sys::*;
use lru::LruCache;
use parking_lot::Mutex;
// use std::sync::Mutex;
use std::num::NonZeroUsize;

/// Cached HiGHS model structure
struct HighsModel {
    highs_ptr: *mut c_void,
    n_cols: i32,
}

// SAFETY: HiGHS model pointer is only accessed while holding a Mutex.
// We wrap `HighsModel` in `Arc<Mutex<...>>` to ensure exclusive access
unsafe impl Send for HighsModel {}
unsafe impl Sync for HighsModel {}

impl Drop for HighsModel {
    fn drop(&mut self) {
        if !self.highs_ptr.is_null() {
            unsafe {
                Highs_destroy(self.highs_ptr);
            }
        }
    }
}

/// HiGHS solver implementation using highs-sys for direct memory control.
///
/// This implementation includes model caching:
/// - Models are cached based on polyhedron hash
/// - LRU eviction policy when cache is full
/// - Reuses cached models across multiple objectives
/// - Thread-safe via parking_lot::Mutex
pub struct HighsSolver {
    model_cache: Option<Arc<Mutex<LruCache<SparseLEIntegerPolyhedron, Arc<Mutex<HighsModel>>>>>>,
}

impl HighsSolver {
    /// Create a new HiGHS solver with specified cache size
    pub fn with_cache_size(size: Option<usize>) -> Self {
        match size {
            Some(0) | None => Self::without_cache(),
            Some(s) => HighsSolver {
                model_cache: Some(Arc::new(Mutex::new(LruCache::new(
                    NonZeroUsize::new(s).unwrap(),
                )))),
            },
        }
    }

    /// Create solver with caching disabled
    pub fn without_cache() -> Self {
        HighsSolver { model_cache: None }
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
            HIGHS_MODEL_STATUS_UNBOUNDED | HIGHS_MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE => {
                Status::Unbounded
            }
            _ => Status::Undefined,
        }
    }

    /// Build a new HiGHS model for the given polyhedron
    fn build_model(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        use_presolve: bool,
    ) -> Result<Arc<Mutex<HighsModel>>, SolveInputError> {
        let n_rows = polyhedron.a.shape.nrows as i32;
        let n_cols = polyhedron.variables.len() as i32;

        // Create HiGHS instance
        let highs_ptr = unsafe { Highs_create() };
        if highs_ptr.is_null() {
            return Err(SolveInputError {
                details: "Failed to create HiGHS instance".to_string(),
            });
        }

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

        // Prepare column bounds and costs (zero costs, will be updated per objective)
        let col_costs = vec![0.0; n_cols as usize];
        let col_lower: Vec<f64> = polyhedron
            .variables
            .iter()
            .map(|v| v.bound.0 as f64)
            .collect();
        let col_upper: Vec<f64> = polyhedron
            .variables
            .iter()
            .map(|v| v.bound.1 as f64)
            .collect();

        // Add columns with constraints
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

        Ok(Arc::new(Mutex::new(HighsModel { highs_ptr, n_cols })))
    }

    /// Get or build a model for the given polyhedron
    fn get_or_build_model(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        use_presolve: bool,
    ) -> Result<Arc<Mutex<HighsModel>>, SolveInputError> {
        match &self.model_cache {
            Some(some_model_cache) => {
                // Check cache first
                {
                    let mut cache = some_model_cache.lock();
                    if let Some(cached_model) = cache.get(polyhedron) {
                        return Ok(Arc::clone(cached_model));
                    }
                }

                // Not in cache, build new model
                let model = self.build_model(polyhedron, use_presolve)?;

                // Store in cache
                {
                    let mut cache = some_model_cache.lock();
                    cache.put(polyhedron.clone(), Arc::clone(&model));
                }

                Ok(model)
            } // Caching enabled, proceed to check cache
            None => {
                // Caching disabled, build new model every time
                return self.build_model(polyhedron, use_presolve);
            }
        }
    }
}

impl Solver for HighsSolver {
    fn solve(
        &self,
        polyhedron: SparseLEIntegerPolyhedron,
        objectives: Vec<HashMap<String, f64>>,
        direction: SolverDirection,
        use_presolve: bool,
    ) -> Result<Vec<ApiSolution>, SolveInputError> {
        // Use GLPK polyhedron for validation
        let glpk_polyhedron = to_glpk_polyhedron(&polyhedron);
        validate_objectives_owned(&glpk_polyhedron.variables, &objectives)?;

        // Get or build cached model, then lock mutex for entire solve call
        let model_mutex = self.get_or_build_model(&polyhedron, use_presolve)?;
        let model = model_mutex.lock();

        let highs_ptr = model.highs_ptr;
        let n_cols = model.n_cols;

        // Set optimization sense (minimize = 1, maximize = -1)
        let sense = match direction {
            SolverDirection::Minimize => 1,
            SolverDirection::Maximize => -1,
        };
        unsafe {
            Highs_changeObjectiveSense(highs_ptr, sense);
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
                Highs_getSolution(
                    highs_ptr,
                    solution_values.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }

            // Map solution back to variable names
            let mut solution_map: HashMap<String, i32> = HashMap::new();
            for (col_idx, var) in polyhedron.variables.iter().enumerate() {
                let value: f64 = solution_values[col_idx];
                let rounded_value = value.round() as i32;
                solution_map.insert(var.id.clone(), rounded_value);
            }

            // Calculate objective value
            let objective_value: f64 = solution_map
                .iter()
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

        Ok(solutions)
    }

    fn name(&self) -> &str {
        "HiGHS"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ApiIntegerSparseMatrix, ApiShape, ApiVariable, SolverDirection};
    use std::collections::HashMap;

    fn create_test_polyhedron() -> SparseLEIntegerPolyhedron {
        SparseLEIntegerPolyhedron {
            a: ApiIntegerSparseMatrix {
                rows: vec![0, 0, 1],
                cols: vec![0, 1, 1],
                vals: vec![1, 2, 1],
                shape: ApiShape { nrows: 2, ncols: 2 },
            },
            b: vec![10, 5],
            variables: vec![
                ApiVariable {
                    id: "x".to_string(),
                    bound: (0, 10),
                },
                ApiVariable {
                    id: "y".to_string(),
                    bound: (0, 10),
                },
            ],
        }
    }

    #[test]
    fn test_cache_reuses_model() {
        let solver = HighsSolver::with_cache_size(Some(10));
        let polyhedron = create_test_polyhedron();

        let mut obj1 = HashMap::new();
        obj1.insert("x".to_string(), 1.0);
        obj1.insert("y".to_string(), 2.0);

        let mut obj2 = HashMap::new();
        obj2.insert("x".to_string(), 2.0);
        obj2.insert("y".to_string(), 1.0);

        // First solve - should build model
        let result1 = solver.solve(
            polyhedron.clone(),
            vec![obj1.clone()],
            SolverDirection::Maximize,
            true,
        );
        assert!(result1.is_ok());

        // Second solve with same polyhedron, different objective - should reuse cached model
        let result2 = solver.solve(
            polyhedron.clone(),
            vec![obj2],
            SolverDirection::Maximize,
            true,
        );
        assert!(result2.is_ok());

        // Third solve with same polyhedron and objective - should still work
        let result3 = solver.solve(
            polyhedron.clone(),
            vec![obj1],
            SolverDirection::Maximize,
            true,
        );
        assert!(result3.is_ok());
    }

    #[test]
    fn test_cache_disabled() {
        let solver = HighsSolver::without_cache();
        let polyhedron = create_test_polyhedron();

        let mut obj = HashMap::new();
        obj.insert("x".to_string(), 1.0);
        obj.insert("y".to_string(), 2.0);

        let result = solver.solve(polyhedron, vec![obj], SolverDirection::Maximize, true);
        assert!(result.is_ok());
    }
}
