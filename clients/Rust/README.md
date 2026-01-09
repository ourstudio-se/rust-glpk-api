# GLPK API Client SDK

A Rust client SDK for interacting with the [GLPK REST API](https://github.com/ourstudio-se/rust-glpk-api) for solving linear programming problems.

## Features

- ✅ Full support for GLPK REST API
- ✅ Type-safe request/response models
- ✅ Builder pattern for easy request construction
- ✅ Authentication support for protected endpoints
- ✅ Async/await with tokio
- ✅ Comprehensive error handling
- ✅ Health check endpoint
- ✅ Multiple objective functions

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
glpk-api-sdk = "0.1.0"
tokio = { version = "1.0", features = ["macros", "rt-multi-thread"] }
```

## Quick Start

```rust
use glpk_api_sdk::{GlpkClient, SolveRequestBuilder, SolverDirection, Variable};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client
    let client = GlpkClient::new("http://localhost:9000")?;

    // Build a request
    let request = SolveRequestBuilder::new()
        .add_variable(Variable::new("x1", 0, 100))
        .add_variable(Variable::new("x2", 0, 100))
        .add_constraint(vec![0, 0], vec![0, 1], vec![2, 3], 100)
        .add_objective([("x1", 1.0), ("x2", 2.0)].into())
        .direction(SolverDirection::Maximize)
        .build()?;

    // Solve
    let response = client.solve(request).await?;

    // Process results
    for solution in response.solutions {
        println!("Status: {:?}", solution.status);
        println!("Objective: {}", solution.objective);
        println!("Solution: {:?}", solution.solution);
    }

    Ok(())
}
```

## Authentication

When the API is running in protected mode (`PROTECT=true`), you need to provide an API key:

```rust
let client = GlpkClient::new("http://localhost:9000")?
    .with_api_key("your-api-key");
```

Or use environment variables:

```rust
use std::env;

let api_key = env::var("GLPK_API_KEY")?;
let client = GlpkClient::new("http://localhost:9000")?
    .with_api_key(api_key);
```

## Examples

### Simple Linear Programming Problem

```rust
use glpk_api_sdk::{GlpkClient, SolveRequestBuilder, SolverDirection, Variable};

let request = SolveRequestBuilder::new()
    .add_variable(Variable::new("x", 0, 10))
    .add_variable(Variable::new("y", 0, 10))
    // Constraint: x + y ≤ 5
    .add_constraint(vec![0, 0], vec![0, 1], vec![1, 1], 5)
    // Maximize: 3x + 2y
    .add_objective([("x", 3.0), ("y", 2.0)].into())
    .direction(SolverDirection::Maximize)
    .build()?;

let response = client.solve(request).await?;
```

### Multiple Objectives

```rust
let request = SolveRequestBuilder::new()
    .add_variable(Variable::new("x1", 0, 1))
    .add_variable(Variable::new("x2", 0, 1))
    .add_variable(Variable::new("x3", 0, 1))
    .add_constraint(vec![0, 0], vec![0, 1], vec![1, 1], 1)
    .add_constraint(vec![1, 1], vec![0, 2], vec![1, 1], 1)
    // Objective 1: Maximize x3
    .add_objective([("x3", 1.0)].into())
    // Objective 2: Maximize x1 + 2*x2 + x3
    .add_objective([("x1", 1.0), ("x2", 2.0), ("x3", 1.0)].into())
    .direction(SolverDirection::Maximize)
    .build()?;

let response = client.solve(request).await?;
// Returns one solution for each objective
```

### Health Check

```rust
let client = GlpkClient::new("http://localhost:9000")?;

if client.health_check().await? {
    println!("Server is healthy");
}
```

## API Documentation

### Types

- **`Variable`** - Decision variable with id and bounds
- **`IntegerSparseMatrix`** - Sparse matrix in coordinate format
- **`SparseLEIntegerPolyhedron`** - Constraint polyhedron (Ax ≤ b)
- **`SolveRequest`** - Complete solve request
- **`SolveResponse`** - Response with solutions
- **`Solution`** - Single solution with status and values
- **`Status`** - Solution status enum (Optimal, Infeasible, etc.)
- **`SolverDirection`** - Maximize or Minimize

### Builder Methods

- **`add_variable(variable)`** - Add a decision variable
- **`add_variables(variables)`** - Add multiple variables
- **`add_constraint(rows, cols, vals, b)`** - Add a constraint
- **`add_objective(objective)`** - Add an objective function
- **`add_objectives(objectives)`** - Add multiple objectives
- **`direction(direction)`** - Set optimization direction
- **`build()`** - Build the request

### Client Methods

- **`new(base_url)`** - Create a new client
- **`with_client(base_url, client)`** - Create with custom reqwest client
- **`with_api_key(key)`** - Set API key for authentication
- **`health_check()`** - Check server health
- **`solve(request)`** - Solve linear programming problem

## Sparse Matrix Format

The SDK uses sparse matrix format for efficiency:

```rust
// Matrix:
// [1  0  2]
// [0  3  0]

let matrix = IntegerSparseMatrix::new(
    vec![0, 0, 1],    // rows
    vec![0, 2, 1],    // cols
    vec![1, 2, 3],    // vals
    2,                // nrows
    3,                // ncols
);
```

## Error Handling

All errors are wrapped in the `GlpkError` enum:

```rust
use glpk_api_sdk::GlpkError;

match client.solve(request).await {
    Ok(response) => { /* handle success */ },
    Err(GlpkError::AuthenticationFailed) => { /* handle auth error */ },
    Err(GlpkError::ApiError(msg)) => { /* handle API error */ },
    Err(e) => { /* handle other errors */ },
}
```

## Running Examples

Start the GLPK API server:

```bash
cd ..
cargo run
```

Run an example:

```bash
cd client-sdk
cargo run --example basic_usage
```

With authentication:

```bash
export GLPK_API_KEY="your-api-key"
cargo run --example with_authentication
```

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Links

- [GLPK API Repository](https://github.com/ourstudio-se/rust-glpk-api)
- [GLPK Documentation](https://www.gnu.org/software/glpk/)
