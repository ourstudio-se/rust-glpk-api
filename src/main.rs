use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_web::middleware::Logger;
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
pub struct UpperBoundSparseGEIntegerPolyhedron {
    A: ApiIntegerSparseMatrix,
    b: Vec<i32>,              // GE right-hand side (we'll flip to LE)
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
    polyhedron: UpperBoundSparseGEIntegerPolyhedron,
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

/// Convert an API GE polyhedron (A x >= b) to a GLPK LE polyhedron (A' x <= b')
/// by negating A and b (A' = -A, b' = -b) and building borrowed variables.
fn ge_to_glpk_le<'a>(
    ge: &'a UpperBoundSparseGEIntegerPolyhedron,
    id_storage: &'a [String],
) -> GlpkPoly<'a> {
    // Flip GE → LE: negate all A values and b
    let mut glpk_A = api_matrix_to_glpk(&ge.A);
    glpk_A.vals = glpk_A.vals.into_iter().map(|v| -v).collect();

    let glpk_b: Vec<Bound> = ge.b.iter().map(|&v| (0, -v)).collect();

    // Borrowed variables from id_storage
    // Ensure id_storage was created from ge.variables in the same order
    let glpk_vars: Vec<GlpkVar<'a>> = ge
        .variables
        .iter()
        .zip(id_storage.iter())
        .map(|(v, id)| GlpkVar {
            id: id.as_str(),
            bound: v.bound,
        })
        .collect();

    GlpkPoly {
        A: glpk_A,
        b: glpk_b,
        variables: glpk_vars,
        double_bound: false, // matches your previous conversion
    }
}

// ---------- Route handlers ----------

/// POST /solve
pub async fn solve(req: web::Json<SolveRequest>) -> impl Responder {
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
    let glpk_polyhedron = ge_to_glpk_le(&req.polyhedron, &id_storage);
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

/// GET /health
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

/// GET /docs
pub async fn docs() -> impl Responder {
    let docs_html = include_str!("../static/docs.html");
    HttpResponse::Ok()
        .content_type("text/html")
        .body(docs_html)
}

/// GET / - Redirect to docs
pub async fn root_redirect() -> impl Responder {
    HttpResponse::Found()
        .append_header(("Location", "/docs"))
        .finish()
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
            .route("/", web::get().to(root_redirect))
            .route("/solve", web::post().to(solve))
            .route("/health", web::get().to(health_check))
            .route("/docs", web::get().to(docs))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
