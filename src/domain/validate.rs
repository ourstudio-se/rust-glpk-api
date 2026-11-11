use std::collections::{HashMap, HashSet};

use glpk_rust::Variable;

pub enum SolveErrorType {
    InputError,
    TimeLimitExceeded,
    Unknown,
}

pub struct SolveError {
    pub error_type: SolveErrorType,
    pub details: String,
}

impl SolveError {
    pub fn input(details: impl Into<String>) -> Self {
        Self {
            error_type: SolveErrorType::InputError,
            details: details.into(),
        }
    }

    pub fn time_limit_exceeded() -> Self {
        Self {
            error_type: SolveErrorType::TimeLimitExceeded,
            details: "solver timed out".into(),
        }
    }

    pub fn unknown(details: impl Into<String>) -> Self {
        Self {
            error_type: SolveErrorType::Unknown,
            details: details.into(),
        }
    }
}

impl From<glpk_rust::SolverError> for SolveError {
    fn from(e: glpk_rust::SolverError) -> Self {
        match e {
            glpk_rust::SolverError::TimeLimitExceeded => SolveError::time_limit_exceeded(),
            other => SolveError::input(format!("GLPK solver error: {}", other)),
        }
    }
}

pub fn validate_objectives_owned(
    variables: &Vec<Variable>,
    objectives: &[HashMap<String, f64>],
) -> Result<(), SolveError> {
    let variable_ids: HashSet<&str> = variables.iter().map(|v| v.id).collect();

    for objective in objectives {
        for objective_variable_id in objective.keys() {
            if !variable_ids.contains(objective_variable_id.as_str()) {
                return Err(SolveError::input(format!(
                    "Objective contains missing variable {}",
                    objective_variable_id,
                )));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_objectives_given_valid_objectives() {
        let variables = vec![
            Variable {
                id: "x1",
                bound: (0, 1),
            },
            Variable {
                id: "x2",
                bound: (0, 1),
            },
        ];
        let objectives = vec![HashMap::from([
            ("x1".to_string(), 1.0),
            ("x2".to_string(), 2.0),
        ])];
        assert!(validate_objectives_owned(&variables, &objectives).is_ok());
    }

    #[test]
    fn glpk_time_limit_maps_to_solve_error_time_limit() {
        let err: SolveError = glpk_rust::SolverError::TimeLimitExceeded.into();
        assert!(matches!(err.error_type, SolveErrorType::TimeLimitExceeded));
    }

    #[test]
    fn other_glpk_error_maps_to_solve_error_input() {
        let err: SolveError = glpk_rust::SolverError::EmptyConstraintMatrix.into();
        assert!(matches!(err.error_type, SolveErrorType::InputError));
    }

    #[test]
    fn test_validate_objectives_given_missing_variable() {
        let variables = vec![
            Variable {
                id: "x1",
                bound: (0, 1),
            },
            Variable {
                id: "x2",
                bound: (0, 1),
            },
        ];
        let objectives = vec![HashMap::from([
            ("x1".to_string(), 1.0),
            ("missing".to_string(), 2.0),
        ])];
        let err = validate_objectives_owned(&variables, &objectives).unwrap_err();
        assert!(matches!(err.error_type, SolveErrorType::InputError));
    }
}
