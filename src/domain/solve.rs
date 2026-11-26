use crate::convert::to_glpk_polyhedron;
use crate::domain::outcome::SolveOutcome;
use crate::domain::solver::{SolveError, Solver};
use crate::domain::validate::validate_objectives_owned;
use crate::models::{ApiSolution, SolveRequest};

/// Single entry point for solving a request, regardless of solver.
///
/// 1. Validates the objectives against the variables.
/// 2. Lets the solver produce one `SolveOutcome` per objective.
/// 3. Converts each outcome into the API contract (see `domain::outcome`).
pub fn solve_request(
    solver: &dyn Solver,
    request: SolveRequest,
    use_presolve: bool,
) -> Result<Vec<ApiSolution>, SolveError> {
    let SolveRequest {
        polyhedron,
        objectives,
        direction,
    } = request;

    let glpk_polyhedron = to_glpk_polyhedron(&polyhedron);
    validate_objectives_owned(&glpk_polyhedron.variables, &objectives)?;

    let outcomes = solver.solve(&polyhedron, &objectives, direction, use_presolve)?;

    if outcomes.len() != objectives.len() {
        return Err(SolveError::unknown(format!(
            "{} returned {} results for {} objectives",
            solver.name(),
            outcomes.len(),
            objectives.len()
        )));
    }

    Ok(outcomes
        .into_iter()
        .map(SolveOutcome::into_api_solution)
        .collect())
}
