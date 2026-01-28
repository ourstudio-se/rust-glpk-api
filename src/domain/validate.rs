use std::collections::{HashMap, HashSet};

use glpk_rust::Variable;

pub struct SolveInputError {
    pub details: String,
}

pub fn validate_objectives_owned(
    variables: &Vec<Variable>,
    objectives: &[HashMap<String, f64>],
) -> Result<(), SolveInputError> {
    let variable_ids: HashSet<&str> = variables.iter().map(|v| v.id).collect();

    for objective in objectives {
        for objective_variable_id in objective.keys() {
            if !variable_ids.contains(objective_variable_id.as_str()) {
                return Err(SolveInputError {
                    details: format!(
                        "Objective contains missing variable {}",
                        objective_variable_id,
                    ),
                });
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
        assert!(validate_objectives_owned(&variables, &objectives).is_err());
    }
}
