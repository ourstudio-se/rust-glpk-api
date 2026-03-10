use crate::convert::to_glpk_polyhedron;
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveInputError};
use crate::models::{ApiSolution, SolverDirection, SparseLEIntegerPolyhedron, Status};
use std::collections::HashMap;
use std::sync::Arc;

use grb::prelude::*;
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;

/// Cached Gurobi model structure
struct CachedGurobiModel {
    model: Model,
    vars: Vec<Var>,
    n_vars: usize,
}

// SAFETY: Gurobi model is properly synchronized through Arc and Mutex
// Each model instance is only accessed by one thread at a time
unsafe impl Send for CachedGurobiModel {}
unsafe impl Sync for CachedGurobiModel {}

/// Gurobi solver implementation with model caching
///
/// This implementation includes model caching:
/// - Models are cached based on polyhedron hash
/// - LRU eviction policy when cache is full
/// - Reuses cached models across multiple objectives
/// - Thread-safe via parking_lot::Mutex
pub struct GurobiSolver {
    model_cache: Arc<Mutex<LruCache<SparseLEIntegerPolyhedron, Arc<Mutex<CachedGurobiModel>>>>>,
    cache_enabled: bool,
}

impl GurobiSolver {
    pub fn new() -> Self {
        Self::with_cache_size(100)
    }

    /// Create a new Gurobi solver with specified cache size
    pub fn with_cache_size(size: usize) -> Self {
        let cache_size = NonZeroUsize::new(size).unwrap_or(NonZeroUsize::new(100).unwrap());
        GurobiSolver {
            model_cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
            cache_enabled: size > 0,
        }
    }

    /// Create solver with caching disabled
    pub fn without_cache() -> Self {
        GurobiSolver {
            model_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1).unwrap()))),
            cache_enabled: false,
        }
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

    /// Build a new Gurobi model for the given polyhedron
    fn build_model(
        polyhedron: &SparseLEIntegerPolyhedron,
        use_presolve: bool,
    ) -> Result<Arc<Mutex<CachedGurobiModel>>, SolveInputError> {
        // Create Gurobi environment
        let mut env = Env::new("").map_err(|e| SolveInputError {
            details: format!("Failed to create Gurobi environment: {}", e),
        })?;

        // Disable Gurobi console output
        env.set(param::OutputFlag, 0).map_err(|e| SolveInputError {
            details: format!("Failed to set Gurobi output flag: {}", e),
        })?;

        // Use all available threads
        env.set(param::Threads, 0).map_err(|e| SolveInputError {
            details: format!("Failed to set Gurobi thread count: {}", e),
        })?;

        // Configure presolve: -1 = auto, 0 = off, 1 = conservative, 2 = aggressive
        env.set(param::Presolve, if use_presolve { -1 } else { 0 })
            .map_err(|e| SolveInputError {
                details: format!("Failed to set Gurobi presolve: {}", e),
            })?;

        // Create a Gurobi model
        let mut model = Model::with_env("optimization", &env).map_err(|e| SolveInputError {
            details: format!("Failed to create Gurobi model: {}", e),
        })?;

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
                .map_err(|e| SolveInputError {
                    details: format!("Failed to add binary variable: {}", e),
                })?
            } else {
                add_intvar!(
                    model,
                    name: &var.id,
                    bounds: lower as f64..upper as f64
                )
                .map_err(|e| SolveInputError {
                    details: format!("Failed to add integer variable: {}", e),
                })?
            };

            vars.push(gurobi_var);
        }

        model.update().map_err(|e| SolveInputError {
            details: format!("Failed to update model after adding variables: {}", e),
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
                .map_err(|e| SolveInputError {
                    details: format!("Failed to add constraint: {}", e),
                })?;
        }

        model.update().map_err(|e| SolveInputError {
            details: format!("Failed to update model after adding constraints: {}", e),
        })?;

        Ok(Arc::new(Mutex::new(CachedGurobiModel {
            model,
            vars,
            n_vars: polyhedron.variables.len(),
        })))
    }

    /// Get or build a model for the given polyhedron
    fn get_or_build_model(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        use_presolve: bool,
    ) -> Result<Arc<Mutex<CachedGurobiModel>>, SolveInputError> {
        if !self.cache_enabled {
            // Cache disabled, always build new model
            return Self::build_model(polyhedron, use_presolve);
        }

        // Check cache first
        {
            let mut cache = self.model_cache.lock();
            if let Some(cached_model) = cache.get(polyhedron) {
                return Ok(Arc::clone(cached_model));
            }
        }

        // Not in cache, build new model
        let model = Self::build_model(polyhedron, use_presolve)?;

        // Store in cache
        {
            let mut cache = self.model_cache.lock();
            cache.put(polyhedron.clone(), Arc::clone(&model));
        }

        Ok(model)
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

        // Get or build cached model
        let cached_model = self.get_or_build_model(polyhedron, use_presolve)?;
        let mut model_lock = cached_model.lock();

        let sense = match direction {
            SolverDirection::Maximize => ModelSense::Maximize,
            SolverDirection::Minimize => ModelSense::Minimize,
        };

        let mut solutions = Vec::new();

        // Solve each objective by updating objective coefficients
        for objective in objectives {
            // Build objective expression
            let obj_expr = polyhedron.variables.iter().enumerate().fold(
                Expr::Constant(0.0),
                |acc, (idx, var)| {
                    let coeff = objective.get(&var.id).copied().unwrap_or(0.0);
                    if coeff != 0.0 {
                        acc + coeff * model_lock.vars[idx]
                    } else {
                        acc
                    }
                },
            );

            model_lock
                .model
                .set_objective(obj_expr, sense)
                .map_err(|e| SolveInputError {
                    details: format!("Failed to set objective: {}", e),
                })?;

            // Optimize
            model_lock.model.optimize().map_err(|e| SolveInputError {
                details: format!("Failed to optimize: {}", e),
            })?;

            // Extract solution
            let model_status = model_lock.model.status().map_err(|e| SolveInputError {
                details: format!("Failed to get model status: {}", e),
            })?;
            let status = Self::convert_status(model_status);

            // Map solution back to variable names
            let mut solution_map: HashMap<String, i32> = HashMap::new();
            for (idx, var) in polyhedron.variables.iter().enumerate() {
                let (lower, upper) = var.bound;

                // Get solution value, or use fixed value if variable was eliminated by presolve
                let value = model_lock
                    .model
                    .get_obj_attr(attr::X, &model_lock.vars[idx])
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
