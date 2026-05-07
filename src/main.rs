mod convert;
mod domain;
mod models;
mod memory_tracker;

use models::SolveRequest;
use memory_tracker::MemoryTracker;

// Use jemalloc as the global allocator for memory profiling
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use domain::solver::Solver;
use domain::solver_factory::{create_solver_with_cache, SolverType};

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
use subtle::ConstantTimeEq;

// ---------- Helper functions ----------
#[cfg(target_os = "macos")]
fn parse_size_to_mb(size_str: &str) -> f64 {
    if let Some(num_str) = size_str.strip_suffix('K') {
        num_str.parse::<f64>().unwrap_or(0.0) / 1024.0
    } else if let Some(num_str) = size_str.strip_suffix('M') {
        num_str.parse::<f64>().unwrap_or(0.0)
    } else if let Some(num_str) = size_str.strip_suffix('G') {
        num_str.parse::<f64>().unwrap_or(0.0) * 1024.0
    } else {
        0.0
    }
}

// ---------- Route handlers ----------
/// POST /solve
pub async fn solve(
    req: web::Json<SolveRequest>,
    solver: web::Data<Box<dyn Solver>>,
    use_presolve: web::Data<bool>,
    solver_semaphore: web::Data<Arc<tokio::sync::Semaphore>>,
    memory_tracker: web::Data<MemoryTracker>,
) -> impl Responder {
    match validate_solve_request(&req) {
        Ok(_) => (),
        Err(response) => return response,
    }

    // Acquire an owned permit asynchronously before spawning the blocking task.
    let sem = solver_semaphore.get_ref().clone();
    let permit = match sem.acquire_owned().await {
        Ok(p) => p,
        Err(e) => {
            sentry::capture_message(
                &format!("Failed to acquire semaphore permit: {}", e),
                sentry::Level::Error,
            );
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": "Something went wrong"}));
        }
    };

    let SolveRequest {
        polyhedron,
        objectives,
        direction,
    } = req.into_inner();

    // Track input size
    let input_size = std::mem::size_of_val(&polyhedron)
        + std::mem::size_of_val(&objectives)
        + polyhedron.a.rows.len() * std::mem::size_of::<i32>()
        + polyhedron.a.cols.len() * std::mem::size_of::<i32>()
        + polyhedron.a.vals.len() * std::mem::size_of::<i32>();
    memory_tracker.track_allocation("solve_requests", input_size);

    let tracker_clone = memory_tracker.get_ref().clone();
    let solve_task_result = tokio::task::spawn_blocking(move || {
        // Hold the permit for the duration of the blocking solver call by moving
        // it into the closure. It will be released automatically when dropped.
        let _permit = permit;
        let result = solver.solve(polyhedron, objectives, direction, *use_presolve.get_ref());

        // Track deallocation after solve
        tracker_clone.track_deallocation("solve_requests", input_size);
        result
    })
    .await;

    let solve_result = match solve_task_result {
        Err(e) => {
            sentry::capture_message(
                &format!("Solver thread did not complete successfully: {}", e),
                sentry::Level::Error,
            );
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Something went wrong",
            }));
        }
        Ok(res) => res,
    };

    match solve_result {
        Ok(api_solutions) => {
            HttpResponse::Ok().json(serde_json::json!({ "solutions": api_solutions }))
        }
        Err(error) => {
            // Capture error with breadcrumb context
            sentry::capture_message(
                &format!("Solve failed: {}", error.details),
                sentry::Level::Error,
            );
            HttpResponse::UnprocessableEntity().json(serde_json::json!({
                "error": error.details,
            }))
        }
    }
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

    // Validate sparse matrix arrays have same length
    let rows_len = req.polyhedron.a.rows.len();
    let cols_len = req.polyhedron.a.cols.len();
    let vals_len = req.polyhedron.a.vals.len();
    if rows_len != cols_len || rows_len != vals_len {
        return Err(HttpResponse::UnprocessableEntity().json(
            serde_json::json!({
                "error": format!("Sparse matrix arrays must have same length: got rows={}, cols={}, vals={}", rows_len, cols_len, vals_len)
            }),
        ));
    }

    // Validate sparse matrix indices are within bounds
    for i in 0..rows_len {
        let row = req.polyhedron.a.rows[i];
        let col = req.polyhedron.a.cols[i];

        if row < 0 || row >= row_count as i32 {
            return Err(HttpResponse::UnprocessableEntity().json(
                serde_json::json!({
                    "error": format!("Row index {} at position {} is out of bounds [0, {})", row, i, row_count)
                }),
            ));
        }

        if col < 0 || col >= column_count as i32 {
            return Err(HttpResponse::UnprocessableEntity().json(
                serde_json::json!({
                    "error": format!("Column index {} at position {} is out of bounds [0, {})", col, i, column_count)
                }),
            ));
        }
    }

    // Input size limits (prevent DoS/OOM)
    const MAX_VARIABLES: usize = 100_000;
    const MAX_CONSTRAINTS: usize = 100_000;
    const MAX_NONZEROS: usize = 1_000_000;

    if variable_count > MAX_VARIABLES {
        return Err(HttpResponse::UnprocessableEntity().json(
            serde_json::json!({
                "error": format!("Too many variables: {} exceeds limit of {}", variable_count, MAX_VARIABLES)
            }),
        ));
    }

    if row_count > MAX_CONSTRAINTS {
        return Err(HttpResponse::UnprocessableEntity().json(
            serde_json::json!({
                "error": format!("Too many constraints: {} exceeds limit of {}", row_count, MAX_CONSTRAINTS)
            }),
        ));
    }

    if rows_len > MAX_NONZEROS {
        return Err(HttpResponse::UnprocessableEntity().json(
            serde_json::json!({
                "error": format!("Too many non-zero elements: {} exceeds limit of {}", rows_len, MAX_NONZEROS)
            }),
        ));
    }

    Ok(())
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

/// GET /memory-profile - Generate memory profile dump
pub async fn memory_profile(
    _memory_tracker: web::Data<MemoryTracker>,
    solver: web::Data<Box<dyn Solver>>,
) -> impl Responder {
    #[cfg(not(target_env = "msvc"))]
    {
        use tikv_jemalloc_ctl::{stats, epoch};

        // Trigger jemalloc to update stats
        if let Err(e) = epoch::mib().unwrap().advance() {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to update jemalloc stats: {}", e)
            }));
        }

        // Get overall memory statistics
        let allocated = stats::allocated::read().unwrap_or(0);
        let resident = stats::resident::read().unwrap_or(0);
        let metadata = stats::metadata::read().unwrap_or(0);
        let active = stats::active::read().unwrap_or(0);

        // Get process info
        let pid = std::process::id();
        let solver_name = solver.name();

        // Get memory breakdown - platform specific
        let type_breakdown: Vec<serde_json::Value> = {
            #[cfg(target_os = "macos")]
            {
                use std::process::Command;

                // Use vmmap without -summary to get detailed resident memory info
                let output = Command::new("vmmap")
                    .arg(pid.to_string())
                    .output();

                if let Ok(output) = output {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
                        let mut region_memory: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

                        // Parse vmmap output - format varies but typically has columns like:
                        // TYPE                       VIRTUAL     RESIDENT    ...
                        for line in stdout.lines() {
                            let parts: Vec<&str> = line.split_whitespace().collect();

                            // Skip header lines
                            if line.contains("REGION TYPE") || line.contains("=====") || parts.is_empty() {
                                continue;
                            }

                            // Look for lines with memory region info
                            // Format: REGION_TYPE ... SIZE ... RESIDENT_SIZE
                            if parts.len() >= 3 {
                                // The first column is usually the region type
                                let region_type = parts[0].to_string();

                                // Find the resident column (usually has K/M/G suffix)
                                for (i, part) in parts.iter().enumerate() {
                                    if i > 0 && (part.ends_with('K') || part.ends_with('M') || part.ends_with('G')) {
                                        // This might be resident size - try next column too
                                        if let Some(next_part) = parts.get(i + 1) {
                                            if next_part.ends_with('K') || next_part.ends_with('M') || next_part.ends_with('G') {
                                                // Second size column is usually resident
                                                let resident_mb = parse_size_to_mb(next_part);
                                                let resident_bytes = (resident_mb * 1024.0 * 1024.0) as usize;
                                                *region_memory.entry(region_type.clone()).or_insert(0) += resident_bytes;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Convert to sorted vec
                        let mut mappings: Vec<_> = region_memory
                            .into_iter()
                            .map(|(region, bytes)| {
                                serde_json::json!({
                                    "region": region,
                                    "resident_bytes": bytes,
                                    "resident_mb": (bytes as f64) / (1024.0 * 1024.0)
                                })
                            })
                            .filter(|m| m["resident_mb"].as_f64().unwrap_or(0.0) > 0.1)
                            .collect();

                        mappings.sort_by(|a, b| {
                            let a_mb = a["resident_mb"].as_f64().unwrap_or(0.0);
                            let b_mb = b["resident_mb"].as_f64().unwrap_or(0.0);
                            b_mb.partial_cmp(&a_mb).unwrap()
                        });

                        mappings.truncate(20);
                        mappings
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            }

            #[cfg(target_os = "linux")]
            {
                use std::fs::File;
                use std::io::{BufRead, BufReader};

                // Parse /proc/self/smaps for detailed memory region info
                if let Ok(file) = File::open("/proc/self/smaps") {
                    let reader = BufReader::new(file);
                    let mut region_memory: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
                    let mut current_region: Option<String> = None;

                    for line in reader.lines().flatten() {
                        // New memory mapping entry starts with an address range
                        // Format: 7f1234567000-7f123456a000 r-xp 00000000 08:01 12345 /lib/x86_64-linux-gnu/libc.so.6
                        if line.contains('-') && !line.starts_with(' ') {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 5 {
                                // Extract region type from the pathname or permissions
                                let region_name = if parts.len() > 5 {
                                    // Has a pathname
                                    let path = parts[5..].join(" ");
                                    if path.starts_with('[') && path.ends_with(']') {
                                        path.trim_matches(|c| c == '[' || c == ']').to_string()
                                    } else if path.contains('/') {
                                        // Extract filename from path
                                        path.split('/').last().unwrap_or("anonymous").to_string()
                                    } else {
                                        path
                                    }
                                } else {
                                    "anonymous".to_string()
                                };
                                current_region = Some(region_name);
                            }
                        } else if line.starts_with("Rss:") {
                            // Rss: Resident Set Size (physical memory used)
                            if let Some(ref region) = current_region {
                                let parts: Vec<&str> = line.split_whitespace().collect();
                                if parts.len() >= 2 {
                                    if let Ok(kb) = parts[1].parse::<usize>() {
                                        let bytes = kb * 1024;
                                        *region_memory.entry(region.clone()).or_insert(0) += bytes;
                                    }
                                }
                            }
                        }
                    }

                    // Convert to sorted vec
                    let mut mappings: Vec<_> = region_memory
                        .into_iter()
                        .map(|(region, bytes)| {
                            serde_json::json!({
                                "region": region,
                                "resident_bytes": bytes,
                                "resident_mb": (bytes as f64) / (1024.0 * 1024.0)
                            })
                        })
                        .filter(|m| m["resident_mb"].as_f64().unwrap_or(0.0) > 0.1)
                        .collect();

                    mappings.sort_by(|a, b| {
                        let a_mb = a["resident_mb"].as_f64().unwrap_or(0.0);
                        let b_mb = b["resident_mb"].as_f64().unwrap_or(0.0);
                        b_mb.partial_cmp(&a_mb).unwrap()
                    });

                    mappings.truncate(20);
                    mappings
                } else {
                    Vec::new()
                }
            }

            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            {
                Vec::new()
            }
        };

        HttpResponse::Ok().json(serde_json::json!({
            "allocator": "jemalloc",
            "process_id": pid,
            "solver": solver_name,
            "total_memory": {
                "allocated_bytes": allocated,
                "active_bytes": active,
                "resident_bytes": resident,
                "metadata_bytes": metadata,
                "allocated_mb": (allocated as f64) / (1024.0 * 1024.0),
                "resident_mb": (resident as f64) / (1024.0 * 1024.0)
            },
            "memory_mappings": type_breakdown,
            "note": "Memory mappings show different regions of process memory (heap, stack, libraries, etc.)"
        }))
    }

    #[cfg(target_env = "msvc")]
    {
        HttpResponse::Ok().json(serde_json::json!({
            "error": "Memory profiling not available on MSVC (Windows). Use jemalloc on Unix systems."
        }))
    }
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

    // Use constant-time comparison to prevent timing attacks
    let valid_token = auth.token.as_bytes().ct_eq(token.as_bytes()).into();

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

    // Optional CAAS tag (default: not set)
    let caas_tag = env::var("SENTRY_CAAS_TAG").ok();

    println!("Initializing Sentry with environment: {}", environment);

    sentry::init((
        dsn,
        sentry::ClientOptions {
            environment: Some(environment.into()),
            attach_stacktrace: true,
            before_send: Some(Arc::new(move |mut event| {
                event.tags.insert("service".into(), service_name.clone());

                // Add caas tag if configured
                if let Some(ref caas_value) = caas_tag {
                    event.tags.insert("caas".into(), caas_value.clone());
                }

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

    // Initialize Sentry if DSN is configured
    // Guard must be kept in scope until the server exits
    let sentry_enabled = env::var("SENTRY_DSN").is_ok();
    let _sentry_guard = if sentry_enabled {
        println!("Sentry monitoring enabled");
        Some(init_sentry())
    } else {
        println!("Sentry monitoring disabled (no SENTRY_DSN configured)");
        None
    };
    // Select solver based on environment variable (default: GLPK)
    let solver_type = env::var("SOLVER")
        .ok()
        .and_then(|s| SolverType::from_str(&s))
        .unwrap_or(SolverType::Glpk);

    // Configure presolve (default: true)
    let use_presolve = env::var("USE_PRESOLVE")
        .ok()
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(true);

    // Configure model cache size (default: 0 disabled, set to enable)
    let cache_size = env::var("MODEL_CACHE_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());

    let solver = create_solver_with_cache(solver_type, cache_size);

    println!(
        "Server is {}",
        if protect { "protected" } else { "unprotected" }
    );
    println!("Using solver: {}", solver.name());
    println!(
        "Presolve: {}",
        if use_presolve { "enabled" } else { "disabled" }
    );
    match cache_size {
        Some(cs) => println!("LRU Model builder cache: {} entries", cs),
        None => println!("LRU Model builder cache: disabled"),
    }
    println!("Starting server on http://127.0.0.1:{}", port);

    // Clone solver and presolve flag for use in the closure
    let solver_data = web::Data::new(solver);
    let presolve_data = web::Data::new(use_presolve);
    let memory_tracker_data = web::Data::new(MemoryTracker::new());

    // Configure maximum concurrent blocking solver threads via env var.
    // Default to 1 unless the user supplies a value. If the env var is set
    // but invalid (non-integer or < 1) the server will panic with an error
    // to avoid silently running with unexpected configuration.

    let max_blocking_threads = env::var("MAX_BLOCKING_THREADS")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(1);
    let solver_semaphore = match max_blocking_threads {
        n if n < 1 => panic!("MAX_BLOCKING_THREADS must be >= 1"),
        n => Arc::new(tokio::sync::Semaphore::new(n as usize)),
    };

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Condition::new(sentry_enabled, Sentry::new()))
            .app_data(solver_data.clone())
            .app_data(presolve_data.clone())
            .app_data(memory_tracker_data.clone())
            .app_data(web::Data::new(solver_semaphore.clone()))
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
            .route("/memory-profile", web::get().to(memory_profile))
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

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::StatusCode;
    use std::collections::HashMap;

    use models::{
        ApiIntegerSparseMatrix, ApiShape, ApiVariable, SolverDirection, SparseLEIntegerPolyhedron,
    };

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
