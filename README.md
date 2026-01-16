# Linear Programming Rust API

A REST API for solving linear programming problems with support for multiple solver backends (GLPK and HiGHS).

## 🚀 Quick Start

### Running Locally

```bash
# Using GLPK (default)
cargo run

# Using HiGHS (requires cmake and feature flag)
SOLVER=highs cargo run --features highs-solver
```

Your application will be available at http://localhost:9000.

### Using Docker

```bash
docker compose up --build
```

## 🔧 Multi-Solver Support

This API supports multiple LP/ILP solver backends through a clean abstraction layer. You can easily switch between GLPK and HiGHS without changing your API or client code.

### Available Solvers

#### GLPK (Default)
- **Status**: ✅ Always available
- **Features**: Robust, battle-tested, integer programming support
- **Requirements**: None (included by default)

#### HiGHS
- **Status**: ⚠️ Optional feature (requires cmake)
- **Features**: Modern, faster for many problems, actively developed
- **Configuration**: Presolve is disabled for consistency with GLPK behavior
- **Requirements**:
  - `cmake` must be installed
  - Enable the `highs-solver` feature flag

### Switching Solvers

Set the `SOLVER` environment variable to choose your solver:

```bash
# Use GLPK (default)
SOLVER=glpk cargo run

# Use HiGHS (requires cmake and feature flag)
SOLVER=highs cargo run --features highs-solver
```

If `SOLVER` is not set, GLPK is used by default.

### Building with HiGHS Support

#### Prerequisites

Install cmake:
```bash
# macOS
brew install cmake

# Ubuntu/Debian
sudo apt-get install cmake

# Arch Linux
sudo pacman -S cmake
```

#### Build Commands

```bash
# Build with HiGHS support
cargo build --features highs-solver

# Run tests with HiGHS
cargo test --features highs-solver

# Run in production with HiGHS
SOLVER=highs cargo run --release --features highs-solver
```

### Performance Comparison

To compare solver performance on your workload:

```bash
# Test with GLPK
time SOLVER=glpk ./target/release/rust-glpk

# Test with HiGHS
time SOLVER=highs ./target/release/rust-glpk --features highs-solver
```

### API Compatibility

**Important**: The API endpoints, request/response formats, and client SDKs remain **completely unchanged** regardless of which solver is used. Clients don't need to know or care which solver backend is running.

## 📚 API Documentation

Visit `http://localhost:9000/docs` for interactive API documentation, or simply go to `http://localhost:9000` (automatically redirects to docs).

## 🔗 Endpoints

- `GET /` - Redirects to documentation
- `GET /docs` - Interactive API documentation  
- `GET /health` - Health check
- `POST /solve` - Solve linear programming problems

## 📝 Usage Example

### Simple Linear Programming Problem

```bash
curl -X POST http://127.0.0.1:9000/solve \
  -H "Content-Type: application/json" \
  -d '{
  "polyhedron": {
    "A": {
      "rows": [0,0,1,1,2,2],
      "cols": [0,1,0,2,1,2],
      "vals": [1,1,1,1,1,1],
      "shape": {"nrows": 3, "ncols": 3}
    },
    "b": [1, 1, 1],
    "variables": [
      { "id": "x1", "bound": [0,1] },
      { "id": "x2", "bound": [0,1] },
      { "id": "x3", "bound": [0,1] }
    ]
  },
  "objectives": [
    { "x1":0, "x2":0, "x3":1 },
    { "x1":1, "x2":2, "x3":1 }
  ],
  "direction": "maximize"
}'
```

### Response

Returns one solution for each objective:

```json
{
    "solutions": [
        {
            "error": null,
            "objective": 1,
            "solution": {
                "x1": 1,
                "x2": 1,
                "x3": 1
            },
            "status": "Optimal"
        },
        {
            "error": null,
            "objective": 4,
            "solution": {
                "x1": 1,
                "x2": 1,
                "x3": 1
            },
            "status": "Optimal"
        }
    ]
}
```

## 🧮 Problem Formulation

The API is designed to solve integer linear programming problems in the standard idiomatic form:

$$
\begin{align}
\text{maximize (or minimize) } & \quad w^T x \\
\text{subject to } & \quad Ax \leq b
\end{align}
$$

Where:
- $w$ is the objective coefficient vector (specified in the `objectives` field)
- $x$ is the decision variable vector (defined in the `variables` field)
- $A$ is the constraint coefficient matrix (specified in the `polyhedron.A` field)
- $b$ is the constraint right-hand side vector (specified in the `polyhedron.b` field)

This standard formulation allows you to express a wide variety of optimization problems by properly setting up the constraint matrix and objective coefficients.

## 📊 Request Structure

### Root Fields
- `polyhedron` - Constraint matrix and variable definitions
- `objectives` - Array of objective functions to optimize
- `direction` - Either "maximize" or "minimize"

### Polyhedron Structure
- `A` - Sparse constraint matrix (rows, cols, vals, shape)
- `b` - Right-hand side constraint values
- `variables` - Array of variable definitions with bounds

### Variable Structure
- `id` - Variable name (string)
- `bound` - [lower_bound, upper_bound] as integers

## 📊 Status Codes

| Code | Status | Description |
|------|--------|-------------|
| 1 | Undefined | Solution status is undefined |
| 2 | Feasible | Solution is feasible |
| 3 | Infeasible | Problem is infeasible |
| 4 | NoFeasible | No feasible solution exists |
| 5 | Optimal | Optimal solution found |
| 6 | Unbounded | Problem is unbounded |
| 7 | SimplexFailed | Simplex method failed |
| 8 | MIPFailed | Mixed-integer programming failed |
| 9 | EmptySpace | Search space is empty |

## ⚙️ Configuration

### Environment Variables

- `PORT` - Server port (default: 9000)
- `JSON_PAYLOAD_LIMIT` - Maximum request size (default: 2MB)
- `SOLVER` - Solver backend: `glpk` (default) or `highs`

### Using .env file

Create a `.env` file in the project root:

```
PORT=8080
JSON_PAYLOAD_LIMIT=5242880
SOLVER=glpk
```

### 🛡️ Protected mode

Enable authentication for the `POST /solve` endpoint by setting the following variables in the enviroment:

- `PROTECT=true` (default: `false`)
- `API_TOKEN=****`

When enabled, all requests to /solve must include a valid API key in a X-API-Key header.

## 🐳 Deploying with Docker

### Build the image

```bash
# Build with GLPK (default)
docker build -t glpk-api .

# Build with HiGHS support
docker build -t glpk-api --build-arg FEATURES=highs-solver .
```

### Docker with HiGHS Example

```dockerfile
FROM rust:latest

# Install cmake for HiGHS
RUN apt-get update && apt-get install -y cmake

WORKDIR /app
COPY . .

# Build with HiGHS support
RUN cargo build --release --features highs-solver

# Set solver at runtime
ENV SOLVER=highs
CMD ["./target/release/rust-glpk"]
```

### For different CPU architecture

```bash
docker build --platform=linux/amd64 -t glpk-api .
```

### Push to registry

```bash
docker push myregistry.com/glpk-api
```

#### Pushing to OurStudio Dockerhub

The image is available on [Our Studio's Dockerhub account](https://hub.docker.com/r/ourstudio/rust-glpk-api).

To build and push a new version:

1. Login to the ourstudio account

```
$ docker login
```

Username: ourstudio
Password: {in Bitwarden}

2. Build and push the new image

```
docker buildx create --use
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t ourstudio/rust-glpk-api:x.y.z \
  -t ourstudio/rust-glpk-api:x.y \
  -t ourstudio/rust-glpk-api:latest \
  --push .
```

Where `x`, `y` and `z` are major, minor and patch version numbers.

3. Verify that the new image is available [here](https://hub.docker.com/r/ourstudio/rust-glpk-api/tags).

## 🧪 Testing

Run the integration tests:

```bash
# Test with GLPK
cargo test

# Test with HiGHS
cargo test --features highs-solver
```

Or test manually with the included script:

```bash
./test.sh
```

## 🔧 Matrix Format

The API uses **sparse matrix format** for efficiency:

- `rows` - Array of row indices (0-based)
- `cols` - Array of column indices (0-based)  
- `vals` - Array of values at those positions
- `shape` - Matrix dimensions `{"nrows": N, "ncols": M}`

## 📋 Notes

- The API converts GE constraints (A x ≥ b) to LE constraints internally
- Variable bounds are specified as `[lower_bound, upper_bound]`
- Multiple objectives are solved independently
- Unknown variables in objectives are silently ignored

## 🔌 Adding New Solvers

To add a new solver (e.g., COIN-OR, CPLEX, Gurobi):

1. **Implement the Solver trait** in `src/domain/solvers/your_solver.rs`:
   ```rust
   use crate::domain::solver::Solver;

   pub struct YourSolver;

   impl Solver for YourSolver {
       fn solve(...) -> Result<Vec<ApiSolution>, SolveInputError> {
           // Your implementation
       }

       fn name(&self) -> &str {
           "YourSolver"
       }
   }
   ```

2. **Add to solver factory** in `src/domain/solver_factory.rs`:
   ```rust
   pub enum SolverType {
       Glpk,
       Highs,
       YourSolver,  // Add here
   }

   // Update from_str and create_solver
   ```

3. **Export from module** in `src/domain/solvers/mod.rs`:
   ```rust
   pub mod your_solver;
   pub use your_solver::YourSolver;
   ```

4. **Use environment variable**:
   ```bash
   SOLVER=yoursolver cargo run
   ```

## 🐛 Troubleshooting

### "cmake not found" error
Install cmake (see Prerequisites above) or use GLPK instead:
```bash
cargo build  # Uses GLPK by default
```

### Feature flag not recognized
Make sure you're using the correct flag:
```bash
cargo build --features highs-solver  # Correct
cargo build --features highs          # Wrong
```

### Solver not switching
Verify the environment variable is set:
```bash
echo $SOLVER
SOLVER=highs cargo run --features highs-solver
```

## ⚠️ Current Limitations

- HiGHS requires `cmake` at build time
- Only one solver can be active per server instance
- Solver selection happens at server startup (not per-request)

## 🔗 References

- [Docker's Rust guide](https://docs.docker.com/language/rust/)
- [GLPK Documentation](https://www.gnu.org/software/glpk/)
- [HiGHS Documentation](https://highs.dev/)
- [Linear Programming on Wikipedia](https://en.wikipedia.org/wiki/Linear_programming)