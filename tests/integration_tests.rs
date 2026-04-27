use serde_json::json;
use serial_test::serial;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU16, Ordering};
use std::thread;
use std::time::Duration;

static PORT_COUNTER: AtomicU16 = AtomicU16::new(9010);

struct TestServer {
    child: Option<Child>,
    port: u16,
}

impl TestServer {
    fn start() -> Self {
        // Get a unique port for this test
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);

        let child = Command::new("cargo")
            .args(&["run"])
            .env("PORT", port.to_string())
            .spawn()
            .expect("Failed to start test server");

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
    let _server = TestServer::start();
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
    let _server = TestServer::start();
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
    let _server = TestServer::start();
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
    let _server = TestServer::start();
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
    let _server = TestServer::start();
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
    let _server = TestServer::start();
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

struct TestServerWithAuth {
    child: Option<Child>,
    port: u16,
}

impl TestServerWithAuth {
    fn start() -> Self {
        // Get a unique port for this test
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);

        let child = Command::new("cargo")
            .args(&["run"])
            .env("PORT", port.to_string())
            .env("PROTECT", "true")
            .env("API_TOKEN", "secret")
            .spawn()
            .expect("Failed to start test server");

        // Wait longer for server to start
        thread::sleep(Duration::from_secs(5));

        // Test if server is responding with better error handling
        let mut server_ready = false;
        for attempt in 0..15 {
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

        TestServerWithAuth {
            child: Some(child),
            port,
        }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for TestServerWithAuth {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[tokio::test]
#[serial]
async fn test_empty_endpoint_should_bypass_auth() {
    let _server = TestServerWithAuth::start();
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
    let _server = TestServerWithAuth::start();
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
    let _server = TestServerWithAuth::start();
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
    let _server = TestServerWithAuth::start();
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
    let _server = TestServerWithAuth::start();
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
    let _server = TestServerWithAuth::start();
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

struct TestServerWithTimeout {
    child: Option<Child>,
    port: u16,
}

impl TestServerWithTimeout {
    fn start(timeout_seconds: u16) -> Self {
        // Get a unique port for this test
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);

        let child = Command::new("cargo")
            .args(&["run"])
            .env("PORT", port.to_string())
            .env("SOLVER_TIME_LIMIT", timeout_seconds.to_string())
            .spawn()
            .expect("Failed to start test server");

        // Wait for server to start
        thread::sleep(Duration::from_secs(10));

        // Test if server is responding
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
            panic!("Server failed to start on port {} after 30 seconds", port);
        }

        TestServerWithTimeout {
            child: Some(child),
            port,
        }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for TestServerWithTimeout {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Test that the solver respects the SOLVER_TIME_LIMIT and returns a 408 timeout
/// when a problem takes too long to solve
#[tokio::test]
#[serial]
async fn test_solve_with_timeout_exceeded() {
    // Start server with 1 second timeout
    let _server = TestServerWithTimeout::start(1);
    let client = reqwest::Client::new();

    // Create a very large problem that will take longer than 1 second to solve
    // This is a dense integer programming problem (500 vars × 200 constraints = 100k non-zeros)
    let num_vars = 500;
    let num_constraints = 200;

    let mut rows = Vec::new();
    let mut cols = Vec::new();
    let mut vals = Vec::new();
    let mut variables = Vec::new();
    let mut objective = serde_json::Map::new();

    // Create a dense constraint matrix (200 x 500 = 100,000 non-zeros)
    for i in 0..num_constraints {
        for j in 0..num_vars {
            rows.push(i);
            cols.push(j);
            // Make coefficients more varied to increase complexity
            vals.push(((i * 7 + j * 3 + 1) % 100 + 1) as i32);
        }
    }

    // Create variables with varied bounds
    for i in 0..num_vars {
        let var_id = format!("x{}", i);
        variables.push(json!({
            "id": var_id.clone(),
            "bound": [0, 10000]  // Larger bounds increase search space
        }));
        // Varied objective coefficients
        objective.insert(var_id, json!(((i * 13 + 7) % 20 + 1) as f64));
    }

    // Tighter constraints to increase difficulty
    let b = vec![1000; num_constraints];

    let request_body = json!({
        "polyhedron": {
            "A": {
                "rows": rows,
                "cols": cols,
                "vals": vals,
                "shape": {"nrows": num_constraints, "ncols": num_vars}
            },
            "b": b,
            "variables": variables
        },
        "objectives": [objective],
        "direction": "maximize"
    });

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .json(&request_body)
        .timeout(Duration::from_secs(5)) // Client timeout longer than server timeout
        .send()
        .await
        .expect("Failed to send request");

    // Should get 408 Request Timeout
    assert_eq!(response.status(), 408);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    assert!(body["error"].is_string());
    let error_msg = body["error"].as_str().unwrap();
    assert!(error_msg.contains("time limit") || error_msg.contains("timeout"));
}

/// Test that the solver completes successfully when the problem finishes
/// within the time limit
#[tokio::test]
#[serial]
async fn test_solve_with_timeout_not_exceeded() {
    // Start server with 60 second timeout (generous)
    let _server = TestServerWithTimeout::start(60);
    let client = reqwest::Client::new();

    // Create a simple, fast problem that should complete quickly
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
            {"x1": 1, "x2": 2, "x3": 3}
        ],
        "direction": "maximize"
    });

    let response = client
        .post(&format!("{}/solve", _server.base_url()))
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    // Should succeed normally
    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    assert!(body["solutions"].is_array());
    let solutions = body["solutions"].as_array().unwrap();
    assert!(!solutions.is_empty());
}
