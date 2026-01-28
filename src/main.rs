mod convert;
mod domain;
mod models;

use models::{ApiSolution, SolveRequest, SolverDirection};

use convert::to_many_borrowed_objectives;
use domain::solve::solve as solve_inner;

use actix_web::body::BoxBody;
use actix_web::http::header::HeaderName;
use actix_web::middleware::{from_fn, Condition, Logger, Next};
use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    Error,
};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};

use dotenv::dotenv;
use std::env;

use sentry_actix::Sentry;
use std::sync::Arc;

// ── Bring in the library types and alias the solver function to avoid name clash
use glpk_rust::Solution;

use crate::convert::to_glpk_polyhedron;

// ---------- Route handlers ----------

/// POST /solve
pub async fn solve(req: web::Json<SolveRequest>) -> impl Responder {
    match validate_solve_request(&req) {
        Ok(_) => (),
        Err(response) => return response,
    }

    let glpk_polyhedron = to_glpk_polyhedron(&req.polyhedron);
    let borrowed_objectives = to_many_borrowed_objectives(&req.objectives);
    let maximize = req.direction == SolverDirection::Maximize;

    // Call the library solver
    let solve_result = solve_inner(glpk_polyhedron, borrowed_objectives, maximize);

    let lib_solutions: Vec<Solution>;
    match solve_result {
        Ok(solutions) => lib_solutions = solutions,
        Err(error) => {
            return HttpResponse::UnprocessableEntity().json(serde_json::json!({
                "error": error.details,
            }))
        }
    }

    // Map library solutions → API solutions with owned Strings
    let api_solutions: Vec<ApiSolution> = lib_solutions.into_iter().map(|s| s.into()).collect();

    HttpResponse::Ok().json(serde_json::json!({ "solutions": api_solutions }))
}

fn validate_solve_request(req: &SolveRequest) -> Result<(), HttpResponse> {
    let variable_count = req.polyhedron.variables.len();
    let column_count = req.polyhedron.a.shape.ncols;
    if variable_count != column_count {
        return Err(HttpResponse::UnprocessableEntity().json(
            serde_json::json!({
                "error": format!("Number of variables must match number of columns in A got {} variables and {} columns", variable_count, column_count)
            }),
        ));
    }

    let b_count = req.polyhedron.b.len();
    let row_count = req.polyhedron.a.shape.nrows;
    if b_count != row_count {
        return Err(HttpResponse::UnprocessableEntity().json(
            serde_json::json!({
                "error": format!("Number of values in b must match number of rows in A got {} values and {} rows", b_count, row_count)
            }),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::StatusCode;
    use std::collections::HashMap;

    use models::{ApiIntegerSparseMatrix, ApiShape, ApiVariable, SparseLEIntegerPolyhedron};

    fn make_valid_request() -> SolveRequest {
        SolveRequest {
            polyhedron: SparseLEIntegerPolyhedron {
                a: ApiIntegerSparseMatrix {
                    rows: vec![0, 1, 2],
                    cols: vec![0, 1, 2],
                    vals: vec![1, 2, 3],
                    shape: ApiShape { nrows: 3, ncols: 3 },
                },
                b: vec![10, 20, 30],
                variables: vec![
                    ApiVariable {
                        id: "x1".into(),
                        bound: (0, 100),
                    },
                    ApiVariable {
                        id: "x2".into(),
                        bound: (0, 100),
                    },
                    ApiVariable {
                        id: "x3".into(),
                        bound: (0, 100),
                    },
                ],
            },
            objectives: vec![{
                let mut obj = HashMap::new();
                obj.insert("x1".to_string(), 1.0);
                obj.insert("x2".to_string(), 2.0);
                obj
            }],
            direction: SolverDirection::Maximize,
        }
    }

    #[test]
    fn validate_solve_request_valid_request() {
        let req = make_valid_request();
        assert!(validate_solve_request(&req).is_ok());
    }

    #[test]
    fn validate_solve_request_mismatch_variables_vs_columns_should_return_422() {
        let mut req = make_valid_request();
        req.polyhedron.variables.pop();
        let resp = validate_solve_request(&req).unwrap_err();
        let status = resp.status();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn validate_solve_request_mismatch_b_vs_rows_should_return_422() {
        let mut req = make_valid_request();
        req.polyhedron.b.pop();
        let resp = validate_solve_request(&req).unwrap_err();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}

/// GET /health
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

/// GET /docs
pub async fn docs() -> impl Responder {
    let docs_html = include_str!("../static/docs.html");
    HttpResponse::Ok().content_type("text/html").body(docs_html)
}

/// GET / - Redirect to docs
pub async fn root_redirect() -> impl Responder {
    HttpResponse::Found()
        .append_header(("Location", "/docs"))
        .finish()
}

// Middleware
static X_API_KEY: HeaderName = HeaderName::from_static("x-api-key");

#[derive(Clone)]
struct AuthConfig {
    token: String,
}

fn unauthorized_error() -> HttpResponse<BoxBody> {
    HttpResponse::Unauthorized()
        .json(serde_json::json!({ "error": "Unauthorized" }))
        .map_into_boxed_body()
}

fn forbidden_error() -> HttpResponse<BoxBody> {
    HttpResponse::Forbidden()
        .json(serde_json::json!({ "error": "Forbidden" }))
        .map_into_boxed_body()
}

fn internal_error() -> HttpResponse<BoxBody> {
    HttpResponse::InternalServerError()
        .json(serde_json::json!({ "error": "Internal server error" }))
        .map_into_boxed_body()
}

async fn token_auth(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let Some(auth) = req.app_data::<web::Data<AuthConfig>>().cloned() else {
        return Ok(req.into_response(internal_error()));
    };

    let Some(raw) = req.headers().get(&X_API_KEY) else {
        return Ok(req.into_response(unauthorized_error()));
    };

    let Ok(token) = raw.to_str() else {
        return Ok(req.into_response(unauthorized_error()));
    };

    let valid_token = auth.token == token;

    if valid_token {
        let res = next.call(req).await?;
        return Ok(res.map_into_boxed_body());
    }

    Ok(req.into_response(forbidden_error()))
}

fn init_sentry() -> sentry::ClientInitGuard {
    let dsn = env::var("SENTRY_DSN").expect("SENTRY_DSN not found");
    let environment = env::var("SENTRY_ENVIRONMENT").expect("SENTRY_ENVIRONMENT not found");
    let service_name = env::var("SENTRY_SERVICE_NAME").expect("SENTRY_SERVICE_NAME not found");

    println!("Initializing Sentry with environment: {}", environment);

    sentry::init((
        dsn,
        sentry::ClientOptions {
            environment: Some(environment.into()),
            attach_stacktrace: true,
            before_send: Some(Arc::new(move |mut event| {
                event.tags.insert("service".into(), service_name.clone());

                // The tag `caas: true` is used to differentiate between
                // caas and non-caas events
                event.tags.insert("caas".into(), "true".into());

                Some(event)
            })),
            ..Default::default()
        },
    ))
}

// ---------- Server bootstrap ----------
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let port = env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(9000);

    let json_limit = env::var("JSON_PAYLOAD_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(2 * 1024 * 1024); // default 2 MB

    let protect = env::var("PROTECT")
        .ok()
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);

    let token = if protect {
        env::var("API_TOKEN").expect("API_TOKEN not available in env")
    } else {
        String::new()
    };

    let use_sentry = env::var("USE_SENTRY")
        .ok()
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);

    // Guard must be kept in scope until the server exits
    let _sentry_guard = if use_sentry {
        Some(init_sentry())
    } else {
        None
    };

    println!(
        "Server is {}",
        if protect { "protected" } else { "unprotected" }
    );
    println!("Starting server on http://127.0.0.1:{}", port);
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Condition::new(use_sentry, Sentry::new()))
            .app_data(
                web::JsonConfig::default()
                    .limit(json_limit)
                    .error_handler(|err, _| {
                        let err_string = err.to_string();
                        actix_web::error::InternalError::from_response(
                            err,
                            HttpResponse::BadRequest()
                                .json(serde_json::json!({ "error": err_string })),
                        )
                        .into()
                    }),
            )
            .app_data(web::Data::new(AuthConfig {
                token: token.clone(),
            }))
            .route("/", web::get().to(root_redirect))
            .route("/health", web::get().to(health_check))
            .route("/docs", web::get().to(docs))
            .service(
                web::scope("")
                    .wrap(Condition::new(protect, from_fn(token_auth)))
                    .route("/solve", web::post().to(solve)),
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
