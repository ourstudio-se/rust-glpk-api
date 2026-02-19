#ifndef HEXALY_WRAPPER_H
#define HEXALY_WRAPPER_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque types for our C API wrapper
typedef void* HxOptimizerWrapper;
typedef void* HxModelWrapper;
typedef void* HxExprWrapper;
typedef void* HxParamWrapper;
typedef void* HxSolutionWrapper;

// Status enum matching Hexaly 14.5 HxState
typedef enum {
    HXW_STATE_STOPPED = 0,
    HXW_STATE_RUNNING = 1,
    HXW_STATE_PAUSED = 2
} HxStateWrapper;

// Solution status enum matching HxSolutionStatus
typedef enum {
    HXW_SOLUTION_NO_SOLUTION = 0,
    HXW_SOLUTION_INCONSISTENT = 1,
    HXW_SOLUTION_INFEASIBLE = 2,
    HXW_SOLUTION_FEASIBLE = 3,
    HXW_SOLUTION_OPTIMAL = 4
} HxSolutionStatusWrapper;

// Create and destroy optimizer
HxOptimizerWrapper hxw_create_optimizer(void);
void hxw_delete_optimizer(HxOptimizerWrapper optimizer);

// Get model from optimizer
HxModelWrapper hxw_get_model(HxOptimizerWrapper optimizer);

// Get param from optimizer
HxParamWrapper hxw_get_param(HxOptimizerWrapper optimizer);

// Get solution from optimizer
HxSolutionWrapper hxw_get_solution(HxOptimizerWrapper optimizer);

// Model operations
void hxw_model_close(HxModelWrapper model);

// Create expressions
HxExprWrapper hxw_model_int(HxModelWrapper model, int64_t lower_bound, int64_t upper_bound);
HxExprWrapper hxw_model_sum(HxModelWrapper model);
HxExprWrapper hxw_model_prod(HxModelWrapper model);
HxExprWrapper hxw_model_scalar(HxModelWrapper model, int64_t value);

// Expression operations
void hxw_expr_add_operand(HxExprWrapper expr, HxExprWrapper operand);

// Operators for building expressions
HxExprWrapper hxw_expr_leq(HxModelWrapper model, HxExprWrapper left, HxExprWrapper right);
HxExprWrapper hxw_expr_eq(HxModelWrapper model, HxExprWrapper left, HxExprWrapper right);
HxExprWrapper hxw_expr_geq(HxModelWrapper model, HxExprWrapper left, HxExprWrapper right);

// Constraint and objective
void hxw_model_add_constraint(HxModelWrapper model, HxExprWrapper expr);
void hxw_model_minimize(HxModelWrapper model, HxExprWrapper expr);
void hxw_model_maximize(HxModelWrapper model, HxExprWrapper expr);

// Parameters
void hxw_param_set_verbosity(HxParamWrapper param, int32_t verbosity);
void hxw_param_set_time_limit(HxParamWrapper param, int32_t seconds);
void hxw_param_set_nb_threads(HxParamWrapper param, int32_t nb_threads);

// Solve
void hxw_solve(HxOptimizerWrapper optimizer);
HxStateWrapper hxw_get_state(HxOptimizerWrapper optimizer);

// Solution operations
HxSolutionStatusWrapper hxw_solution_get_status(HxSolutionWrapper solution);
int64_t hxw_solution_get_int_value(HxSolutionWrapper solution, HxExprWrapper expr);
double hxw_solution_get_double_value(HxSolutionWrapper solution, HxExprWrapper expr);

#ifdef __cplusplus
}
#endif

#endif // HEXALY_WRAPPER_H
