use serde_json::json;
use serial_test::serial;
use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU16, Ordering};
use std::thread;
use std::time::Duration;

static PORT_COUNTER: AtomicU16 = AtomicU16::new(9010);

fn test_solver() -> String {
    std::env::var("TEST_SOLVER")
        .unwrap_or_else(|_| "glpk".to_string())
        .to_lowercase()
}

struct TestServer {
    child: Option<Child>,
    port: u16,
}

impl TestServer {
    fn start(env_vars: Option<HashMap<String, String>>) -> Self {
        // Get a unique port for this test
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);

        let mut cmd = Command::new("cargo");
        cmd.args(&["run"]);
        // TEST_SOLVER=highs|gurobi runs the server with that solver (default GLPK)
        let solver = test_solver();
        if solver != "glpk" {
            cmd.args(&["--features", &format!("{}-solver", solver)]);
        }
        cmd.env("SOLVER", &solver);
        cmd.env("PORT", port.to_string());

        if let Some(env_vars) = env_vars {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        let child = cmd.spawn().expect("Failed to start test server");

        // Wait longer for server to start
        thread::sleep(Duration::from_secs(10));

        // Test if server is responding with better error handling
        let mut server_ready = false;
        for attempt in 0..30 {
            if let Ok(output) = std::process::Command::new("curl")
                .args(&[
                    "-s",
                    "-o",
                    "/dev/null",
                    "-w",
                    "%{http_code}",
                    &format!("http://127.0.0.1:{}/health", port),
                ])
                .output()
            {
                let status_code = String::from_utf8_lossy(&output.stdout);
                if status_code.trim() == "200" {
                    server_ready = true;
                    break;
                }
            }
            println!("Attempt {}: Server not ready yet, waiting...", attempt + 1);
            thread::sleep(Duration::from_millis(1000));
        }

        if !server_ready {
            panic!("Server failed to start on port {} after 15 seconds", port);
        }

        TestServer {
            child: Some(child),
            port,
        }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[tokio::test]
#[serial]
async fn test_health_endpoint() {
    let _server = TestServer::start(None);
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health", _server.base_url()))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);
    let body = response.text().await.expect("Failed to read response body");
    assert_eq!(body, "OK");
}

#[tokio::test]
#[serial]
async fn test_solve_valid_request() {
    let _server = TestServer::start(None);
    let client = reqwest::Client::new();

    let request_body = json!({
        "polyhedron": {
            "A": {
                "rows": [0, 0, 1, 1, 2, 2],
                "cols": [0, 1, 0, 2, 1, 2],
                "vals": [1, 1, 1, 1, 1, 1],
                "shape": {"nrows": 3, "ncols": 3}
            },
            "b": [1, 1, 1],
            "variables": [
                {"id": "x1", "bound": [0, 1]},
                {"id": "x2", "bound": [0, 1]},
                {"id": "x3", "bound": [0, 1]}
            ]
        },
        "objectives": [
            {"x1": 0, "x2": 0, "x3": 1}
        ],
        "direction": "maximize"
    });

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    assert!(body["solutions"].is_array());
    let solutions = body["solutions"].as_array().unwrap();
    assert!(!solutions.is_empty());
}

#[tokio::test]
#[serial]
async fn test_solve_invalid_json() {
    let _server = TestServer::start(None);
    let client = reqwest::Client::new();

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .header("content-type", "application/json")
        .body("invalid json")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    assert!(body["error"].is_string());
}

#[tokio::test]
#[serial]
async fn test_solve_minimize_direction() {
    let _server = TestServer::start(None);
    let client = reqwest::Client::new();

    let request_body = json!({
        "polyhedron": {
            "A": {
                "rows": [0, 0],
                "cols": [0, 1],
                "vals": [1, 1],
                "shape": {"nrows": 1, "ncols": 2}
            },
            "b": [2],
            "variables": [
                {"id": "x1", "bound": [0, 5]},
                {"id": "x2", "bound": [0, 5]}
            ]
        },
        "objectives": [
            {"x1": 1, "x2": 1}
        ],
        "direction": "minimize"
    });

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    assert!(body["solutions"].is_array());
}

#[tokio::test]
#[serial]
async fn test_nonexistent_endpoint() {
    let _server = TestServer::start(None);
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/nonexistent", _server.base_url()))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
#[serial]
async fn test_docs_endpoint() {
    let _server = TestServer::start(None);
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/docs", _server.base_url()))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let body = response.text().await.expect("Failed to read response body");
    assert!(body.contains("GLPK Rust API Documentation"));
    assert!(body.contains("<!DOCTYPE html"));
}

fn start_test_server_with_auth() -> TestServer {
    TestServer::start(Some(HashMap::from([
        ("PROTECT".to_string(), "true".to_string()),
        ("API_TOKEN".to_string(), "secret".to_string()),
    ])))
}

#[tokio::test]
#[serial]
async fn test_empty_endpoint_should_bypass_auth() {
    let _server = start_test_server_with_auth();
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}", _server.base_url()))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let body = response.text().await.expect("Failed to read response body");
    assert!(body.contains("GLPK Rust API Documentation"));
    assert!(body.contains("<!DOCTYPE html"));
}

#[tokio::test]
#[serial]
async fn test_health_endpoint_should_bypass_auth() {
    let _server = start_test_server_with_auth();
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health", _server.base_url()))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);
    let body = response.text().await.expect("Failed to read response body");
    assert_eq!(body, "OK");
}

#[tokio::test]
#[serial]
async fn test_docs_endpoint_should_bypass_auth() {
    let _server = start_test_server_with_auth();
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/docs", _server.base_url()))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let body = response.text().await.expect("Failed to read response body");
    assert!(body.contains("GLPK Rust API Documentation"));
    assert!(body.contains("<!DOCTYPE html"));
}

#[tokio::test]
#[serial]
async fn test_solve_valid_token() {
    let _server = start_test_server_with_auth();
    let client = reqwest::Client::new();

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .header("content-type", "application/json")
        .header("x-api-key", "secret")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 400); // Bad Request due to missing body, but is authorized.
}

#[tokio::test]
#[serial]
async fn test_solve_invalid_token() {
    let _server = start_test_server_with_auth();
    let client = reqwest::Client::new();

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .header("content-type", "application/json")
        .header("x-api-key", "invalid_token")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 403);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    assert_eq!(body["error"], "Forbidden");
}

#[tokio::test]
#[serial]
async fn test_solve_no_token_header() {
    let _server = start_test_server_with_auth();
    let client = reqwest::Client::new();

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .header("content-type", "application/json")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    assert_eq!(body["error"], "Unauthorized");
}

fn start_test_server_with_solve_time_limit() -> TestServer {
    TestServer::start(Some(HashMap::from([(
        "MAX_GLPK_SOLVE_MS".to_string(),
        "2".to_string(),
    )])))
}

#[tokio::test]
#[serial]
async fn test_solve_max_glpk_solve_ms() {
    if test_solver() != "glpk" {
        return; // MAX_GLPK_SOLVE_MS only applies to GLPK
    }
    let _server = start_test_server_with_solve_time_limit();
    let client = reqwest::Client::new();

    let request_body = json!({
        "polyhedron": {
            "A": {
                "rows": [0, 0, 1, 1, 2, 2],
                "cols": [0, 1, 0, 2, 1, 2],
                "vals": [1, 1, 1, 1, 1, 1],
                "shape": {"nrows": 3, "ncols": 3}
            },
            "b": [1, 1, 1],
            "variables": [
                {"id": "x1", "bound": [0, 1]},
                {"id": "x2", "bound": [0, 1]},
                {"id": "x3", "bound": [0, 1]}
            ]
        },
        "objectives": [
            {"x1": 0, "x2": 0, "x3": 1}
        ],
        "direction": "maximize"
    });

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 504);
}

// ---------- Unified status contract (same answers for every solver) ----------

fn solver_display_name() -> &'static str {
    match test_solver().as_str() {
        "highs" => "HiGHS",
        "gurobi" => "Gurobi",
        _ => "GLPK",
    }
}

async fn solve_single(server: &TestServer, polyhedron: serde_json::Value) -> serde_json::Value {
    let response = reqwest::Client::new()
        .post(&format!("{}/solve", server.base_url()))
        .json(&json!({
            "polyhedron": polyhedron,
            "objectives": [{"x": 1}],
            "direction": "maximize"
        }))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    body["solutions"][0].clone()
}

/// MIPFailed, empty solution, objective 0 and
/// "Problem is infeasible...: <Solver> reported <NAME> (<code>)"
fn assert_infeasible(solution: &serde_json::Value) {
    assert_eq!(solution["status"], "MIPFailed", "{}", solution);
    assert_eq!(solution["solution"], json!({}), "{}", solution);
    assert_eq!(solution["objective"], 0, "{}", solution);
    let error = solution["error"]
        .as_str()
        .expect("error should be a string");
    let (summary, detail) = error.split_once(": ").expect("summary: detail");
    assert!(
        summary == "Problem is infeasible" || summary == "Problem is infeasible or unbounded",
        "{}",
        error
    );
    let prefix = format!("{} reported ", solver_display_name());
    assert!(detail.starts_with(&prefix), "{}", error);
    let (_, code) = detail.rsplit_once(" (").expect("(<code>)");
    let code = code.strip_suffix(')').expect("ends with )");
    assert!(code.parse::<i32>().is_ok(), "{}", error);
}

async fn assert_status_contract(use_presolve: &str) {
    let server = TestServer::start(Some(HashMap::from([(
        "USE_PRESOLVE".to_string(),
        use_presolve.to_string(),
    )])));

    // Optimal: x + y <= 1, maximize x
    let optimal = solve_single(
        &server,
        json!({
            "A": {"rows": [0, 0], "cols": [0, 1], "vals": [1, 1], "shape": {"nrows": 1, "ncols": 2}},
            "b": [1],
            "variables": [{"id": "x", "bound": [0, 1]}, {"id": "y", "bound": [0, 1]}]
        }),
    )
    .await;
    assert_eq!(optimal["status"], "Optimal", "{}", optimal);
    assert_eq!(optimal["error"], serde_json::Value::Null);
    assert_eq!(optimal["objective"], 1);
    assert_eq!(optimal["solution"], json!({"x": 1, "y": 0}));

    // LP relaxation infeasible: x in [5, 10], x <= 3
    let lp_infeasible = solve_single(
        &server,
        json!({
            "A": {"rows": [0], "cols": [0], "vals": [1], "shape": {"nrows": 1, "ncols": 1}},
            "b": [3],
            "variables": [{"id": "x", "bound": [5, 10]}]
        }),
    )
    .await;
    assert_infeasible(&lp_infeasible);

    // Only integer infeasible: 2x = 1
    let int_infeasible = solve_single(
        &server,
        json!({
            "A": {"rows": [0, 1], "cols": [0, 0], "vals": [2, -2], "shape": {"nrows": 2, "ncols": 1}},
            "b": [1, -1],
            "variables": [{"id": "x", "bound": [0, 1]}]
        }),
    )
    .await;
    assert_infeasible(&int_infeasible);
}

#[tokio::test]
#[serial]
async fn test_status_contract_with_presolve() {
    assert_status_contract("true").await;
}

#[tokio::test]
#[serial]
async fn test_status_contract_without_presolve() {
    assert_status_contract("false").await;
}
