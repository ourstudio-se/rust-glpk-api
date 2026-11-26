//! The response contract towards clients.
//!
//! Every solver translates its own result into a [`SolveOutcome`]. This module
//! is the only place that decides how an outcome is presented in the API, so
//! that clients see the same behaviour regardless of which solver is running:
//!
//! | Outcome                                  | `status`    | `solution` / `objective` | `error`                      |
//! |------------------------------------------|-------------|--------------------------|------------------------------|
//! | `Optimal`                                | `Optimal`   | filled in                | `null`                       |
//! | `Feasible`                               | `Feasible`  | filled in                | `null`                       |
//! | `Infeasible`, `Unbounded`, `Failed`      | `MIPFailed` | `{}` / `0`               | `"<summary> (<raw error>)"`  |

use crate::models::{ApiSolution, Status};
use std::collections::HashMap;

/// What happened when solving one objective.
///
/// `raw` is the solver's own description of the result, prefixed with the
/// solver name, e.g. `"HiGHS: model status 8 kInfeasible"`.
#[derive(Debug, PartialEq)]
pub enum SolveOutcome {
    /// A proven optimal solution.
    Optimal {
        values: HashMap<String, i32>,
        objective: i32,
    },
    /// A valid solution that is not proven optimal (e.g. a limit was reached).
    Feasible {
        values: HashMap<String, i32>,
        objective: i32,
    },
    /// The problem has no solution.
    Infeasible { raw: String },
    /// The problem is unbounded, or the solver could not tell unbounded from infeasible.
    Unbounded { raw: String },
    /// The solver did not produce a usable solution for any other reason.
    Failed { raw: String },
}

impl SolveOutcome {
    pub fn into_api_solution(self) -> ApiSolution {
        match self {
            SolveOutcome::Optimal { values, objective } => {
                solution(Status::Optimal, values, objective)
            }
            SolveOutcome::Feasible { values, objective } => {
                solution(Status::Feasible, values, objective)
            }
            SolveOutcome::Infeasible { raw } => failure("Problem is infeasible", raw),
            SolveOutcome::Unbounded { raw } => failure("Problem is unbounded or infeasible", raw),
            SolveOutcome::Failed { raw } => failure("Solver failed", raw),
        }
    }
}

fn solution(status: Status, values: HashMap<String, i32>, objective: i32) -> ApiSolution {
    ApiSolution {
        status,
        objective,
        solution: values,
        error: None,
    }
}

fn failure(summary: &str, raw: String) -> ApiSolution {
    ApiSolution {
        status: Status::MIPFailed,
        objective: 0,
        solution: HashMap::new(),
        error: Some(format!("{} ({})", summary, raw)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_values() -> HashMap<String, i32> {
        HashMap::from([("x".to_string(), 1), ("y".to_string(), 2)])
    }

    #[test]
    fn optimal_returns_solution_and_objective_without_error() {
        let api = SolveOutcome::Optimal {
            values: some_values(),
            objective: 7,
        }
        .into_api_solution();

        assert_eq!(api.status, Status::Optimal);
        assert_eq!(api.solution, some_values());
        assert_eq!(api.objective, 7);
        assert_eq!(api.error, None);
    }

    #[test]
    fn feasible_returns_solution_and_objective_without_error() {
        let api = SolveOutcome::Feasible {
            values: some_values(),
            objective: 5,
        }
        .into_api_solution();

        assert_eq!(api.status, Status::Feasible);
        assert_eq!(api.solution, some_values());
        assert_eq!(api.objective, 5);
        assert_eq!(api.error, None);
    }

    #[test]
    fn infeasible_returns_mipfailed_with_summary_and_raw_error() {
        let api = SolveOutcome::Infeasible {
            raw: "HiGHS: model status 8 kInfeasible".to_string(),
        }
        .into_api_solution();

        assert_eq!(api.status, Status::MIPFailed);
        assert!(api.solution.is_empty());
        assert_eq!(api.objective, 0);
        assert_eq!(
            api.error.as_deref(),
            Some("Problem is infeasible (HiGHS: model status 8 kInfeasible)")
        );
    }

    #[test]
    fn unbounded_returns_mipfailed_with_summary_and_raw_error() {
        let api = SolveOutcome::Unbounded {
            raw: "Gurobi: status InfOrUnbd".to_string(),
        }
        .into_api_solution();

        assert_eq!(api.status, Status::MIPFailed);
        assert!(api.solution.is_empty());
        assert_eq!(api.objective, 0);
        assert_eq!(
            api.error.as_deref(),
            Some("Problem is unbounded or infeasible (Gurobi: status InfOrUnbd)")
        );
    }

    #[test]
    fn failed_returns_mipfailed_with_summary_and_raw_error() {
        let api = SolveOutcome::Failed {
            raw: "GLPK: GLPK MIP solver failed with code: 10".to_string(),
        }
        .into_api_solution();

        assert_eq!(api.status, Status::MIPFailed);
        assert!(api.solution.is_empty());
        assert_eq!(api.objective, 0);
        assert_eq!(
            api.error.as_deref(),
            Some("Solver failed (GLPK: GLPK MIP solver failed with code: 10)")
        );
    }
}
