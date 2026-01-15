use crate::domain::solver::Solver;
use crate::domain::solvers::GlpkSolver;

#[cfg(feature = "highs-solver")]
use crate::domain::solvers::HighsSolver;

/// Available solver backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverType {
    Glpk,
    #[cfg(feature = "highs-solver")]
    Highs,
}

impl SolverType {
    /// Parse solver type from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "glpk" => Some(SolverType::Glpk),
            #[cfg(feature = "highs-solver")]
            "highs" => Some(SolverType::Highs),
            _ => None,
        }
    }
}

/// Create a solver instance based on the specified type
pub fn create_solver(solver_type: SolverType) -> Box<dyn Solver> {
    match solver_type {
        SolverType::Glpk => Box::new(GlpkSolver::new()),
        #[cfg(feature = "highs-solver")]
        SolverType::Highs => Box::new(HighsSolver::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solver_type_from_str() {
        assert_eq!(SolverType::from_str("glpk"), Some(SolverType::Glpk));
        assert_eq!(SolverType::from_str("GLPK"), Some(SolverType::Glpk));
        #[cfg(feature = "highs-solver")]
        assert_eq!(SolverType::from_str("highs"), Some(SolverType::Highs));
        #[cfg(feature = "highs-solver")]
        assert_eq!(SolverType::from_str("HiGHS"), Some(SolverType::Highs));
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
}
