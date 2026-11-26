use crate::domain::outcome::SolveOutcome;
use crate::domain::solver::{SolveError, Solver};
use crate::models::{SolverDirection, SparseLEIntegerPolyhedron};
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_void;
use std::sync::Arc;

use highs_sys::*;
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;

/// Cached HiGHS model structure
struct HighsModel {
    highs_ptr: *mut c_void,
    n_cols: i32,
}

// `HighsModel` contains a raw pointer to a HiGHS instance, which is
// not intrinsically thread-safe. We declare `HighsModel` as `Send`
// because it is only ever used behind a `Mutex`, e.g. via
// `Arc<Mutex<HighsModel>>` in `HighsSolver::model_cache`. All accesses to
// `highs_ptr` must be performed while holding that mutex, ensuring that the
// underlying HiGHS instance is never used concurrently from multiple threads
// and is not accessed outside this synchronized context.
unsafe impl Send for HighsModel {}

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

    /// Build a new HiGHS model for the given polyhedron
    fn build_model(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        use_presolve: bool,
    ) -> Result<Arc<Mutex<HighsModel>>, SolveError> {
        let n_rows = polyhedron.a.shape.nrows as i32;
        let n_cols = polyhedron.variables.len() as i32;

        // Create HiGHS instance
        let highs_ptr = unsafe { Highs_create() };
        if highs_ptr.is_null() {
            return Err(SolveError::input("Failed to create HiGHS instance"));
        }
        // Owned from here on: if any step below fails, `Drop` frees the instance.
        let model = HighsModel { highs_ptr, n_cols };

        // Set options
        let presolve_str = if use_presolve {
            CString::new("on").unwrap()
        } else {
            CString::new("off").unwrap()
        };
        let option_name = CString::new("presolve").unwrap();
        check_build_step(
            unsafe {
                Highs_setStringOptionValue(highs_ptr, option_name.as_ptr(), presolve_str.as_ptr())
            },
            "Highs_setStringOptionValue(presolve)",
        )?;

        // Disable output
        let output_flag = CString::new("output_flag").unwrap();
        check_build_step(
            unsafe { Highs_setBoolOptionValue(highs_ptr, output_flag.as_ptr(), 0) },
            "Highs_setBoolOptionValue(output_flag)",
        )?;

        // Prepare row bounds (Ax <= b means -inf <= Ax <= b)
        let row_lower = vec![f64::NEG_INFINITY; n_rows as usize];
        let row_upper: Vec<f64> = polyhedron.b.iter().map(|&b| b as f64).collect();

        // Add rows FIRST (before columns reference them)
        check_build_step(
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
                )
            },
            "Highs_addRows",
        )?;

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
                    col_index.push(polyhedron.a.rows[i]);
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
        check_build_step(
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
                )
            },
            "Highs_addCols",
        )?;

        // Set integrality for all columns
        for col_idx in 0..n_cols {
            check_build_step(
                unsafe { Highs_changeColIntegrality(highs_ptr, col_idx, 1) }, // 1 = integer
                "Highs_changeColIntegrality",
            )?;
        }

        Ok(Arc::new(Mutex::new(model)))
    }

    /// Get or build a model for the given polyhedron
    fn obtain_model(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        use_presolve: bool,
    ) -> Result<Arc<Mutex<HighsModel>>, SolveError> {
        match &self.model_cache {
            Some(some_model_cache) => {
                // Check cache first
                {
                    let mut cache = some_model_cache.lock();
                    if let Some(cached_model) = cache.get(polyhedron) {
                        return Ok(Arc::clone(cached_model));
                    }
                }

                // Not in cache, build new model.
                // A failed build returns here and is never stored in the cache.
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
                self.build_model(polyhedron, use_presolve)
            }
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
    ) -> Result<Vec<SolveOutcome>, SolveError> {
        // Get or build cached model, then lock mutex for entire solve call
        let model_mutex = self.obtain_model(polyhedron, use_presolve)?;
        let model = model_mutex.lock();

        let sense = match direction {
            SolverDirection::Minimize => OBJECTIVE_SENSE_MINIMIZE,
            SolverDirection::Maximize => OBJECTIVE_SENSE_MAXIMIZE,
        };
        check_build_step(
            unsafe { Highs_changeObjectiveSense(model.highs_ptr, sense) },
            "Highs_changeObjectiveSense",
        )?;

        Ok(objectives
            .iter()
            .map(|objective| solve_objective(&model, polyhedron, objective))
            .collect())
    }

    fn name(&self) -> &str {
        "HiGHS"
    }
}

/// Solve a single objective: set objective → run → classify → read solution.
fn solve_objective(
    model: &HighsModel,
    polyhedron: &SparseLEIntegerPolyhedron,
    objective: &HashMap<String, f64>,
) -> SolveOutcome {
    if let Err(raw) = set_objective(model.highs_ptr, polyhedron, objective) {
        return SolveOutcome::Failed { raw };
    }

    let run_status = unsafe { Highs_run(model.highs_ptr) };
    let model_status = unsafe { Highs_getModelStatus(model.highs_ptr) };
    let has_feasible_solution = has_feasible_solution(model.highs_ptr);

    match classify_run(run_status, model_status, has_feasible_solution) {
        RunResult::Optimal => match read_solution(model, polyhedron, objective) {
            Ok((values, objective)) => SolveOutcome::Optimal { values, objective },
            Err(raw) => SolveOutcome::Failed { raw },
        },
        RunResult::Feasible => match read_solution(model, polyhedron, objective) {
            Ok((values, objective)) => SolveOutcome::Feasible { values, objective },
            Err(raw) => SolveOutcome::Failed { raw },
        },
        RunResult::NoSolution(outcome) => outcome,
    }
}

/// How a finished `Highs_run` should be handled.
#[derive(Debug, PartialEq)]
enum RunResult {
    /// Proven optimal: read the solution.
    Optimal,
    /// Stopped early but with a feasible solution: read the solution.
    Feasible,
    /// No usable solution: return this outcome as is.
    NoSolution(SolveOutcome),
}

/// Decide what a finished HiGHS run means. Pure function, no solver calls.
///
/// A warning from `Highs_run` is not treated as a failure on its own;
/// the model status decides.
fn classify_run(
    run_status: HighsInt,
    model_status: HighsInt,
    has_feasible_solution: bool,
) -> RunResult {
    if run_status == STATUS_ERROR {
        return RunResult::NoSolution(SolveOutcome::Failed {
            raw: format!("HiGHS: Highs_run returned {} kError", run_status),
        });
    }

    let raw = format!(
        "HiGHS: model status {} {}",
        model_status,
        model_status_name(model_status)
    );

    match model_status {
        MODEL_STATUS_OPTIMAL => RunResult::Optimal,
        MODEL_STATUS_INFEASIBLE => RunResult::NoSolution(SolveOutcome::Infeasible { raw }),
        MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE | MODEL_STATUS_UNBOUNDED => {
            RunResult::NoSolution(SolveOutcome::Unbounded { raw })
        }
        // Error statuses: whatever solution is left over cannot be trusted.
        MODEL_STATUS_NOTSET
        | MODEL_STATUS_LOAD_ERROR
        | MODEL_STATUS_MODEL_ERROR
        | MODEL_STATUS_PRESOLVE_ERROR
        | MODEL_STATUS_SOLVE_ERROR
        | MODEL_STATUS_POSTSOLVE_ERROR
        | MODEL_STATUS_MODEL_EMPTY => RunResult::NoSolution(SolveOutcome::Failed { raw }),
        // Stopped before proving optimality (time limit, objective bound,
        // memory limit, kUnknown, ...) but with a feasible solution.
        _ if has_feasible_solution => RunResult::Feasible,
        _ => RunResult::NoSolution(SolveOutcome::Failed { raw }),
    }
}

/// Name of a HiGHS model status, as used in the HiGHS documentation.
fn model_status_name(model_status: HighsInt) -> &'static str {
    match model_status {
        MODEL_STATUS_NOTSET => "kNotset",
        MODEL_STATUS_LOAD_ERROR => "kLoadError",
        MODEL_STATUS_MODEL_ERROR => "kModelError",
        MODEL_STATUS_PRESOLVE_ERROR => "kPresolveError",
        MODEL_STATUS_SOLVE_ERROR => "kSolveError",
        MODEL_STATUS_POSTSOLVE_ERROR => "kPostsolveError",
        MODEL_STATUS_MODEL_EMPTY => "kModelEmpty",
        MODEL_STATUS_OPTIMAL => "kOptimal",
        MODEL_STATUS_INFEASIBLE => "kInfeasible",
        MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE => "kUnboundedOrInfeasible",
        MODEL_STATUS_UNBOUNDED => "kUnbounded",
        MODEL_STATUS_OBJECTIVE_BOUND => "kObjectiveBound",
        MODEL_STATUS_OBJECTIVE_TARGET => "kObjectiveTarget",
        MODEL_STATUS_REACHED_TIME_LIMIT => "kTimeLimit",
        MODEL_STATUS_REACHED_ITERATION_LIMIT => "kIterationLimit",
        MODEL_STATUS_UNKNOWN => "kUnknown",
        MODEL_STATUS_REACHED_SOLUTION_LIMIT => "kSolutionLimit",
        MODEL_STATUS_REACHED_INTERRUPT => "kInterrupt",
        MODEL_STATUS_REACHED_MEMORY_LIMIT => "kMemoryLimit",
        _ => "unknown",
    }
}

/// Fail the whole request if a HiGHS call returned an error.
fn check_build_step(status: HighsInt, call: &str) -> Result<(), SolveError> {
    if status == STATUS_ERROR {
        return Err(SolveError::input(format!(
            "HiGHS: {} returned {} kError",
            call, status
        )));
    }
    Ok(())
}

/// Set the objective coefficients for all columns.
fn set_objective(
    highs_ptr: *mut c_void,
    polyhedron: &SparseLEIntegerPolyhedron,
    objective: &HashMap<String, f64>,
) -> Result<(), String> {
    for (col_idx, var) in polyhedron.variables.iter().enumerate() {
        let obj_coeff = objective.get(&var.id).copied().unwrap_or(0.0);
        let status = unsafe { Highs_changeColCost(highs_ptr, col_idx as i32, obj_coeff) };
        if status == STATUS_ERROR {
            return Err(format!(
                "HiGHS: Highs_changeColCost returned {} kError",
                status
            ));
        }
    }
    Ok(())
}

/// Whether HiGHS holds a feasible primal solution after the last run.
fn has_feasible_solution(highs_ptr: *mut c_void) -> bool {
    let info_name = CString::new("primal_solution_status").unwrap();
    let mut solution_status: HighsInt = SOLUTION_STATUS_NONE;
    let status =
        unsafe { Highs_getIntInfoValue(highs_ptr, info_name.as_ptr(), &mut solution_status) };
    status != STATUS_ERROR && solution_status == SOLUTION_STATUS_FEASIBLE
}

/// Read variable values and objective value of the current solution.
fn read_solution(
    model: &HighsModel,
    polyhedron: &SparseLEIntegerPolyhedron,
    objective: &HashMap<String, f64>,
) -> Result<(HashMap<String, i32>, i32), String> {
    let mut solution_values = vec![0.0; model.n_cols as usize];
    let status = unsafe {
        Highs_getSolution(
            model.highs_ptr,
            solution_values.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if status == STATUS_ERROR {
        return Err(format!(
            "HiGHS: Highs_getSolution returned {} kError",
            status
        ));
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
        .filter_map(|(var_id, &val)| objective.get(var_id).map(|coeff| coeff * (val as f64)))
        .sum();

    Ok((solution_map, objective_value.round() as i32))
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
            &polyhedron,
            &[obj1.clone()],
            SolverDirection::Maximize,
            true,
        );
        assert!(result1.is_ok());

        // Second solve with same polyhedron, different objective - should reuse cached model
        let result2 = solver.solve(&polyhedron, &[obj2], SolverDirection::Maximize, true);
        assert!(result2.is_ok());

        // Third solve with same polyhedron and objective - should still work
        let result3 = solver.solve(&polyhedron, &[obj1], SolverDirection::Maximize, true);
        assert!(result3.is_ok());
    }

    #[test]
    fn test_cache_disabled() {
        let solver = HighsSolver::without_cache();
        let polyhedron = create_test_polyhedron();

        let mut obj = HashMap::new();
        obj.insert("x".to_string(), 1.0);
        obj.insert("y".to_string(), 2.0);

        let result = solver.solve(&polyhedron, &[obj], SolverDirection::Maximize, true);
        assert!(result.is_ok());
    }

    // ---------- classify_run: pure mapping from HiGHS status to outcome ----------

    #[test]
    fn optimal_status_reads_solution() {
        assert_eq!(
            classify_run(STATUS_OK, MODEL_STATUS_OPTIMAL, true),
            RunResult::Optimal
        );
    }

    #[test]
    fn warning_with_optimal_status_still_reads_solution() {
        assert_eq!(
            classify_run(STATUS_WARNING, MODEL_STATUS_OPTIMAL, true),
            RunResult::Optimal
        );
    }

    #[test]
    fn run_error_is_failed() {
        assert_eq!(
            classify_run(STATUS_ERROR, MODEL_STATUS_OPTIMAL, true),
            RunResult::NoSolution(SolveOutcome::Failed {
                raw: "HiGHS: Highs_run returned -1 kError".to_string()
            })
        );
    }

    #[test]
    fn infeasible_status_is_infeasible() {
        assert_eq!(
            classify_run(STATUS_OK, MODEL_STATUS_INFEASIBLE, false),
            RunResult::NoSolution(SolveOutcome::Infeasible {
                raw: "HiGHS: model status 8 kInfeasible".to_string()
            })
        );
    }

    #[test]
    fn unbounded_or_infeasible_status_is_unbounded() {
        assert_eq!(
            classify_run(STATUS_OK, MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE, false),
            RunResult::NoSolution(SolveOutcome::Unbounded {
                raw: "HiGHS: model status 9 kUnboundedOrInfeasible".to_string()
            })
        );
    }

    #[test]
    fn unbounded_status_is_unbounded() {
        assert!(matches!(
            classify_run(STATUS_OK, MODEL_STATUS_UNBOUNDED, false),
            RunResult::NoSolution(SolveOutcome::Unbounded { .. })
        ));
    }

    #[test]
    fn time_limit_with_feasible_solution_is_feasible() {
        assert_eq!(
            classify_run(STATUS_WARNING, MODEL_STATUS_REACHED_TIME_LIMIT, true),
            RunResult::Feasible
        );
    }

    #[test]
    fn time_limit_without_solution_is_failed() {
        assert_eq!(
            classify_run(STATUS_WARNING, MODEL_STATUS_REACHED_TIME_LIMIT, false),
            RunResult::NoSolution(SolveOutcome::Failed {
                raw: "HiGHS: model status 13 kTimeLimit".to_string()
            })
        );
    }

    #[test]
    fn stopped_early_with_feasible_solution_is_feasible() {
        for model_status in [
            MODEL_STATUS_OBJECTIVE_BOUND,
            MODEL_STATUS_OBJECTIVE_TARGET,
            MODEL_STATUS_REACHED_MEMORY_LIMIT,
            MODEL_STATUS_UNKNOWN,
        ] {
            assert_eq!(
                classify_run(STATUS_WARNING, model_status, true),
                RunResult::Feasible,
                "{}",
                model_status_name(model_status)
            );
        }
    }

    #[test]
    fn error_status_is_failed_even_with_feasible_solution() {
        for model_status in [
            MODEL_STATUS_SOLVE_ERROR,
            MODEL_STATUS_POSTSOLVE_ERROR,
            MODEL_STATUS_MODEL_EMPTY,
        ] {
            assert!(
                matches!(
                    classify_run(STATUS_OK, model_status, true),
                    RunResult::NoSolution(SolveOutcome::Failed { .. })
                ),
                "{}",
                model_status_name(model_status)
            );
        }
    }

    #[test]
    fn other_status_is_failed() {
        assert_eq!(
            classify_run(STATUS_OK, MODEL_STATUS_MODEL_EMPTY, false),
            RunResult::NoSolution(SolveOutcome::Failed {
                raw: "HiGHS: model status 6 kModelEmpty".to_string()
            })
        );
    }
}
