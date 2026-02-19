#include "hexaly_wrapper.h"
#include <optimizer/hexalyoptimizer.h>

using namespace hexaly;

// Create and destroy optimizer
HxOptimizerWrapper hxw_create_optimizer() {
    try {
        HexalyOptimizer* optimizer = new HexalyOptimizer();
        return static_cast<HxOptimizerWrapper>(optimizer);
    } catch (...) {
        return nullptr;
    }
}

void hxw_delete_optimizer(HxOptimizerWrapper optimizer) {
    if (optimizer) {
        HexalyOptimizer* opt = static_cast<HexalyOptimizer*>(optimizer);
        delete opt;
    }
}

// Get model from optimizer
HxModelWrapper hxw_get_model(HxOptimizerWrapper optimizer) {
    if (!optimizer) return nullptr;
    try {
        HexalyOptimizer* opt = static_cast<HexalyOptimizer*>(optimizer);
        HxModel* model = new HxModel(opt->getModel());
        return static_cast<HxModelWrapper>(model);
    } catch (...) {
        return nullptr;
    }
}

// Get param from optimizer
HxParamWrapper hxw_get_param(HxOptimizerWrapper optimizer) {
    if (!optimizer) return nullptr;
    try {
        HexalyOptimizer* opt = static_cast<HexalyOptimizer*>(optimizer);
        HxParam* param = new HxParam(opt->getParam());
        return static_cast<HxParamWrapper>(param);
    } catch (...) {
        return nullptr;
    }
}

// Get solution from optimizer
HxSolutionWrapper hxw_get_solution(HxOptimizerWrapper optimizer) {
    if (!optimizer) return nullptr;
    try {
        HexalyOptimizer* opt = static_cast<HexalyOptimizer*>(optimizer);
        HxSolution* solution = new HxSolution(opt->getSolution());
        return static_cast<HxSolutionWrapper>(solution);
    } catch (...) {
        return nullptr;
    }
}

// Model operations
void hxw_model_close(HxModelWrapper model) {
    if (!model) return;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        m->close();
    } catch (...) {
        // Ignore errors
    }
}

// Create expressions
HxExprWrapper hxw_model_int(HxModelWrapper model, int64_t lower_bound, int64_t upper_bound) {
    if (!model) return nullptr;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* expr = new HxExpression(m->intVar(lower_bound, upper_bound));
        return static_cast<HxExprWrapper>(expr);
    } catch (...) {
        return nullptr;
    }
}

HxExprWrapper hxw_model_sum(HxModelWrapper model) {
    if (!model) return nullptr;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* expr = new HxExpression(m->sum());
        return static_cast<HxExprWrapper>(expr);
    } catch (...) {
        return nullptr;
    }
}

HxExprWrapper hxw_model_prod(HxModelWrapper model) {
    if (!model) return nullptr;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* expr = new HxExpression(m->prod());
        return static_cast<HxExprWrapper>(expr);
    } catch (...) {
        return nullptr;
    }
}

HxExprWrapper hxw_model_scalar(HxModelWrapper model, int64_t value) {
    if (!model) return nullptr;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* expr = new HxExpression(m->createConstant(value));
        return static_cast<HxExprWrapper>(expr);
    } catch (...) {
        return nullptr;
    }
}

// Expression operations
void hxw_expr_add_operand(HxExprWrapper expr, HxExprWrapper operand) {
    if (!expr || !operand) return;
    try {
        HxExpression* e = static_cast<HxExpression*>(expr);
        HxExpression* op = static_cast<HxExpression*>(operand);
        e->addOperand(*op);
    } catch (...) {
        // Ignore errors
    }
}

// Operators for building expressions
HxExprWrapper hxw_expr_leq(HxModelWrapper model, HxExprWrapper left, HxExprWrapper right) {
    if (!model || !left || !right) return nullptr;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* l = static_cast<HxExpression*>(left);
        HxExpression* r = static_cast<HxExpression*>(right);
        HxExpression* result = new HxExpression(m->leq(*l, *r));
        return static_cast<HxExprWrapper>(result);
    } catch (...) {
        return nullptr;
    }
}

HxExprWrapper hxw_expr_eq(HxModelWrapper model, HxExprWrapper left, HxExprWrapper right) {
    if (!model || !left || !right) return nullptr;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* l = static_cast<HxExpression*>(left);
        HxExpression* r = static_cast<HxExpression*>(right);
        HxExpression* result = new HxExpression(m->eq(*l, *r));
        return static_cast<HxExprWrapper>(result);
    } catch (...) {
        return nullptr;
    }
}

HxExprWrapper hxw_expr_geq(HxModelWrapper model, HxExprWrapper left, HxExprWrapper right) {
    if (!model || !left || !right) return nullptr;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* l = static_cast<HxExpression*>(left);
        HxExpression* r = static_cast<HxExpression*>(right);
        HxExpression* result = new HxExpression(m->geq(*l, *r));
        return static_cast<HxExprWrapper>(result);
    } catch (...) {
        return nullptr;
    }
}

// Constraint and objective
void hxw_model_add_constraint(HxModelWrapper model, HxExprWrapper expr) {
    if (!model || !expr) return;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* e = static_cast<HxExpression*>(expr);
        m->constraint(*e);
    } catch (...) {
        // Ignore errors
    }
}

void hxw_model_minimize(HxModelWrapper model, HxExprWrapper expr) {
    if (!model || !expr) return;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* e = static_cast<HxExpression*>(expr);
        m->minimize(*e);
    } catch (...) {
        // Ignore errors
    }
}

void hxw_model_maximize(HxModelWrapper model, HxExprWrapper expr) {
    if (!model || !expr) return;
    try {
        HxModel* m = static_cast<HxModel*>(model);
        HxExpression* e = static_cast<HxExpression*>(expr);
        m->maximize(*e);
    } catch (...) {
        // Ignore errors
    }
}

// Parameters
void hxw_param_set_verbosity(HxParamWrapper param, int32_t verbosity) {
    if (!param) return;
    try {
        HxParam* p = static_cast<HxParam*>(param);
        p->setVerbosity(verbosity);
    } catch (...) {
        // Ignore errors
    }
}

void hxw_param_set_time_limit(HxParamWrapper param, int32_t seconds) {
    if (!param) return;
    try {
        HxParam* p = static_cast<HxParam*>(param);
        p->setTimeLimit(seconds);
    } catch (...) {
        // Ignore errors
    }
}

void hxw_param_set_nb_threads(HxParamWrapper param, int32_t nb_threads) {
    if (!param) return;
    try {
        HxParam* p = static_cast<HxParam*>(param);
        p->setNbThreads(nb_threads);
    } catch (...) {
        // Ignore errors
    }
}

// Solve
void hxw_solve(HxOptimizerWrapper optimizer) {
    if (!optimizer) return;
    try {
        HexalyOptimizer* opt = static_cast<HexalyOptimizer*>(optimizer);
        opt->solve();
    } catch (...) {
        // Ignore errors
    }
}

HxStateWrapper hxw_get_state(HxOptimizerWrapper optimizer) {
    if (!optimizer) return HXW_STATE_STOPPED;
    try {
        HexalyOptimizer* opt = static_cast<HexalyOptimizer*>(optimizer);
        HxState state = opt->getState();

        // Map Hexaly states to our enum
        switch (state) {
            case HxState::S_Stopped: return HXW_STATE_STOPPED;
            case HxState::S_Running: return HXW_STATE_RUNNING;
            case HxState::S_Paused: return HXW_STATE_PAUSED;
            default: return HXW_STATE_STOPPED;
        }
    } catch (...) {
        return HXW_STATE_STOPPED;
    }
}

// Solution operations
HxSolutionStatusWrapper hxw_solution_get_status(HxSolutionWrapper solution) {
    if (!solution) return HXW_SOLUTION_NO_SOLUTION;
    try {
        HxSolution* sol = static_cast<HxSolution*>(solution);
        HxSolutionStatus status = sol->getStatus();

        // Map solution status
        switch (status) {
            case HxSolutionStatus::SS_Inconsistent: return HXW_SOLUTION_INCONSISTENT;
            case HxSolutionStatus::SS_Infeasible: return HXW_SOLUTION_INFEASIBLE;
            case HxSolutionStatus::SS_Feasible: return HXW_SOLUTION_FEASIBLE;
            case HxSolutionStatus::SS_Optimal: return HXW_SOLUTION_OPTIMAL;
            default: return HXW_SOLUTION_NO_SOLUTION;
        }
    } catch (...) {
        return HXW_SOLUTION_NO_SOLUTION;
    }
}

int64_t hxw_solution_get_int_value(HxSolutionWrapper solution, HxExprWrapper expr) {
    if (!solution || !expr) return 0;
    try {
        HxSolution* sol = static_cast<HxSolution*>(solution);
        HxExpression* e = static_cast<HxExpression*>(expr);
        return sol->getIntValue(*e);
    } catch (...) {
        return 0;
    }
}

double hxw_solution_get_double_value(HxSolutionWrapper solution, HxExprWrapper expr) {
    if (!solution || !expr) return 0.0;
    try {
        HxSolution* sol = static_cast<HxSolution*>(solution);
        HxExpression* e = static_cast<HxExpression*>(expr);
        return sol->getDoubleValue(*e);
    } catch (...) {
        return 0.0;
    }
}
