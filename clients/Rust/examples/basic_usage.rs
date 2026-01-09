use glpk_api_sdk::{GlpkClient, SolveRequestBuilder, SolverDirection, Variable};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client (adjust URL as needed)
    let client = GlpkClient::new("http://127.0.0.1:9001")?;

    // Check if the server is healthy
    match client.health_check().await {
        Ok(true) => println!("✓ Server is healthy"),
        Ok(false) => println!("⚠ Server returned non-success status"),
        Err(e) => println!("✗ Health check failed: {}", e),
    }

    // Example problem from the README:
    // Variables: x1, x2, x3 with bounds [0, 1]
    // Constraints:
    //   x1 + x2 ≤ 1
    //   x1 + x3 ≤ 1
    //   x2 + x3 ≤ 1
    // Objectives:
    //   1. Maximize x3
    //   2. Maximize x1 + 2*x2 + x3

    let request = SolveRequestBuilder::new()
        .add_variable(Variable::new("x1", 0, 1))
        .add_variable(Variable::new("x2", 0, 1))
        .add_variable(Variable::new("x3", 0, 1))
        // Constraint 1: x1 + x2 ≤ 1 (row 0, cols 0 and 1)
        .add_constraint(vec![0, 0], vec![0, 1], vec![1, 1], 1)
        // Constraint 2: x1 + x3 ≤ 1 (row 1, cols 0 and 2)
        .add_constraint(vec![1, 1], vec![0, 2], vec![1, 1], 1)
        // Constraint 3: x2 + x3 ≤ 1 (row 2, cols 1 and 2)
        .add_constraint(vec![2, 2], vec![1, 2], vec![1, 1], 1)
        // Objective 1: x3
        .add_objective([("x3".to_string(), 1.0)].into())
        // Objective 2: x1 + 2*x2 + x3
        .add_objective(
            [
                ("x1".to_string(), 1.0),
                ("x2".to_string(), 2.0),
                ("x3".to_string(), 1.0),
            ]
            .into(),
        )
        .direction(SolverDirection::Maximize)
        .build()?;

    println!("\n📊 Solving linear programming problem...\n");

    let response = client.solve(request).await?;

    println!("✓ Received {} solution(s)\n", response.solutions.len());

    for (i, solution) in response.solutions.iter().enumerate() {
        println!("Solution {}:", i + 1);
        println!("  Status: {:?}", solution.status);
        println!("  Objective value: {}", solution.objective);
        println!("  Variables:");
        for (var, value) in &solution.solution {
            println!("    {} = {}", var, value);
        }
        if let Some(ref error) = solution.error {
            println!("  Error: {}", error);
        }
        println!();
    }

    Ok(())
}
