use actix_web::body::BoxBody;
use actix_web::http::header::HeaderName;
use actix_web::middleware::{from_fn, Condition, Logger, Next};
use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    Error,
};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};

use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

// ── Bring in the library types and alias the solver function to avoid name clash
use glpk_rust::{
    solve_ilps as glpk_solve_ilps, Bound, IntegerSparseMatrix as GlpkMatrix,
    SparseLEIntegerPolyhedron as GlpkPoly, Status as GlpkStatus, Variable as GlpkVar,
};

// ---------- API (wire) types: owned & serde-friendly ----------

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiVariable {
    id: String,
    bound: Bound, // (i32, i32) from glpk_rust
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiShape {
    nrows: usize,
    ncols: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiIntegerSparseMatrix {
    rows: Vec<i32>,
    cols: Vec<i32>,
    vals: Vec<i32>,
    shape: ApiShape,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SparseLEIntegerPolyhedron {
    A: ApiIntegerSparseMatrix,
    b: Vec<i32>, // LE right-hand side
    variables: Vec<ApiVariable>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SolverDirection {
    Maximize,
    Minimize,
}

type ObjectiveOwned = HashMap<String, f64>;

#[derive(Deserialize)]
pub struct SolveRequest {
    polyhedron: SparseLEIntegerPolyhedron,
    objectives: Vec<ObjectiveOwned>,
    direction: SolverDirection,
}

// ---------- API response types (decoupled from the lib) ----------

#[derive(Serialize, Deserialize)]
enum Status {
    Undefined = 1,
    Feasible = 2,
    Infeasible = 3,
    NoFeasible = 4,
    Optimal = 5,
    Unbounded = 6,
    SimplexFailed = 7,
    MIPFailed = 8,
    EmptySpace = 9,
}

impl From<GlpkStatus> for Status {
    fn from(s: GlpkStatus) -> Self {
        // Assumes your crate uses the same variant names
        match s {
            GlpkStatus::Undefined => Status::Undefined,
            GlpkStatus::Feasible => Status::Feasible,
            GlpkStatus::Infeasible => Status::Infeasible,
            GlpkStatus::NoFeasible => Status::NoFeasible,
            GlpkStatus::Optimal => Status::Optimal,
            GlpkStatus::Unbounded => Status::Unbounded,
            GlpkStatus::SimplexFailed => Status::SimplexFailed,
            GlpkStatus::MIPFailed => Status::MIPFailed,
            GlpkStatus::EmptySpace => Status::EmptySpace,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ApiSolution {
    status: Status,
    objective: i32, // matches glpk_rust’s current output
    solution: HashMap<String, i64>,
    error: Option<String>,
}

// ---------- Helpers: convert API types → glpk_rust types ----------

fn api_matrix_to_glpk(m: &ApiIntegerSparseMatrix) -> GlpkMatrix {
    GlpkMatrix {
        rows: m.rows.clone(),
        cols: m.cols.clone(),
        vals: m.vals.clone(),
    }
}

/// Convert an API LE polyhedron to a GLPK LE polyhedron by building borrowed variables.
fn api_le_to_glpk_le<'a>(
    le: &'a SparseLEIntegerPolyhedron,
    id_storage: &'a [String],
) -> GlpkPoly<'a> {
    let glpk_a = api_matrix_to_glpk(&le.A);
    let glpk_b: Vec<Bound> = le.b.iter().map(|&v| (0, v)).collect();

    // Borrowed variables from id_storage
    // Ensure id_storage was created from le.variables in the same order
    let glpk_vars: Vec<GlpkVar<'a>> = le
        .variables
        .iter()
        .zip(id_storage.iter())
        .map(|(v, id)| GlpkVar {
            id: id.as_str(),
            bound: v.bound,
        })
        .collect();

    GlpkPoly {
        A: glpk_a,
        b: glpk_b,
        variables: glpk_vars,
        double_bound: false,
    }
}

// ---------- Route handlers ----------

/// POST /solve
pub async fn solve(req: web::Json<SolveRequest>) -> impl Responder {
    match validate_solve_request(&req) {
        Ok(_) => (),
        Err(response) => return response,
    }

    // Keep owned IDs alive while GLPK borrows &str from them
    let id_storage: Vec<String> = req
        .polyhedron
        .variables
        .iter()
        .map(|v| v.id.clone())
        .collect();

    // Build a quick intern map (&str -> &str) so we can map objective keys to the same &strs as variables
    let mut intern: HashMap<&str, &str> = HashMap::with_capacity(id_storage.len());
    for s in &id_storage {
        intern.insert(s.as_str(), s.as_str());
    }

    // Build a borrowed LE polyhedron for the solver
    let glpk_polyhedron = api_le_to_glpk_le(&req.polyhedron, &id_storage);
    // Solver expects &mut
    let mut glpk_polyhedron = glpk_polyhedron;

    // Convert objectives from HashMap<String, f64> → HashMap<&str, f64>
    // and ignore objective vars not in the polytope (as per your spec).
    let mut borrowed_objectives: Vec<HashMap<&str, f64>> = Vec::with_capacity(req.objectives.len());
    for obj in &req.objectives {
        let mut bobj: HashMap<&str, f64> = HashMap::with_capacity(obj.len());
        for (k, v) in obj {
            if let Some(&interned) = intern.get(k.as_str()) {
                bobj.insert(interned, *v);
            }
            // else: silently ignore unknown var (per your comment)
        }
        borrowed_objectives.push(bobj);
    }

    let maximize = req.direction == SolverDirection::Maximize;

    // Call the library solver
    let lib_solutions = glpk_solve_ilps(&mut glpk_polyhedron, borrowed_objectives, maximize, false);

    // Map library solutions → API solutions with owned Strings
    let api_solutions: Vec<ApiSolution> = lib_solutions
        .into_iter()
        .map(|s| ApiSolution {
            status: s.status.into(),
            objective: s.objective,
            solution: s
                .solution
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            error: s.error,
        })
        .collect();

    HttpResponse::Ok().json(serde_json::json!({ "solutions": api_solutions }))
}

fn validate_solve_request(req: &SolveRequest) -> Result<(), HttpResponse> {
    let variable_count = req.polyhedron.variables.len();
    let column_count = req.polyhedron.A.shape.ncols;
    if variable_count != column_count {
        return Err(HttpResponse::UnprocessableEntity().json(
            serde_json::json!({
                "error": format!("Number of variables must match number of columns in A got {} variables and {} columns", variable_count, column_count)
            }),
        ));
    }

    let b_count = req.polyhedron.b.len();
    let row_count = req.polyhedron.A.shape.nrows;
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

    fn make_valid_request() -> SolveRequest {
        SolveRequest {
            polyhedron: SparseLEIntegerPolyhedron {
                A: ApiIntegerSparseMatrix {
                    rows: vec![0, 1, 2],
                    cols: vec![0, 1, 2],
                    vals: vec![1, 2, 3],
                    shape: ApiShape { nrows: 3, ncols: 3 },
                },
                b: vec![10, 20, 30],
                variables: vec![
                    ApiVariable { id: "x1".into(), bound: (0, 100) },
                    ApiVariable { id: "x2".into(), bound: (0, 100) },
                    ApiVariable { id: "x3".into(), bound: (0, 100) },
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

    println!(
        "Server is {}",
        if protect { "protected" } else { "unprotected" }
    );
    println!("Starting server on http://127.0.0.1:{}", port);
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
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
