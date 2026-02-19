//! Safe Rust bindings for Hexaly (HexalyOptimizer) optimization library
//!
//! This crate provides a safe, idiomatic Rust interface to Hexaly.

use hexaly_sys::*;
use std::ptr;

/// Hexaly solver state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Stopped,
    Running,
    Paused,
}

impl From<HxStateWrapper> for State {
    fn from(state: HxStateWrapper) -> Self {
        match state {
            HxStateWrapper_HXW_STATE_STOPPED => State::Stopped,
            HxStateWrapper_HXW_STATE_RUNNING => State::Running,
            HxStateWrapper_HXW_STATE_PAUSED => State::Paused,
            _ => State::Stopped,
        }
    }
}

/// A Hexaly expression
#[derive(Clone)]
pub struct Expression {
    ptr: HxExprWrapper,
    // We don't own the expression - it's managed by the model
    _marker: std::marker::PhantomData<*mut ()>,
}

impl Expression {
    fn new(ptr: HxExprWrapper) -> Self {
        Expression {
            ptr,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn as_ptr(&self) -> HxExprWrapper {
        self.ptr
    }
}

/// Hexaly model for building optimization problems
pub struct Model {
    ptr: HxModelWrapper,
}

impl Model {
    fn new(ptr: HxModelWrapper) -> Self {
        Model { ptr }
    }

    /// Create an integer decision variable with bounds
    pub fn int_var(&self, lower_bound: i64, upper_bound: i64) -> Expression {
        unsafe {
            let expr = hxw_model_int(self.ptr, lower_bound, upper_bound);
            Expression::new(expr)
        }
    }

    /// Create a constant scalar value (NOT a decision variable)
    pub fn scalar(&self, value: i64) -> Expression {
        unsafe {
            let expr = hxw_model_scalar(self.ptr, value);
            Expression::new(expr)
        }
    }

    /// Create a sum expression
    pub fn sum(&self) -> Expression {
        unsafe {
            let expr = hxw_model_sum(self.ptr);
            Expression::new(expr)
        }
    }

    /// Create a product expression
    pub fn prod(&self) -> Expression {
        unsafe {
            let expr = hxw_model_prod(self.ptr);
            Expression::new(expr)
        }
    }

    /// Add an operand to an expression (for sum, prod, etc.)
    pub fn add_operand(&self, expr: &Expression, operand: &Expression) {
        unsafe {
            hxw_expr_add_operand(expr.ptr, operand.ptr);
        }
    }

    /// Create a less-than-or-equal constraint
    pub fn leq(&self, left: &Expression, right: &Expression) -> Expression {
        unsafe {
            let expr = hxw_expr_leq(self.ptr, left.ptr, right.ptr);
            Expression::new(expr)
        }
    }

    /// Create an equality constraint
    pub fn eq(&self, left: &Expression, right: &Expression) -> Expression {
        unsafe {
            let expr = hxw_expr_eq(self.ptr, left.ptr, right.ptr);
            Expression::new(expr)
        }
    }

    /// Create a greater-than-or-equal constraint
    pub fn geq(&self, left: &Expression, right: &Expression) -> Expression {
        unsafe {
            let expr = hxw_expr_geq(self.ptr, left.ptr, right.ptr);
            Expression::new(expr)
        }
    }

    /// Add a constraint to the model
    pub fn add_constraint(&self, constraint: Expression) {
        unsafe {
            hxw_model_add_constraint(self.ptr, constraint.ptr);
        }
    }

    /// Set the objective to minimize
    pub fn minimize(&self, objective: Expression) {
        unsafe {
            hxw_model_minimize(self.ptr, objective.ptr);
        }
    }

    /// Set the objective to maximize
    pub fn maximize(&self, objective: Expression) {
        unsafe {
            hxw_model_maximize(self.ptr, objective.ptr);
        }
    }

    /// Close the model (must be called before solve)
    pub fn close(&self) {
        unsafe {
            hxw_model_close(self.ptr);
        }
    }
}

/// Hexaly parameters for configuring the solver
pub struct Param {
    ptr: HxParamWrapper,
}

impl Param {
    fn new(ptr: HxParamWrapper) -> Self {
        Param { ptr }
    }

    /// Set verbosity level (0 = quiet, 1 = normal, 2+ = verbose)
    pub fn set_verbosity(&self, verbosity: i32) {
        unsafe {
            hxw_param_set_verbosity(self.ptr, verbosity);
        }
    }

    /// Set time limit in seconds
    pub fn set_time_limit(&self, seconds: i32) {
        unsafe {
            hxw_param_set_time_limit(self.ptr, seconds);
        }
    }

    /// Set number of threads to use
    pub fn set_nb_threads(&self, nb_threads: i32) {
        unsafe {
            hxw_param_set_nb_threads(self.ptr, nb_threads);
        }
    }
}

/// Main Hexaly environment
pub struct HexalyOptimizer {
    env: HxOptimizerWrapper,
}

impl HexalyOptimizer {
    /// Create a new Hexaly environment
    pub fn new() -> Result<Self, String> {
        unsafe {
            let env = hxw_create_optimizer();
            if env.is_null() {
                return Err("Failed to create Hexaly environment".to_string());
            }
            Ok(HexalyOptimizer { env })
        }
    }

    /// Get the model builder
    pub fn model(&self) -> Model {
        unsafe {
            let ptr = hxw_get_model(self.env);
            Model::new(ptr)
        }
    }

    /// Get the parameter configuration
    pub fn param(&self) -> Param {
        unsafe {
            let ptr = hxw_get_param(self.env);
            Param::new(ptr)
        }
    }

    /// Solve the optimization problem
    pub fn solve(&self) {
        unsafe {
            hxw_solve(self.env);
        }
    }

    /// Get the solver state after solving
    pub fn state(&self) -> State {
        unsafe {
            let state = hxw_get_state(self.env);
            State::from(state)
        }
    }

    /// Get the integer value of an expression from the solution
    pub fn get_int_value(&self, expr: &Expression) -> i64 {
        unsafe {
            let solution = hxw_get_solution(self.env);
            hxw_solution_get_int_value(solution, expr.ptr)
        }
    }

    /// Get the double value of an expression from the solution
    pub fn get_double_value(&self, expr: &Expression) -> f64 {
        unsafe {
            let solution = hxw_get_solution(self.env);
            hxw_solution_get_double_value(solution, expr.ptr)
        }
    }
}

impl Drop for HexalyOptimizer {
    fn drop(&mut self) {
        unsafe {
            if !self.env.is_null() {
                hxw_delete_optimizer(self.env);
            }
        }
    }
}

impl Default for HexalyOptimizer {
    fn default() -> Self {
        Self::new().expect("Failed to create HexalyOptimizer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_solver() {
        let solver = HexalyOptimizer::new();
        assert!(solver.is_ok());
    }

    #[test]
    fn test_simple_problem() {
        // Simple problem: maximize x + y subject to x + y <= 10, x, y in [0, 10]
        let ls = HexalyOptimizer::new().unwrap();
        let model = ls.model();

        let x = model.int_var(0, 10);
        let y = model.int_var(0, 10);

        // Create sum x + y
        let sum_xy = model.sum();
        model.add_operand(&sum_xy, &x);
        model.add_operand(&sum_xy, &y);

        // Add constraint: x + y <= 10
        let ten = model.int_var(10, 10);
        let constraint = model.leq(&sum_xy, &ten);
        model.add_constraint(constraint);

        // Objective: maximize x + y
        let objective = model.sum();
        model.add_operand(&objective, &x);
        model.add_operand(&objective, &y);
        model.maximize(objective);

        model.close();

        let param = ls.param();
        param.set_verbosity(0);

        ls.solve();

        let state = ls.state();
        assert_eq!(state, State::Stopped);

        let x_val = ls.get_int_value(&x);
        let y_val = ls.get_int_value(&y);

        // Solution should sum to 10
        assert_eq!(x_val + y_val, 10);
    }
}
