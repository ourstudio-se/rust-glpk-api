use crate::domain::solver::Solver;
use crate::domain::solvers::GlpkSolver;

#[cfg(feature = "highs-solver")]
use crate::domain::solvers::HighsSolver;

#[cfg(feature = "gurobi-solver")]
use crate::domain::solvers::GurobiSolver;

/// Available solver backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverType {
    Glpk,
    #[cfg(feature = "highs-solver")]
    Highs,
    #[cfg(feature = "gurobi-solver")]
    Gurobi,
}

impl SolverType {
    /// Parse solver type from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "glpk" => Some(SolverType::Glpk),
            #[cfg(feature = "highs-solver")]
            "highs" => Some(SolverType::Highs),
            #[cfg(not(feature = "highs-solver"))]
            "highs" => panic!("Highs solver specified in environment but feature flag not present. Enable using `--features highs-solver`"),
            #[cfg(feature = "gurobi-solver")]
            "gurobi" => Some(SolverType::Gurobi),
            #[cfg(not(feature = "gurobi-solver"))]
            "gurobi" => panic!("Gurobi solver specified in environment but feature flag not present. Enable using `--features gurobi-solver`"),
            _ => None,
        }
    }
}

/// Create a solver instance with specified cache size
pub fn create_solver_with_cache(
    solver_type: SolverType,
    cache_size: Option<usize>,
) -> Box<dyn Solver> {
    match solver_type {
        SolverType::Glpk => match cache_size {
            Some(size) => Box::new(GlpkSolver::with_cache_size(Some(size))),
            None => Box::new(GlpkSolver::without_cache()),
        },
        #[cfg(feature = "highs-solver")]
        SolverType::Highs => match cache_size {
            Some(size) => Box::new(HighsSolver::with_cache_size(Some(size))),
            None => Box::new(HighsSolver::without_cache()),
        },
        #[cfg(feature = "gurobi-solver")]
        SolverType::Gurobi => match cache_size {
            Some(size) => Box::new(GurobiSolver::with_cache_size(Some(size))),
            None => Box::new(GurobiSolver::without_cache()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn create_solver(solver_type: SolverType) -> Box<dyn Solver> {
        create_solver_with_cache(solver_type, None) // Default to no cache
    }

    #[test]
    fn test_solver_type_from_str() {
        assert_eq!(SolverType::from_str("glpk"), Some(SolverType::Glpk));
        assert_eq!(SolverType::from_str("GLPK"), Some(SolverType::Glpk));
        #[cfg(feature = "highs-solver")]
        assert_eq!(SolverType::from_str("highs"), Some(SolverType::Highs));
        #[cfg(feature = "highs-solver")]
        assert_eq!(SolverType::from_str("HiGHS"), Some(SolverType::Highs));
        #[cfg(feature = "gurobi-solver")]
        assert_eq!(SolverType::from_str("gurobi"), Some(SolverType::Gurobi));
        #[cfg(feature = "gurobi-solver")]
        assert_eq!(SolverType::from_str("Gurobi"), Some(SolverType::Gurobi));
        assert_eq!(SolverType::from_str("unknown"), None);
    }

    #[test]
    fn test_create_glpk_solver() {
        let solver = create_solver(SolverType::Glpk);
        assert_eq!(solver.name(), "GLPK");
    }

    #[cfg(feature = "highs-solver")]
    #[test]
    fn test_create_highs_solver() {
        let solver = create_solver(SolverType::Highs);
        assert_eq!(solver.name(), "HiGHS");
    }

    #[cfg(feature = "gurobi-solver")]
    #[test]
    fn test_create_gurobi_solver() {
        let solver = create_solver(SolverType::Gurobi);
        assert_eq!(solver.name(), "Gurobi");
    }
}
