//! Contract tests: the same scenarios run against every compiled-in solver.
//!
//! These tests guarantee that clients see the same `status`, `solution`,
//! `objective` and `error` behaviour regardless of solver (see `domain::outcome`).
//!
//! - `cargo test`                          → GLPK
//! - `cargo test --features highs-solver`  → GLPK + HiGHS (with and without cache)
//! - `cargo test --features gurobi-solver` → GLPK + Gurobi (with and without cache)

use crate::domain::solve::solve_request;
use crate::domain::solver::Solver;
use crate::domain::solvers::GlpkSolver;
use crate::models::{
    ApiIntegerSparseMatrix, ApiShape, ApiSolution, ApiVariable, SolveRequest, SolverDirection,
    SparseLEIntegerPolyhedron, Status,
};
use std::collections::HashMap;

#[cfg(feature = "highs-solver")]
use crate::domain::solvers::HighsSolver;

#[cfg(feature = "gurobi-solver")]
use crate::domain::solvers::GurobiSolver;

const PRESOLVE_SETTINGS: [bool; 2] = [true, false];

/// Every solver compiled into this build, with a label for failure messages.
fn available_solvers() -> Vec<(&'static str, Box<dyn Solver>)> {
    #[allow(unused_mut)]
    let mut solvers: Vec<(&'static str, Box<dyn Solver>)> =
        vec![("GLPK", Box::new(GlpkSolver::without_cache(None)))];

    #[cfg(feature = "highs-solver")]
    {
        solvers.push(("HiGHS", Box::new(HighsSolver::without_cache())));
        solvers.push((
            "HiGHS (cached)",
            Box::new(HighsSolver::with_cache_size(Some(10))),
        ));
    }

    #[cfg(feature = "gurobi-solver")]
    {
        solvers.push(("Gurobi", Box::new(GurobiSolver::without_cache())));
        solvers.push((
            "Gurobi (cached)",
            Box::new(GurobiSolver::with_cache_size(Some(10))),
        ));
    }

    solvers
}

// ---------- Test problems ----------

fn variable(id: &str, lower: i32, upper: i32) -> ApiVariable {
    ApiVariable {
        id: id.to_string(),
        bound: (lower, upper),
    }
}

fn objective(coefficients: &[(&str, f64)]) -> HashMap<String, f64> {
    coefficients
        .iter()
        .map(|(id, coeff)| (id.to_string(), *coeff))
        .collect()
}

/// 2 <= x + y <= 4, with x, y in [0, 3].
fn feasible_problem() -> SparseLEIntegerPolyhedron {
    SparseLEIntegerPolyhedron {
        a: ApiIntegerSparseMatrix {
            rows: vec![0, 0, 1, 1],
            cols: vec![0, 1, 0, 1],
            vals: vec![1, 1, -1, -1],
            shape: ApiShape { nrows: 2, ncols: 2 },
        },
        b: vec![4, -2],
        variables: vec![variable("x", 0, 3), variable("y", 0, 3)],
    }
}

/// x <= 3 with x in [5, 10]: infeasible even without integer requirements.
fn lp_infeasible_problem() -> SparseLEIntegerPolyhedron {
    SparseLEIntegerPolyhedron {
        a: ApiIntegerSparseMatrix {
            rows: vec![0],
            cols: vec![0],
            vals: vec![1],
            shape: ApiShape { nrows: 1, ncols: 1 },
        },
        b: vec![3],
        variables: vec![variable("x", 5, 10)],
    }
}

/// 2x <= 1 and 2x >= 1 with x in [0, 1]: only x = 0.5 fits, so no integer solution.
fn integer_infeasible_problem() -> SparseLEIntegerPolyhedron {
    SparseLEIntegerPolyhedron {
        a: ApiIntegerSparseMatrix {
            rows: vec![0, 1],
            cols: vec![0, 0],
            vals: vec![2, -2],
            shape: ApiShape { nrows: 2, ncols: 1 },
        },
        b: vec![1, -1],
        variables: vec![variable("x", 0, 1)],
    }
}

fn infeasible_problems() -> Vec<(&'static str, SparseLEIntegerPolyhedron)> {
    vec![
        ("lp infeasible", lp_infeasible_problem()),
        ("integer infeasible", integer_infeasible_problem()),
    ]
}

fn solve(
    solver: &dyn Solver,
    polyhedron: SparseLEIntegerPolyhedron,
    objectives: Vec<HashMap<String, f64>>,
    direction: SolverDirection,
    use_presolve: bool,
) -> Vec<ApiSolution> {
    let request = SolveRequest {
        polyhedron,
        objectives,
        direction,
    };
    match solve_request(solver, request, use_presolve) {
        Ok(solutions) => solutions,
        Err(error) => panic!("{} returned an error: {}", solver.name(), error.details),
    }
}

fn assert_mipfailed_without_solution(result: &ApiSolution, context: &str) {
    assert_eq!(result.status, Status::MIPFailed, "{}", context);
    assert!(result.solution.is_empty(), "{}", context);
    assert_eq!(result.objective, 0, "{}", context);
    assert!(result.error.is_some(), "{}", context);
}

// ---------- Tests ----------

#[test]
fn maximize_returns_optimal_with_solution() {
    for (label, solver) in available_solvers() {
        let results = solve(
            solver.as_ref(),
            feasible_problem(),
            vec![objective(&[("x", 1.0), ("y", 2.0)])],
            SolverDirection::Maximize,
            true,
        );

        let result = &results[0];
        assert_eq!(result.status, Status::Optimal, "{}", label);
        assert_eq!(result.solution["x"], 1, "{}", label);
        assert_eq!(result.solution["y"], 3, "{}", label);
        assert_eq!(result.objective, 7, "{}", label);
        assert_eq!(result.error, None, "{}", label);
    }
}

#[test]
fn minimize_returns_optimal_with_solution() {
    for (label, solver) in available_solvers() {
        let results = solve(
            solver.as_ref(),
            feasible_problem(),
            vec![objective(&[("x", 2.0), ("y", 1.0)])],
            SolverDirection::Minimize,
            true,
        );

        let result = &results[0];
        assert_eq!(result.status, Status::Optimal, "{}", label);
        assert_eq!(result.solution["x"], 0, "{}", label);
        assert_eq!(result.solution["y"], 2, "{}", label);
        assert_eq!(result.objective, 2, "{}", label);
        assert_eq!(result.error, None, "{}", label);
    }
}

#[test]
fn infeasible_problem_returns_mipfailed_with_empty_solution() {
    for (label, solver) in available_solvers() {
        for (problem_name, problem) in infeasible_problems() {
            for use_presolve in PRESOLVE_SETTINGS {
                let context = format!("{}, {}, presolve={}", label, problem_name, use_presolve);
                let results = solve(
                    solver.as_ref(),
                    problem.clone(),
                    vec![objective(&[("x", 1.0)])],
                    SolverDirection::Maximize,
                    use_presolve,
                );

                assert_eq!(results.len(), 1, "{}", context);
                assert_mipfailed_without_solution(&results[0], &context);
            }
        }
    }
}

#[test]
fn non_optimal_result_never_contains_solution_values() {
    for (label, solver) in available_solvers() {
        for (problem_name, problem) in infeasible_problems() {
            let context = format!("{}, {}", label, problem_name);
            let results = solve(
                solver.as_ref(),
                problem,
                vec![objective(&[("x", 1.0)]), objective(&[("x", -1.0)])],
                SolverDirection::Minimize,
                true,
            );

            for result in &results {
                assert_mipfailed_without_solution(result, &context);
            }
        }
    }
}

#[test]
fn error_message_starts_with_summary_and_names_solver() {
    for (label, solver) in available_solvers() {
        let results = solve(
            solver.as_ref(),
            lp_infeasible_problem(),
            vec![objective(&[("x", 1.0)])],
            SolverDirection::Maximize,
            true,
        );

        let error = results[0].error.clone().unwrap_or_default();
        let has_summary = error.starts_with("Problem is infeasible (")
            || error.starts_with("Problem is unbounded or infeasible (")
            || error.starts_with("Solver failed (");
        assert!(has_summary, "{}: unexpected error '{}'", label, error);
        assert!(
            error.contains(solver.name()),
            "{}: error '{}' does not name the solver",
            label,
            error
        );
    }
}

#[test]
fn multiple_objectives_return_one_result_each_in_order() {
    for (label, solver) in available_solvers() {
        let results = solve(
            solver.as_ref(),
            feasible_problem(),
            vec![objective(&[("y", 1.0)]), objective(&[("x", 1.0)])],
            SolverDirection::Maximize,
            true,
        );

        assert_eq!(results.len(), 2, "{}", label);
        assert_eq!(results[0].status, Status::Optimal, "{}", label);
        assert_eq!(results[0].solution["y"], 3, "{}", label);
        assert_eq!(results[1].status, Status::Optimal, "{}", label);
        assert_eq!(results[1].solution["x"], 3, "{}", label);
    }
}

#[test]
fn objective_with_unknown_variable_is_rejected() {
    for (label, solver) in available_solvers() {
        let request = SolveRequest {
            polyhedron: feasible_problem(),
            objectives: vec![objective(&[("unknown", 1.0)])],
            direction: SolverDirection::Maximize,
        };

        assert!(
            solve_request(solver.as_ref(), request, true).is_err(),
            "{}",
            label
        );
    }
}

#[test]
fn repeated_requests_on_same_solver_give_same_results() {
    // Exercises model reuse for cached solvers: a failed solve must not
    // affect later solves of the same or another problem.
    for (label, solver) in available_solvers() {
        for round in 1..=2 {
            let context = format!("{}, round {}", label, round);

            let infeasible = solve(
                solver.as_ref(),
                lp_infeasible_problem(),
                vec![objective(&[("x", 1.0)])],
                SolverDirection::Maximize,
                true,
            );
            assert_mipfailed_without_solution(&infeasible[0], &context);

            let optimal = solve(
                solver.as_ref(),
                feasible_problem(),
                vec![objective(&[("x", 1.0), ("y", 2.0)])],
                SolverDirection::Maximize,
                true,
            );
            assert_eq!(optimal[0].status, Status::Optimal, "{}", context);
            assert_eq!(optimal[0].objective, 7, "{}", context);
        }
    }
}
