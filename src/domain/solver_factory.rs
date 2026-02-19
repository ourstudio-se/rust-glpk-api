use crate::domain::solver::Solver;
use crate::domain::solvers::GlpkSolver;

#[cfg(feature = "highs-solver")]
use crate::domain::solvers::HighsSolver;

#[cfg(feature = "gurobi-solver")]
use crate::domain::solvers::GurobiSolver;

#[cfg(feature = "hexaly-solver")]
use crate::domain::solvers::HexalySolver;

/// Available solver backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverType {
    Glpk,
    #[cfg(feature = "highs-solver")]
    Highs,
    #[cfg(feature = "gurobi-solver")]
    Gurobi,
    #[cfg(feature = "hexaly-solver")]
    Hexaly,
}

impl SolverType {
    /// Parse solver type from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "glpk" => Some(SolverType::Glpk),
            #[cfg(feature = "highs-solver")]
            "highs" => Some(SolverType::Highs),
            #[cfg(feature = "gurobi-solver")]
            "gurobi" => Some(SolverType::Gurobi),
            #[cfg(feature = "hexaly-solver")]
            "hexaly" => Some(SolverType::Hexaly),
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
        #[cfg(feature = "gurobi-solver")]
        SolverType::Gurobi => Box::new(GurobiSolver::new()),
        #[cfg(feature = "hexaly-solver")]
        SolverType::Hexaly => Box::new(HexalySolver::new()),
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
        #[cfg(feature = "gurobi-solver")]
        assert_eq!(SolverType::from_str("gurobi"), Some(SolverType::Gurobi));
        #[cfg(feature = "gurobi-solver")]
        assert_eq!(SolverType::from_str("Gurobi"), Some(SolverType::Gurobi));
        #[cfg(feature = "hexaly-solver")]
        assert_eq!(SolverType::from_str("hexaly"), Some(SolverType::Hexaly));
        #[cfg(feature = "hexaly-solver")]
        assert_eq!(SolverType::from_str("Hexaly"), Some(SolverType::Hexaly));
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

    #[cfg(feature = "hexaly-solver")]
    #[test]
    fn test_create_hexaly_solver() {
        let solver = create_solver(SolverType::Hexaly);
        assert_eq!(solver.name(), "Hexaly");
    }
}
