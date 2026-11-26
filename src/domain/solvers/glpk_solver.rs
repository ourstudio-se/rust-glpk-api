use crate::convert::{to_borrowed_objective, to_glpk_polyhedron};
use crate::domain::outcome::SolveOutcome;
use crate::domain::solver::{SolveError, Solver};
use crate::models::{SolverDirection, SparseLEIntegerPolyhedron};
use glpk_rust::{solve_ilps, Solution, Status as GlpkStatus};
use std::collections::HashMap;

const NO_TERMINAL_OUTPUT: bool = false;

/// GLPK solver implementation
///
/// Note: GLPK does not support model caching due to its mutable API design.
/// The cache_size parameter is accepted for API consistency but has no effect.
pub struct GlpkSolver {
    timeout_ms: Option<u32>,
}

impl GlpkSolver {
    /// Create a new GLPK solver with specified cache size
    /// Note: Cache is not supported for GLPK, parameter ignored
    pub fn with_cache_size(_size: Option<usize>, timeout_ms: Option<u32>) -> Self {
        GlpkSolver { timeout_ms }
    }

    /// Create solver with caching disabled (same as default for GLPK)
    pub fn without_cache(timeout_ms: Option<u32>) -> Self {
        GlpkSolver { timeout_ms }
    }
}

impl Solver for GlpkSolver {
    fn solve(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        objectives: &[HashMap<String, f64>],
        direction: SolverDirection,
        use_presolve: bool,
    ) -> Result<Vec<SolveOutcome>, SolveError> {
        // Solver expects &mut
        let mut glpk_polyhedron = to_glpk_polyhedron(polyhedron);

        // Convert to borrowed objectives for GLPK
        let borrowed_objectives: Vec<HashMap<&str, f64>> =
            objectives.iter().map(to_borrowed_objective).collect();

        let maximize = direction == SolverDirection::Maximize;

        // Call the GLPK library solver
        let lib_solutions = solve_ilps(
            &mut glpk_polyhedron,
            borrowed_objectives,
            maximize,
            use_presolve,
            NO_TERMINAL_OUTPUT,
            self.timeout_ms,
        )?;

        Ok(lib_solutions.into_iter().map(outcome_from_glpk).collect())
    }

    fn name(&self) -> &str {
        "GLPK"
    }
}

/// Translate a `glpk-rust` solution into a `SolveOutcome`.
fn outcome_from_glpk(solution: Solution) -> SolveOutcome {
    let Solution {
        status,
        objective,
        solution: values,
        error,
    } = solution;

    match status {
        GlpkStatus::Optimal => SolveOutcome::Optimal {
            values,
            objective: objective as i32, // Match current api contract
        },
        GlpkStatus::Feasible => SolveOutcome::Feasible {
            values,
            objective: objective as i32, // Match current api contract
        },
        GlpkStatus::Infeasible | GlpkStatus::NoFeasible => SolveOutcome::Infeasible {
            raw: glpk_raw_error(&status, error),
        },
        GlpkStatus::Unbounded => SolveOutcome::Unbounded {
            raw: glpk_raw_error(&status, error),
        },
        GlpkStatus::SimplexFailed
        | GlpkStatus::MIPFailed
        | GlpkStatus::Undefined
        | GlpkStatus::EmptySpace => SolveOutcome::Failed {
            raw: glpk_raw_error(&status, error),
        },
    }
}

/// The library's own error message, or the status name if there is none.
fn glpk_raw_error(status: &GlpkStatus, error: Option<String>) -> String {
    let message = error.unwrap_or_else(|| format!("status {:?}", status));
    format!("GLPK: {}", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glpk_solution(status: GlpkStatus, error: Option<&str>) -> Solution {
        Solution {
            status,
            objective: 0.0,
            solution: HashMap::new(),
            error: error.map(str::to_string),
        }
    }

    #[test]
    fn optimal_keeps_values_and_truncated_objective() {
        let solution = Solution {
            status: GlpkStatus::Optimal,
            objective: 2.7,
            solution: HashMap::from([("x".to_string(), 1)]),
            error: None,
        };

        assert_eq!(
            outcome_from_glpk(solution),
            SolveOutcome::Optimal {
                values: HashMap::from([("x".to_string(), 1)]),
                objective: 2,
            }
        );
    }

    #[test]
    fn feasible_keeps_values_and_objective() {
        let solution = Solution {
            status: GlpkStatus::Feasible,
            objective: 3.0,
            solution: HashMap::from([("x".to_string(), 3)]),
            error: None,
        };

        assert_eq!(
            outcome_from_glpk(solution),
            SolveOutcome::Feasible {
                values: HashMap::from([("x".to_string(), 3)]),
                objective: 3,
            }
        );
    }

    #[test]
    fn no_feasible_maps_to_infeasible_with_library_message() {
        let solution = glpk_solution(GlpkStatus::NoFeasible, Some("No feasible solution exists"));

        assert_eq!(
            outcome_from_glpk(solution),
            SolveOutcome::Infeasible {
                raw: "GLPK: No feasible solution exists".to_string()
            }
        );
    }

    #[test]
    fn infeasible_maps_to_infeasible() {
        let solution = glpk_solution(GlpkStatus::Infeasible, Some("Infeasible solution exists"));

        assert!(matches!(
            outcome_from_glpk(solution),
            SolveOutcome::Infeasible { .. }
        ));
    }

    #[test]
    fn unbounded_maps_to_unbounded() {
        let solution = glpk_solution(GlpkStatus::Unbounded, Some("Problem is unbounded"));

        assert!(matches!(
            outcome_from_glpk(solution),
            SolveOutcome::Unbounded { .. }
        ));
    }

    #[test]
    fn mip_failed_maps_to_failed_with_library_message() {
        let solution = glpk_solution(
            GlpkStatus::MIPFailed,
            Some("GLPK MIP solver failed with code: 10"),
        );

        assert_eq!(
            outcome_from_glpk(solution),
            SolveOutcome::Failed {
                raw: "GLPK: GLPK MIP solver failed with code: 10".to_string()
            }
        );
    }

    #[test]
    fn simplex_failed_maps_to_failed() {
        let solution = glpk_solution(
            GlpkStatus::SimplexFailed,
            Some("GLPK simplex solver failed with code: 4"),
        );

        assert!(matches!(
            outcome_from_glpk(solution),
            SolveOutcome::Failed { .. }
        ));
    }

    #[test]
    fn status_without_message_uses_status_name() {
        let solution = glpk_solution(GlpkStatus::EmptySpace, None);

        assert_eq!(
            outcome_from_glpk(solution),
            SolveOutcome::Failed {
                raw: "GLPK: status EmptySpace".to_string()
            }
        );
    }
}
