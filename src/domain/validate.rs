use std::collections::{HashMap, HashSet};

use glpk_rust::{
    Variable
};

pub struct SolveInputError{
    pub details: String,
}

pub fn validate_objectives(
    variables: &Vec<Variable>,
    objectives: &Vec<HashMap<&str, f64>>,
) -> Result<(), SolveInputError> {
    let variable_ids: HashSet<&str> = variables
        .iter()
        .map(|v| v.id)
        .collect();


    for objective in objectives {
        for (objective_variable, _) in objective {
            if !variable_ids.contains(objective_variable) {
                return Err(
                    SolveInputError{ 
                        details: format!(
                            "Objective contains missing variable {}", 
                            objective_variable,
                        ),
                    },
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_objectives_given_valid_objectives_should_return_ok() {
        let variables = vec![
            Variable { id: "x1", bound: (0, 1) },
            Variable { id: "x2", bound: (0, 1) },
        ];
        let objectives = vec![
            HashMap::from([("x1", 1.0), ("x2", 2.0)]),
        ];
        assert!(
            validate_objectives(&variables, &objectives).is_ok()
        );
    }

    #[test]
    fn test_validate_objectives_given_missing_variable_should_return_error() {
        let variables = vec![
            Variable { id: "x1", bound: (0, 1) },
            Variable { id: "x2", bound: (0, 1) },
        ];
        let objectives = vec![
            HashMap::from([("x1", 1.0), ("missing", 2.0)]),
        ];
        assert!(
            validate_objectives(&variables, &objectives).is_err()
        );
    }
}
