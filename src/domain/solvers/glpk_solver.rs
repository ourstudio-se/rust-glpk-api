use crate::convert::{to_borrowed_objective, to_glpk_polyhedron};
use crate::domain::solver::Solver;
use crate::domain::validate::{validate_objectives_owned, SolveInputError};
use crate::models::{ApiSolution, SolverDirection, SparseLEIntegerPolyhedron};
use std::collections::HashMap;

use glpk_rust::{solve_ilps as glpk_solve_ilps, Solution};

const NO_TERMINAL_OUTPUT: bool = false;

/// GLPK solver implementation
///
/// Note: GLPK does not support model caching due to its mutable API design.
/// The cache_size parameter is accepted for API consistency but has no effect.
pub struct GlpkSolver;

impl GlpkSolver {
    /// Create a new GLPK solver with specified cache size
    /// Note: Cache is not supported for GLPK, parameter ignored
    pub fn with_cache_size(_size: Option<usize>) -> Self {
        GlpkSolver
    }

    /// Create solver with caching disabled (same as default for GLPK)
    pub fn without_cache() -> Self {
        GlpkSolver
    }
}

impl Solver for GlpkSolver {
    fn solve(
        &self,
        polyhedron: &SparseLEIntegerPolyhedron,
        objectives: &[HashMap<String, f64>],
        direction: SolverDirection,
        _use_presolve: bool,
    ) -> Result<Vec<ApiSolution>, SolveInputError> {
        let glpk_polyhedron = to_glpk_polyhedron(polyhedron);

        // Validate objectives against variables
        validate_objectives_owned(&glpk_polyhedron.variables, objectives)?;

        // Convert to borrowed objectives for GLPK
        let borrowed_objectives: Vec<HashMap<&str, f64>> = objectives
            .iter()
            .map(|obj| to_borrowed_objective(obj))
            .collect();

        let maximize = direction == SolverDirection::Maximize;

        // Solver expects &mut
        let mut mut_polyhedron = glpk_polyhedron;

        // Call the GLPK library solver
        let lib_solutions: Vec<Solution> = glpk_solve_ilps(
            &mut mut_polyhedron,
            borrowed_objectives,
            maximize,
            _use_presolve,
            NO_TERMINAL_OUTPUT,
        );

        // Convert GLPK solutions to API solutions
        let api_solutions: Vec<ApiSolution> = lib_solutions.into_iter().map(|s| s.into()).collect();

        Ok(api_solutions)
    }

    fn name(&self) -> &str {
        "GLPK"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glpk_solver_simple_solve() {
        let solver = GlpkSolver::without_cache();
        let polyhedron = SparseLEIntegerPolyhedron {
            a: crate::models::ApiIntegerSparseMatrix {
                rows: vec![0, 0],
                cols: vec![0, 1],
                vals: vec![-1, -1],
                shape: crate::models::ApiShape { nrows: 1, ncols: 2 },
            },
            b: vec![-1],
            variables: vec![
                crate::models::ApiVariable {
                    id: "x".to_string(),
                    bound: (0, 1),
                },
                crate::models::ApiVariable {
                    id: "y".to_string(),
                    bound: (0, 1),
                },
            ],
        };
        let objectives = vec![HashMap::from([
            ("x".to_string(), 1.0),
            ("y".to_string(), 1.0),
        ])];
        let direction = SolverDirection::Maximize;
        let solutions = solver.solve(&polyhedron, &objectives, direction, false);
        match solutions {
            Ok(solutions) => {
                assert_eq!(solutions.len(), 1);
                let solution = &solutions[0];
                assert_eq!(solution.error, None);
                assert_eq!(solution.objective, 2.0);
                assert_eq!(solution.solution.get("x"), Some(&1));
                assert_eq!(solution.solution.get("y"), Some(&1));
            }
            Err(e) => panic!("Solver failed with error: {:?}", e.details),
        }
    }

    #[test]
    fn test_glpk_solver_f64_max_obj_vals_returns_correct_solution() {
        /*
            Testing that our assumptions of the handling of too large objective coefficients are correct.
            - We expect that GLPK will return an infinite objective value but still provide a valid solution for x and y.
        */
        let solver = GlpkSolver::without_cache();
        let polyhedron = SparseLEIntegerPolyhedron {
            a: crate::models::ApiIntegerSparseMatrix {
                rows: vec![0, 0, 0],
                cols: vec![0, 1, 2],
                vals: vec![-1, -1, 1],
                shape: crate::models::ApiShape { nrows: 1, ncols: 3 },
            },
            b: vec![-1],
            variables: vec![
                crate::models::ApiVariable {
                    id: "x".to_string(),
                    bound: (0, 1),
                },
                crate::models::ApiVariable {
                    id: "y".to_string(),
                    bound: (0, 1),
                },
                crate::models::ApiVariable {
                    id: "z".to_string(),
                    bound: (0, 1),
                },
            ],
        };
        let objectives = vec![HashMap::from([
            ("x".to_string(), f64::MAX),
            ("y".to_string(), f64::MAX),
            ("z".to_string(), f64::MAX),
        ])];
        let direction = SolverDirection::Maximize;
        let solutions = solver.solve(&polyhedron, &objectives, direction, false);
        match solutions {
            Ok(solutions) => {
                assert_eq!(solutions.len(), 1);
                let solution = &solutions[0];
                assert_eq!(solution.error, None);
                println!(
                    "Objective value with max coefficients: {}",
                    solution.objective
                );
                // We expect the objective to be inf but still x and y to be 1
                assert_eq!(solution.objective, f64::INFINITY);
                assert_eq!(solution.solution.get("x"), Some(&1));
                assert_eq!(solution.solution.get("y"), Some(&1));
                assert_eq!(solution.solution.get("z"), Some(&1));
            }
            Err(e) => panic!("Solver failed with error: {:?}", e.details),
        }
    }

    #[test]
    fn test_glpk_solver_tolerance() {
        /*
            According to wiki and double precision floating points:
            "
                Between 2^52=4,503,599,627,370,496 and 2^53=9,007,199,254,740,992 the representable numbers are exactly the integers.
                For the next range, from 2^53 to 2^54, everything is multiplied by 2, so the representable numbers are the even ones, etc.
                Conversely, for the previous range from 2^51 to 2^52, the spacing is 0.5, etc.
            "
            This means that 2^52 ≈ 2^52+1 and so glpk won't make any difference
        */
        let solver = GlpkSolver::without_cache();
        let polyhedron = SparseLEIntegerPolyhedron {
            a: crate::models::ApiIntegerSparseMatrix {
                rows: vec![0, 0, 0],
                cols: vec![0, 1, 2],
                vals: vec![-1, -1, 1],
                shape: crate::models::ApiShape { nrows: 1, ncols: 3 },
            },
            b: vec![-1],
            variables: vec![
                crate::models::ApiVariable {
                    id: "x".to_string(),
                    bound: (0, 1),
                },
                crate::models::ApiVariable {
                    id: "y".to_string(),
                    bound: (0, 1),
                },
                crate::models::ApiVariable {
                    id: "z".to_string(),
                    bound: (0, 1),
                },
            ],
        };
        // The max exp is one thing in GLPK whereas the tolerance is another.
        // The tolerance seams to be 33 (or 2^(-33)), e.g. the diff between the smallest and largest number used.
        const EXP_TOL_MAX: i32 = 33;
        let direction = SolverDirection::Maximize;
        for i in 0..4 {
            let objective = HashMap::from([
                ("x".to_string(), 2f64.powi(EXP_TOL_MAX + i)),
                ("y".to_string(), 1.0),
                ("z".to_string(), 1.0),
            ]);
            let api_solutions = solver
                .solve(&polyhedron, &[objective], direction, false)
                .ok()
                .unwrap();
            let api_solution = &api_solutions[0];
            // Only first solution should be correct
            if i == 0 {
                assert_eq!(api_solution.objective, 2f64.powi(EXP_TOL_MAX) + 2.0);
                assert_eq!(api_solution.solution.get("x"), Some(&1));
                assert_eq!(api_solution.solution.get("y"), Some(&1));
                assert_eq!(api_solution.solution.get("z"), Some(&1));
            } else {
                assert_eq!(api_solution.objective, 2f64.powi(EXP_TOL_MAX + i));
                assert_eq!(api_solution.solution.get("x"), Some(&1));
                assert_eq!(api_solution.solution.get("y"), Some(&0));
                assert_eq!(api_solution.solution.get("z"), Some(&0));
            }
        }

        // However, sending many objectives at the time will set glpk to use the same tolerance for all of them, which means that some of them will be affected by the tolerance.
        let objectives = (0..4)
            .map(|i| {
                HashMap::from([
                    ("x".to_string(), 2f64.powi(EXP_TOL_MAX + i)),
                    ("y".to_string(), 1.0),
                    ("z".to_string(), 1.0),
                ])
            })
            .collect::<Vec<_>>();
        let api_solutions = solver
            .solve(&polyhedron, &objectives, direction, false)
            .ok()
            .unwrap();
        for (i, api_solution) in api_solutions.iter().enumerate() {
            // Due to the tolerance, all objectives will be treated as if they were the same, which means that all of them will have the same solution and objective value as the first one.
            assert_eq!(
                api_solution.objective,
                2f64.powi(EXP_TOL_MAX + (i as i32)) + 2.0
            );
            assert_eq!(api_solution.solution.get("x"), Some(&1));
            assert_eq!(api_solution.solution.get("y"), Some(&1));
            assert_eq!(api_solution.solution.get("z"), Some(&1));
        }

        // But if we change the order of them, the last one will be the one that is correct and the rest will be affected by the tolerance.
        let objectives = (0..4)
            .rev()
            .map(|i| {
                HashMap::from([
                    ("x".to_string(), 2f64.powi(EXP_TOL_MAX + i)),
                    ("y".to_string(), 1.0),
                    ("z".to_string(), 1.0),
                ])
            })
            .collect::<Vec<_>>();
        let api_solutions = solver
            .solve(&polyhedron, &objectives, direction, false)
            .ok()
            .unwrap();
        for (i, api_solution) in api_solutions.iter().enumerate() {
            // Due to the tolerance, all objectives will be treated as if they were the same, which means that all of them will have the same solution and objective value as the first one.
            if i == 3 {
                assert_eq!(api_solution.objective, 2f64.powi(EXP_TOL_MAX) + 2.0);
                assert_eq!(api_solution.solution.get("x"), Some(&1));
                assert_eq!(api_solution.solution.get("y"), Some(&1));
                assert_eq!(api_solution.solution.get("z"), Some(&1));
            } else {
                assert_eq!(
                    api_solution.objective,
                    2f64.powi(EXP_TOL_MAX + ((3 - i) as i32))
                );
                assert_eq!(api_solution.solution.get("x"), Some(&1));
                assert_eq!(api_solution.solution.get("y"), Some(&0));
                assert_eq!(api_solution.solution.get("z"), Some(&0));
            }
        }
    }
}
