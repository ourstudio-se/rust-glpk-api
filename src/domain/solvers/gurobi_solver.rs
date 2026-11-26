use crate::domain::outcome::SolveOutcome;
use crate::domain::solver::{SolveError, Solver};
use crate::models::{SolverDirection, SparseLEIntegerPolyhedron};
use std::collections::HashMap;
use std::sync::Arc;

use grb::prelude::*;
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;

/// Cached Gurobi model structure
struct GurobiModel {
    model: Model,
    vars: Vec<Var>,
}

// SAFETY: Gurobi model is properly synchronized through Arc and Mutex
// Each model instance is only accessed by one thread at a time
unsafe impl Send for GurobiModel {}
unsafe impl Sync for GurobiModel {}

/// Gurobi solver implementation with model caching
///
/// This implementation includes model caching:
/// - Models are cached based on polyhedron hash
/// - LRU eviction policy when cache is full
/// - Reuses cached models across multiple objectives
/// - Thread-safe via parking_lot::Mutex
pub struct GurobiSolver {
    model_cache: Option<Arc<Mutex<LruCache<SparseLEIntegerPolyhedron, Arc<Mutex<GurobiModel>>>>>>,
}

impl GurobiSolver {
    /// Create a new Gurobi solver with specified cache size
    pub fn with_cache_size(size: Option<usize>) -> Self {
        match size {
            Some(0) | None => Self::without_cache(),
            Some(s) => GurobiSolver {
                model_cache: Some(Arc::new(Mutex::new(LruCache::new(
                    NonZeroUsize::new(s).unwrap(),
                )))),
            },
        }
    }

    /// Create solver with caching disabled
    pub fn without_cache() -> Self {
        GurobiSolver { model_cache: None }
    }

    /// Build a new Gurobi model for the given polyhedron
    fn build_model(
        polyhedron: &SparseLEIntegerPolyhedron,
        use_presolve: bool,
    ) -> Result<Arc<Mutex<GurobiModel>>, SolveError> {
        // Create Gurobi environment
        let mut env = Env::new("").map_err(|e| {
            SolveError::input(format!("Failed to create Gurobi environment: {}", e))
        })?;

        // Disable Gurobi console output
        env.set(param::OutputFlag, 0)
            .map_err(|e| SolveError::input(format!("Failed to set Gurobi output flag: {}", e)))?;

        // Use all available threads
        env.set(param::Threads, 0)
            .map_err(|e| SolveError::input(format!("Failed to set Gurobi thread count: {}", e)))?;

        // Configure presolve: -1 = auto, 0 = off, 1 = conservative, 2 = aggressive
        env.set(param::Presolve, if use_presolve { -1 } else { 0 })
            .map_err(|e| SolveError::input(format!("Failed to set Gurobi presolve: {}", e)))?;

        // Create a Gurobi model
        let mut model = Model::with_env("optimization", &env)
            .map_err(|e| SolveError::input(format!("Failed to create Gurobi model: {}", e)))?;

        // Add variables
        let mut vars: Vec<Var> = Vec::new();
        for var in polyhedron.variables.iter() {
            let (lower, upper) = var.bound;

            // Use binary variables for [0,1] bounds
            let gurobi_var = if lower == 0 && upper == 1 {
                add_binvar!(
                    model,
                    name: &var.id
                )
                .map_err(|e| SolveError::input(format!("Failed to add binary variable: {}", e)))?
            } else {
                add_intvar!(
                    model,
                    name: &var.id,
                    bounds: lower as f64..upper as f64
                )
                .map_err(|e| SolveError::input(format!("Failed to add integer variable: {}", e)))?
            };

            vars.push(gurobi_var);
        }

        model.update().map_err(|e| {
            SolveError::input(format!(
                "Failed to update model after adding variables: {}",
                e
            ))
        })?;

        // Build sparse matrix structure
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

            // Build linear expression
            let expr = entries
                .iter()
                .fold(Expr::Constant(0.0), |acc, &(col_idx, coeff)| {
                    acc + coeff * vars[col_idx]
                });

            let constraint_name = format!("c{}", row_idx);
            model
                .add_constr(&constraint_name, c!(expr <= rhs))
                .map_err(|e| SolveError::input(format!("Failed to add constraint: {}", e)))?;
        }

        model.update().map_err(|e| {
            SolveError::input(format!(
                "Failed to update model after adding constraints: {}",
                e
            ))
        })?;

        Ok(Arc::new(Mutex::new(GurobiModel { model, vars })))
    }

    /// Get or build a model for the given polyhedron
    fn obtain_model(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        use_presolve: bool,
    ) -> Result<Arc<Mutex<GurobiModel>>, SolveError> {
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
                let model = Self::build_model(polyhedron, use_presolve)?;

                // Store in cache
                {
                    let mut cache = some_model_cache.lock();
                    cache.put(polyhedron.clone(), Arc::clone(&model));
                }

                Ok(model)
            }
            None => {
                // Cache disabled, always build new model
                Self::build_model(polyhedron, use_presolve)
            }
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
    ) -> std::result::Result<Vec<SolveOutcome>, SolveError> {
        // Get or build cached model
        let cached_model = self.obtain_model(polyhedron, use_presolve)?;
        let mut model_lock = cached_model.lock();

        let sense = match direction {
            SolverDirection::Maximize => ModelSense::Maximize,
            SolverDirection::Minimize => ModelSense::Minimize,
        };

        Ok(objectives
            .iter()
            .map(|objective| solve_objective(&mut model_lock, polyhedron, objective, sense))
            .collect())
    }

    fn name(&self) -> &str {
        "Gurobi"
    }
}

/// Solve a single objective: set objective → optimize → classify → read solution.
fn solve_objective(
    gurobi: &mut GurobiModel,
    polyhedron: &SparseLEIntegerPolyhedron,
    objective: &HashMap<String, f64>,
    sense: ModelSense,
) -> SolveOutcome {
    // Build objective expression
    let obj_expr =
        polyhedron
            .variables
            .iter()
            .enumerate()
            .fold(Expr::Constant(0.0), |acc, (idx, var)| {
                let coeff = objective.get(&var.id).copied().unwrap_or(0.0);
                if coeff != 0.0 {
                    acc + coeff * gurobi.vars[idx]
                } else {
                    acc
                }
            });

    if let Err(e) = gurobi.model.set_objective(obj_expr, sense) {
        return SolveOutcome::Failed {
            raw: format!("Gurobi: failed to set objective: {}", e),
        };
    }

    if let Err(e) = gurobi.model.optimize() {
        return SolveOutcome::Failed {
            raw: format!("Gurobi: failed to optimize: {}", e),
        };
    }

    let status = match gurobi.model.status() {
        Ok(status) => status,
        Err(e) => {
            return SolveOutcome::Failed {
                raw: format!("Gurobi: failed to get model status: {}", e),
            }
        }
    };
    let solution_count = gurobi.model.get_attr(attr::SolCount).unwrap_or(0);

    match classify_status(status, solution_count) {
        RunResult::Optimal => {
            let (values, objective) = read_solution(gurobi, polyhedron, objective);
            SolveOutcome::Optimal { values, objective }
        }
        RunResult::Feasible => {
            let (values, objective) = read_solution(gurobi, polyhedron, objective);
            SolveOutcome::Feasible { values, objective }
        }
        RunResult::NoSolution(outcome) => outcome,
    }
}

/// How a finished Gurobi optimization should be handled.
#[derive(Debug, PartialEq)]
enum RunResult {
    /// Proven optimal: read the solution.
    Optimal,
    /// Stopped early but with at least one solution: read the solution.
    Feasible,
    /// No usable solution: return this outcome as is.
    NoSolution(SolveOutcome),
}

/// Decide what a Gurobi status means. Pure function, no solver calls.
fn classify_status(status: grb::Status, solution_count: i32) -> RunResult {
    let raw = format!("Gurobi: status {:?}", status);

    match status {
        grb::Status::Optimal => RunResult::Optimal,
        grb::Status::Infeasible => RunResult::NoSolution(SolveOutcome::Infeasible { raw }),
        grb::Status::InfOrUnbd | grb::Status::Unbounded => {
            RunResult::NoSolution(SolveOutcome::Unbounded { raw })
        }
        // No solve has finished, so any solution count is not from this run.
        grb::Status::Loaded | grb::Status::InProgress => {
            RunResult::NoSolution(SolveOutcome::Failed { raw })
        }
        // Stopped before proving optimality (time limit, SubOptimal,
        // WorkLimit, MemLimit, ...). Gurobi only keeps feasible solutions,
        // so a non-zero solution count means a usable incumbent.
        _ if solution_count > 0 => RunResult::Feasible,
        _ => RunResult::NoSolution(SolveOutcome::Failed { raw }),
    }
}

/// Read variable values and objective value of the current solution.
fn read_solution(
    gurobi: &GurobiModel,
    polyhedron: &SparseLEIntegerPolyhedron,
    objective: &HashMap<String, f64>,
) -> (HashMap<String, i32>, i32) {
    // Map solution back to variable names
    let mut solution_map: HashMap<String, i32> = HashMap::new();
    for (idx, var) in polyhedron.variables.iter().enumerate() {
        let (lower, upper) = var.bound;

        // Get solution value, or use fixed value if variable was eliminated by presolve
        let value = gurobi
            .model
            .get_obj_attr(attr::X, &gurobi.vars[idx])
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
        .filter_map(|(var_id, &val)| objective.get(var_id).map(|coeff| coeff * (val as f64)))
        .sum();

    (solution_map, objective_value.round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimal_status_reads_solution() {
        assert_eq!(classify_status(grb::Status::Optimal, 1), RunResult::Optimal);
    }

    #[test]
    fn infeasible_status_is_infeasible() {
        assert_eq!(
            classify_status(grb::Status::Infeasible, 0),
            RunResult::NoSolution(SolveOutcome::Infeasible {
                raw: "Gurobi: status Infeasible".to_string()
            })
        );
    }

    #[test]
    fn inf_or_unbd_status_is_unbounded() {
        assert_eq!(
            classify_status(grb::Status::InfOrUnbd, 0),
            RunResult::NoSolution(SolveOutcome::Unbounded {
                raw: "Gurobi: status InfOrUnbd".to_string()
            })
        );
    }

    #[test]
    fn unbounded_status_is_unbounded() {
        assert!(matches!(
            classify_status(grb::Status::Unbounded, 0),
            RunResult::NoSolution(SolveOutcome::Unbounded { .. })
        ));
    }

    #[test]
    fn time_limit_with_solution_is_feasible() {
        assert_eq!(
            classify_status(grb::Status::TimeLimit, 1),
            RunResult::Feasible
        );
    }

    #[test]
    fn time_limit_without_solution_is_failed() {
        assert_eq!(
            classify_status(grb::Status::TimeLimit, 0),
            RunResult::NoSolution(SolveOutcome::Failed {
                raw: "Gurobi: status TimeLimit".to_string()
            })
        );
    }

    #[test]
    fn stopped_early_with_solution_is_feasible() {
        for status in [
            grb::Status::SubOptimal,
            grb::Status::UserObjLimit,
            grb::Status::WorkLimit,
            grb::Status::MemLimit,
            grb::Status::Numeric,
        ] {
            assert_eq!(
                classify_status(status, 3),
                RunResult::Feasible,
                "{:?}",
                status
            );
        }
    }

    #[test]
    fn unfinished_solve_is_failed_even_with_solution_count() {
        for status in [grb::Status::Loaded, grb::Status::InProgress] {
            assert!(
                matches!(
                    classify_status(status, 1),
                    RunResult::NoSolution(SolveOutcome::Failed { .. })
                ),
                "{:?}",
                status
            );
        }
    }

    #[test]
    fn other_status_is_failed() {
        assert!(matches!(
            classify_status(grb::Status::Numeric, 0),
            RunResult::NoSolution(SolveOutcome::Failed { .. })
        ));
    }
}
