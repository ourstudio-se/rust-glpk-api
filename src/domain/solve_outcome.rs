//! Maps each solver's native status codes onto the unified API contract:
//! `Optimal` / `Feasible` carry a solution, every other outcome becomes
//! `MIPFailed` with an error of the form
//! `<Summary>: <Solver> reported <NAME> (<code>)`.
//!
//! The mappings work on plain integer codes so they compile and are tested
//! without any solver feature flags (Gurobi cannot be built everywhere).
// The HiGHS/Gurobi mappings are only called when their feature is enabled,
// but are always compiled so their unit tests run in every build.
#![cfg_attr(
    not(all(feature = "highs-solver", feature = "gurobi-solver")),
    allow(dead_code)
)]

use crate::models::{ApiSolution, Status};
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Optimal,
    Feasible,
    /// Full error message for a `MIPFailed` solution
    Failed(String),
}

enum Failure {
    Infeasible,
    Unbounded,
    InfeasibleOrUnbounded,
    TimeLimit,
    SolverFailed,
}

impl Failure {
    fn summary(&self) -> &'static str {
        match self {
            Failure::Infeasible => "Problem is infeasible",
            Failure::Unbounded => "Problem is unbounded",
            Failure::InfeasibleOrUnbounded => "Problem is infeasible or unbounded",
            Failure::TimeLimit => "Time limit reached without a solution",
            Failure::SolverFailed => "Solver failed",
        }
    }
}

fn failed(failure: Failure, solver: &str, name: &str, code: i32) -> Outcome {
    Outcome::Failed(format!(
        "{}: {} reported {} ({})",
        failure.summary(),
        solver,
        name,
        code
    ))
}

/// Solution returned for every outcome that is not `Optimal` or `Feasible`.
pub fn failed_solution(error: String) -> ApiSolution {
    ApiSolution {
        status: Status::MIPFailed,
        objective: 0,
        solution: HashMap::new(),
        error: Some(error),
    }
}

// ---------- GLPK ----------

/// GLPK return codes from glp_simplex / glp_intopt (glpk.h)
fn glpk_return_code_name(code: i32) -> Option<&'static str> {
    Some(match code {
        1 => "GLP_EBADB",
        2 => "GLP_ESING",
        3 => "GLP_ECOND",
        4 => "GLP_EBOUND",
        5 => "GLP_EFAIL",
        6 => "GLP_EOBJLL",
        7 => "GLP_EOBJUL",
        8 => "GLP_EITLIM",
        9 => "GLP_ETMLIM",
        10 => "GLP_ENOPFS",
        11 => "GLP_ENODFS",
        12 => "GLP_EROOT",
        13 => "GLP_ESTOP",
        14 => "GLP_EMIPGAP",
        15 => "GLP_ENOFEAS",
        16 => "GLP_ENOCVG",
        17 => "GLP_EINSTAB",
        18 => "GLP_EDATA",
        19 => "GLP_ERANGE",
        _ => return None,
    })
}

/// Map a glpk-rust solution status (its `Status` discriminant) and error text.
///
/// glpk-rust only exposes the glp_simplex / glp_intopt return code inside the
/// error text ("GLPK ... solver failed with code: N"), so it is parsed from there.
pub fn glpk(status: i32, error: Option<&str>) -> Outcome {
    const SOLVER: &str = "GLPK";
    let routine = match status {
        5 => return Outcome::Optimal,
        2 => return Outcome::Feasible,
        1 => return failed(Failure::SolverFailed, SOLVER, "GLP_UNDEF", 1),
        3 => return failed(Failure::Infeasible, SOLVER, "GLP_INFEAS", 3),
        4 => return failed(Failure::Infeasible, SOLVER, "GLP_NOFEAS", 4),
        6 => return failed(Failure::Unbounded, SOLVER, "GLP_UNBND", 6),
        7 => "glp_simplex", // Status::SimplexFailed
        8 => "glp_intopt",  // Status::MIPFailed
        other => {
            return failed(Failure::SolverFailed, SOLVER, "status", other);
        }
    };

    let error = error.unwrap_or_default();
    let code = error
        .rsplit_once("code: ")
        .and_then(|(_, c)| c.trim().parse::<i32>().ok());
    let Some(code) = code else {
        return Outcome::Failed(format!(
            "{}: {} reported {}",
            Failure::SolverFailed.summary(),
            SOLVER,
            error
        ));
    };

    let failure = match code {
        10 => Failure::Infeasible,                 // GLP_ENOPFS
        11 | 12 => Failure::InfeasibleOrUnbounded, // GLP_ENODFS, GLP_EROOT
        9 => Failure::TimeLimit,                   // GLP_ETMLIM
        _ => Failure::SolverFailed,
    };
    let name = match glpk_return_code_name(code) {
        Some(n) => format!("{} {}", routine, n),
        None => routine.to_string(),
    };
    failed(failure, SOLVER, &name, code)
}

// ---------- HiGHS ----------

const HIGHS_RUN_STATUS_ERROR: i32 = -1;
const HIGHS_SOLUTION_STATUS_FEASIBLE: i32 = 2;

fn highs_model_status_name(code: i32) -> &'static str {
    match code {
        0 => "kNotset",
        1 => "kLoadError",
        2 => "kModelError",
        3 => "kPresolveError",
        4 => "kSolveError",
        5 => "kPostsolveError",
        6 => "kModelEmpty",
        7 => "kOptimal",
        8 => "kInfeasible",
        9 => "kUnboundedOrInfeasible",
        10 => "kUnbounded",
        11 => "kObjectiveBound",
        12 => "kObjectiveTarget",
        13 => "kTimeLimit",
        14 => "kIterationLimit",
        15 => "kUnknown",
        16 => "kSolutionLimit",
        17 => "kInterrupt",
        18 => "kMemoryLimit",
        _ => "kUnknownModelStatus",
    }
}

/// Map HiGHS run status (`Highs_run`), model status and primal solution status.
///
/// The model status is reported since it is the more specific one; the run
/// status is only reported when it is `kError` and no model status was set.
pub fn highs(run_status: i32, model_status: i32, primal_solution_status: i32) -> Outcome {
    const SOLVER: &str = "HiGHS";
    if run_status == HIGHS_RUN_STATUS_ERROR {
        return if model_status == 0 {
            failed(Failure::SolverFailed, SOLVER, "kError", run_status)
        } else {
            failed(
                Failure::SolverFailed,
                SOLVER,
                highs_model_status_name(model_status),
                model_status,
            )
        };
    }

    let failure = match model_status {
        7 => return Outcome::Optimal,
        // Stopped at a limit: usable only if a feasible solution was found
        11 | 12 | 13 | 14 | 16 | 17 | 18
            if primal_solution_status == HIGHS_SOLUTION_STATUS_FEASIBLE =>
        {
            return Outcome::Feasible
        }
        8 => Failure::Infeasible,
        9 => Failure::InfeasibleOrUnbounded,
        10 => Failure::Unbounded,
        13 => Failure::TimeLimit,
        _ => Failure::SolverFailed,
    };
    failed(
        failure,
        SOLVER,
        highs_model_status_name(model_status),
        model_status,
    )
}

// ---------- Gurobi ----------

fn gurobi_status_name(code: i32) -> &'static str {
    match code {
        1 => "LOADED",
        2 => "OPTIMAL",
        3 => "INFEASIBLE",
        4 => "INF_OR_UNBD",
        5 => "UNBOUNDED",
        6 => "CUTOFF",
        7 => "ITERATION_LIMIT",
        8 => "NODE_LIMIT",
        9 => "TIME_LIMIT",
        10 => "SOLUTION_LIMIT",
        11 => "INTERRUPTED",
        12 => "NUMERIC",
        13 => "SUBOPTIMAL",
        14 => "INPROGRESS",
        15 => "USER_OBJ_LIMIT",
        16 => "WORK_LIMIT",
        17 => "MEM_LIMIT",
        _ => "UNKNOWN_STATUS",
    }
}

/// Map Gurobi optimization status code and `SolCount`.
pub fn gurobi(status: i32, sol_count: i32) -> Outcome {
    let failure = match status {
        2 => return Outcome::Optimal,
        // Stopped early (limit, interrupt, suboptimal): usable if a solution exists
        7 | 8 | 9 | 10 | 11 | 13 | 15 | 16 | 17 if sol_count > 0 => return Outcome::Feasible,
        3 => Failure::Infeasible,
        4 => Failure::InfeasibleOrUnbounded,
        5 => Failure::Unbounded,
        9 => Failure::TimeLimit,
        _ => Failure::SolverFailed,
    };
    failed(failure, "Gurobi", gurobi_status_name(status), status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(s: &str) -> Outcome {
        Outcome::Failed(s.to_string())
    }

    #[test]
    fn glpk_mapping() {
        assert_eq!(glpk(5, None), Outcome::Optimal);
        assert_eq!(glpk(2, None), Outcome::Feasible);
        assert_eq!(
            glpk(8, Some("GLPK MIP solver failed with code: 10")),
            err("Problem is infeasible: GLPK reported glp_intopt GLP_ENOPFS (10)")
        );
        assert_eq!(
            glpk(8, Some("GLPK MIP solver failed with code: 11")),
            err("Problem is infeasible or unbounded: GLPK reported glp_intopt GLP_ENODFS (11)")
        );
        assert_eq!(
            glpk(8, Some("GLPK MIP solver failed with code: 12")),
            err("Problem is infeasible or unbounded: GLPK reported glp_intopt GLP_EROOT (12)")
        );
        assert_eq!(
            glpk(7, Some("GLPK simplex solver failed with code: 5")),
            err("Solver failed: GLPK reported glp_simplex GLP_EFAIL (5)")
        );
        assert_eq!(
            glpk(8, Some("GLPK MIP solver failed with code: 99")),
            err("Solver failed: GLPK reported glp_intopt (99)")
        );
        assert_eq!(
            glpk(8, Some("something unexpected")),
            err("Solver failed: GLPK reported something unexpected")
        );
        assert_eq!(
            glpk(4, Some("No feasible solution exists")),
            err("Problem is infeasible: GLPK reported GLP_NOFEAS (4)")
        );
        assert_eq!(
            glpk(3, None),
            err("Problem is infeasible: GLPK reported GLP_INFEAS (3)")
        );
        assert_eq!(
            glpk(6, None),
            err("Problem is unbounded: GLPK reported GLP_UNBND (6)")
        );
        assert_eq!(
            glpk(1, None),
            err("Solver failed: GLPK reported GLP_UNDEF (1)")
        );
    }

    #[test]
    fn highs_mapping() {
        assert_eq!(highs(0, 7, 2), Outcome::Optimal);
        assert_eq!(highs(1, 13, 2), Outcome::Feasible);
        assert_eq!(highs(1, 16, 2), Outcome::Feasible);
        assert_eq!(
            highs(0, 8, 0),
            err("Problem is infeasible: HiGHS reported kInfeasible (8)")
        );
        assert_eq!(
            highs(0, 9, 0),
            err("Problem is infeasible or unbounded: HiGHS reported kUnboundedOrInfeasible (9)")
        );
        assert_eq!(
            highs(0, 10, 2),
            err("Problem is unbounded: HiGHS reported kUnbounded (10)")
        );
        assert_eq!(
            highs(1, 13, 0),
            err("Time limit reached without a solution: HiGHS reported kTimeLimit (13)")
        );
        assert_eq!(
            highs(1, 14, 0),
            err("Solver failed: HiGHS reported kIterationLimit (14)")
        );
        assert_eq!(
            highs(-1, 0, 0),
            err("Solver failed: HiGHS reported kError (-1)")
        );
        assert_eq!(
            highs(-1, 4, 0),
            err("Solver failed: HiGHS reported kSolveError (4)")
        );
    }

    #[test]
    fn gurobi_mapping() {
        assert_eq!(gurobi(2, 1), Outcome::Optimal);
        assert_eq!(gurobi(9, 1), Outcome::Feasible);
        assert_eq!(gurobi(13, 1), Outcome::Feasible);
        assert_eq!(
            gurobi(3, 0),
            err("Problem is infeasible: Gurobi reported INFEASIBLE (3)")
        );
        assert_eq!(
            gurobi(4, 0),
            err("Problem is infeasible or unbounded: Gurobi reported INF_OR_UNBD (4)")
        );
        assert_eq!(
            gurobi(5, 0),
            err("Problem is unbounded: Gurobi reported UNBOUNDED (5)")
        );
        assert_eq!(
            gurobi(9, 0),
            err("Time limit reached without a solution: Gurobi reported TIME_LIMIT (9)")
        );
        assert_eq!(
            gurobi(12, 0),
            err("Solver failed: Gurobi reported NUMERIC (12)")
        );
    }

    #[test]
    fn failed_solution_shape() {
        let s = failed_solution("x".into());
        assert!(matches!(s.status, Status::MIPFailed));
        assert_eq!(s.objective, 0);
        assert!(s.solution.is_empty());
        assert_eq!(s.error.as_deref(), Some("x"));
    }
}
