use crate::error::{GlpkError, Result};
use crate::types::{
    IntegerSparseMatrix, Objective, Shape, SolveRequest, SolverDirection,
    SparseLEIntegerPolyhedron, Variable,
};

/// Builder for constructing solve requests with a fluent API
#[derive(Debug, Default)]
pub struct SolveRequestBuilder {
    variables: Vec<Variable>,
    constraint_rows: Vec<i32>,
    constraint_cols: Vec<i32>,
    constraint_vals: Vec<i32>,
    b: Vec<i32>,
    objectives: Vec<Objective>,
    direction: Option<SolverDirection>,
}

impl SolveRequestBuilder {
    /// Create a new solve request builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a decision variable
    ///
    /// # Example
    ///
    /// ```
    /// use glpk_api_sdk::{SolveRequestBuilder, Variable};
    ///
    /// let builder = SolveRequestBuilder::new()
    ///     .add_variable(Variable::new("x1", 0, 100));
    /// ```
    pub fn add_variable(mut self, variable: Variable) -> Self {
        self.variables.push(variable);
        self
    }

    /// Add multiple decision variables
    ///
    /// # Example
    ///
    /// ```
    /// use glpk_api_sdk::{SolveRequestBuilder, Variable};
    ///
    /// let builder = SolveRequestBuilder::new()
    ///     .add_variables(vec![
    ///         Variable::new("x1", 0, 100),
    ///         Variable::new("x2", 0, 100),
    ///     ]);
    /// ```
    pub fn add_variables(mut self, variables: Vec<Variable>) -> Self {
        self.variables.extend(variables);
        self
    }

    /// Add a constraint row to the constraint matrix A
    ///
    /// The constraint is of the form: sum(A[row, col] * x[col]) ≤ b
    ///
    /// # Arguments
    ///
    /// * `rows` - Row indices for non-zero elements (all same value for one constraint)
    /// * `cols` - Column indices for non-zero elements (which variables)
    /// * `vals` - Values of non-zero elements (coefficients)
    /// * `b_value` - Right-hand side value for this constraint
    ///
    /// # Example
    ///
    /// ```
    /// use glpk_api_sdk::SolveRequestBuilder;
    ///
    /// // Add constraint: x0 + x1 ≤ 1
    /// let builder = SolveRequestBuilder::new()
    ///     .add_constraint(vec![0, 0], vec![0, 1], vec![1, 1], 1);
    /// ```
    pub fn add_constraint(
        mut self,
        rows: Vec<i32>,
        cols: Vec<i32>,
        vals: Vec<i32>,
        b_value: i32,
    ) -> Self {
        self.constraint_rows.extend(rows);
        self.constraint_cols.extend(cols);
        self.constraint_vals.extend(vals);
        self.b.push(b_value);
        self
    }

    /// Set the constraint matrix A in one go
    ///
    /// This sets all the sparse matrix data at once, replacing any previously added constraints.
    ///
    /// # Arguments
    ///
    /// * `rows` - All row indices for non-zero elements
    /// * `cols` - All column indices for non-zero elements
    /// * `vals` - All values at the specified positions
    ///
    /// # Example
    ///
    /// ```
    /// use glpk_api_sdk::SolveRequestBuilder;
    ///
    /// // Set matrix with two constraints:
    /// // Row 0: x0 + x1 ≤ b[0]
    /// // Row 1: 2*x0 + 3*x1 ≤ b[1]
    /// let builder = SolveRequestBuilder::new()
    ///     .set_constraint_matrix(
    ///         vec![0, 0, 1, 1],
    ///         vec![0, 1, 0, 1],
    ///         vec![1, 1, 2, 3]
    ///     );
    /// ```
    pub fn set_constraint_matrix(mut self, rows: Vec<i32>, cols: Vec<i32>, vals: Vec<i32>) -> Self {
        self.constraint_rows = rows;
        self.constraint_cols = cols;
        self.constraint_vals = vals;
        self
    }

    /// Set all right-hand side constraint values (b vector) in one go
    ///
    /// This replaces any previously set b values.
    ///
    /// # Arguments
    ///
    /// * `b` - Vector of right-hand side values for constraints
    ///
    /// # Example
    ///
    /// ```
    /// use glpk_api_sdk::SolveRequestBuilder;
    ///
    /// let builder = SolveRequestBuilder::new()
    ///     .set_constraint_rhs(vec![10, 20, 30]);
    /// ```
    pub fn set_constraint_rhs(mut self, b: Vec<i32>) -> Self {
        self.b = b;
        self
    }

    /// Add an objective function to optimize
    ///
    /// Multiple objectives can be added, and each will be solved independently.
    ///
    /// # Example
    ///
    /// ```
    /// use glpk_api_sdk::SolveRequestBuilder;
    /// use std::collections::HashMap;
    ///
    /// let mut objective = HashMap::new();
    /// objective.insert("x1".to_string(), 1.0);
    /// objective.insert("x2".to_string(), 2.0);
    ///
    /// let builder = SolveRequestBuilder::new()
    ///     .add_objective(objective);
    /// ```
    pub fn add_objective(mut self, objective: Objective) -> Self {
        self.objectives.push(objective);
        self
    }

    /// Add multiple objective functions
    pub fn add_objectives(mut self, objectives: Vec<Objective>) -> Self {
        self.objectives.extend(objectives);
        self
    }

    /// Set the optimization direction
    ///
    /// # Example
    ///
    /// ```
    /// use glpk_api_sdk::{SolveRequestBuilder, SolverDirection};
    ///
    /// let builder = SolveRequestBuilder::new()
    ///     .direction(SolverDirection::Maximize);
    /// ```
    pub fn direction(mut self, direction: SolverDirection) -> Self {
        self.direction = Some(direction);
        self
    }

    /// Build the solve request
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No variables have been added
    /// - No objectives have been added
    /// - No direction has been set
    /// - The constraint matrix dimensions don't match
    pub fn build(self) -> Result<SolveRequest> {
        if self.variables.is_empty() {
            return Err(GlpkError::InvalidRequest(
                "At least one variable is required".to_string(),
            ));
        }

        if self.objectives.is_empty() {
            return Err(GlpkError::InvalidRequest(
                "At least one objective is required".to_string(),
            ));
        }

        let direction = self.direction.ok_or_else(|| {
            GlpkError::InvalidRequest("Direction (maximize/minimize) must be set".to_string())
        })?;

        let nrows = self.b.len();
        let ncols = self.variables.len();

        // Validate constraint matrix dimensions
        if self.constraint_rows.len() != self.constraint_cols.len()
            || self.constraint_rows.len() != self.constraint_vals.len()
        {
            return Err(GlpkError::InvalidRequest(
                "Constraint matrix rows, cols, and vals must have the same length".to_string(),
            ));
        }

        let matrix = IntegerSparseMatrix {
            rows: self.constraint_rows,
            cols: self.constraint_cols,
            vals: self.constraint_vals,
            shape: Shape { nrows, ncols },
        };

        let polyhedron = SparseLEIntegerPolyhedron {
            a: matrix,
            b: self.b,
            variables: self.variables,
        };

        Ok(SolveRequest {
            polyhedron,
            objectives: self.objectives,
            direction,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_valid_request() {
        let result = SolveRequestBuilder::new()
            .add_variable(Variable::new("x1", 0, 100))
            .add_variable(Variable::new("x2", 0, 100))
            .add_constraint(vec![0, 0], vec![0, 1], vec![1, 2], 10)
            .add_objective([("x1".to_string(), 1.0), ("x2".to_string(), 2.0)].into())
            .direction(SolverDirection::Maximize)
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_no_variables() {
        let result = SolveRequestBuilder::new()
            .add_objective([("x1".to_string(), 1.0)].into())
            .direction(SolverDirection::Maximize)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_no_objectives() {
        let result = SolveRequestBuilder::new()
            .add_variable(Variable::new("x1", 0, 100))
            .direction(SolverDirection::Maximize)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_no_direction() {
        let result = SolveRequestBuilder::new()
            .add_variable(Variable::new("x1", 0, 100))
            .add_objective([("x1".to_string(), 1.0)].into())
            .build();

        assert!(result.is_err());
    }
}
