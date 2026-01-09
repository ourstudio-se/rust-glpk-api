use glpk_api_sdk::{GlpkClient, SolveRequestBuilder, SolverDirection, Variable};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read configuration from environment variables
    let api_url = env::var("GLPK_API_URL").unwrap_or_else(|_| "http://127.0.0.1:9001".to_string());
    let api_key = env::var("GLPK_API_KEY").ok();

    // Create a client with optional authentication
    let mut client = GlpkClient::new(&api_url)?;

    if let Some(key) = api_key {
        println!("🔐 Using API key authentication");
        client = client.with_api_key(key);
    } else {
        println!("⚠ No API key provided (set GLPK_API_KEY environment variable)");
    }

    // Simple optimization problem
    let request = SolveRequestBuilder::new()
        .add_variable(Variable::new("x", 0, 100))
        .add_variable(Variable::new("y", 0, 100))
        // Constraint: 2x + 3y ≤ 100
        .add_constraint(vec![0, 0], vec![0, 1], vec![2, 3], 100)
        // Maximize: x + 2y
        .add_objective([("x".to_string(), 1.0), ("y".to_string(), 2.0)].into())
        .direction(SolverDirection::Maximize)
        .build()?;

    println!("📊 Solving optimization problem...");

    match client.solve(request).await {
        Ok(response) => {
            println!("✓ Success!\n");
            for solution in response.solutions {
                println!("Status: {:?}", solution.status);
                println!("Objective: {}", solution.objective);
                println!("Solution: {:?}", solution.solution);
            }
        }
        Err(e) => {
            eprintln!("✗ Error: {}", e);
            if matches!(e, glpk_api_sdk::GlpkError::AuthenticationFailed) {
                eprintln!("\nTip: Make sure GLPK_API_KEY is set correctly");
            }
            return Err(e.into());
        }
    }

    Ok(())
}
