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
        thread::sleep(Duration::from_secs(5));
        
        // Test if server is responding with better error handling
        let mut server_ready = false;
        for attempt in 0..15 {
            if let Ok(output) = std::process::Command::new("curl")
                .args(&["-s", "-o", "/dev/null", "-w", "%{http_code}", &format!("http://127.0.0.1:{}/health", port)])
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