# Linear Programming Rust API

A REST API for solving linear programming problems with support for multiple solver backends (GLPK, HiGHS, Gurobi, and Hexaly).

## 🚀 Quick Start

### Running Locally

```bash
# Using GLPK (default)
cargo run

# Using HiGHS (requires cmake and feature flag)
SOLVER=highs cargo run --features highs-solver

# Using Gurobi (requires Gurobi installation and feature flag)
GUROBI_HOME=/path/to/gurobi SOLVER=gurobi cargo run --features gurobi-solver
```

Your application will be available at http://localhost:9000.

### Using Docker Compose

**Quick Start - Run all solvers on macOS:**

```bash
# One command to start everything (recommended)
./run-all-solvers.sh
```

This script will:
- Start GLPK and HiGHS in Docker (ports 9000-9001)
- Start Gurobi natively on macOS (port 9002)

**Manual Docker Compose (GLPK + HiGHS only):**

```bash
cd deploy
docker compose up -d
```

This will start:
- **GLPK solver** on `http://localhost:9000`
- **HiGHS solver** on `http://localhost:9001`

**Running Gurobi separately on macOS:**

```bash
# Gurobi must run natively (Docker can't use macOS Gurobi binaries)
PORT=9002 \
GUROBI_HOME=/Library/gurobi1301/macos_universal2 \
SOLVER=gurobi \
cargo run --features gurobi-solver
```

**Test the solvers:**

```bash
curl http://localhost:9000/health  # GLPK
curl http://localhost:9001/health  # HiGHS
curl http://localhost:9002/health  # Gurobi
```

## 🔧 Multi-Solver Support

This API supports multiple LP/ILP solver backends through a clean abstraction layer. You can easily switch between GLPK, HiGHS, and Gurobi without changing your API or client code.

### Available Solvers

#### GLPK (Default)
- **Status**: ✅ Always available
- **Features**: Robust, battle-tested, integer programming support
- **Requirements**: None (included by default)

#### HiGHS
- **Status**: ⚠️ Optional feature (requires cmake)
- **Features**: Modern, faster for many problems, actively developed
- **Configuration**: Presolve can be controlled via `USE_PRESOLVE` environment variable (default: enabled)
- **Requirements**:
  - `cmake` must be installed
  - Enable the `highs-solver` feature flag

#### Gurobi
- **Status**: ⚠️ Optional feature (requires Gurobi license)
- **Features**: Commercial solver, highly optimized, excellent performance on large problems
- **Configuration**:
  - Console output is disabled by default for production performance
  - Automatically uses all available CPU cores for parallel optimization
  - Binary variables (bounds [0,1]) are automatically detected and optimized
  - Presolve can be controlled via `USE_PRESOLVE` environment variable (default: enabled)
- **Requirements**:
  - Gurobi must be installed locally (version 10-12 supported)
  - Valid Gurobi license
  - `GUROBI_HOME` environment variable set
  - Enable the `gurobi-solver` feature flag

#### Hexaly (LocalSolver)
- **Status**: ⚠️ Optional feature (requires Hexaly license and manual FFI setup)
- **Features**: Commercial solver with local search algorithms, handles non-linear problems, excellent for combinatorial optimization
- **Configuration**:
  - Console output is disabled by default
  - Time limits and thread count can be configured in the solver code
- **Requirements**:
  - Hexaly must be installed locally
  - Valid Hexaly license
  - `HEXALY_HOME` or `LOCALSOLVER_HOME` environment variable set
  - Enable the `hexaly-solver` feature flag
  - **Note**: Uses custom FFI bindings (see [hexaly/README.md](hexaly/README.md) for detailed setup)

### Switching Solvers

Set the `SOLVER` environment variable to choose your solver:

```bash
# Use GLPK (default)
SOLVER=glpk cargo run

# Use HiGHS (requires cmake and feature flag)
SOLVER=highs cargo run --features highs-solver

# Use Gurobi (requires Gurobi installation and feature flag)
GUROBI_HOME=/Library/gurobi1301/macos_universal2 SOLVER=gurobi cargo run --features gurobi-solver

# Use Hexaly (requires Hexaly installation and feature flag)
HEXALY_HOME=/path/to/hexaly SOLVER=hexaly cargo run --features hexaly-solver
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

### Building with Hexaly Support

#### Prerequisites

1. **Install Hexaly**:
   - Download from [Hexaly Downloads](https://www.hexaly.com/)
   - Follow the installation instructions for your platform
   - Obtain a valid license (free academic licenses available)

2. **Set HEXALY_HOME**:
   ```bash
   # macOS (example for version 13.0)
   export HEXALY_HOME=/Applications/hexaly_13_0

   # Linux
   export HEXALY_HOME=/opt/hexaly_13_0
   ```

   Add this to your shell profile (`.bashrc`, `.zshrc`, etc.) to make it permanent.

3. **Run setup script** (optional but recommended):
   ```bash
   ./setup-hexaly.sh
   ```

   This script will verify your installation and configure the build environment.

#### Build Commands

```bash
# Build with Hexaly support
HEXALY_HOME=/path/to/hexaly cargo build --features hexaly-solver

# Run tests with Hexaly
HEXALY_HOME=/path/to/hexaly cargo test --features hexaly-solver

# Run in production with Hexaly
HEXALY_HOME=/path/to/hexaly SOLVER=hexaly cargo run --release --features hexaly-solver
```

For detailed FFI setup, troubleshooting, and architecture information, see **[hexaly/README.md](hexaly/README.md)**.

### Building with Gurobi Support

#### Prerequisites

1. **Install Gurobi**:
   - Download from [Gurobi Downloads](https://www.gurobi.com/downloads/)
   - Follow the installation instructions for your platform
   - Obtain a valid license (free academic licenses available)

2. **Set GUROBI_HOME**:
   ```bash
   # macOS (example for version 13.0.1)
   export GUROBI_HOME=/Library/gurobi1301/macos_universal2

   # Linux (adjust version and path as needed)
   export GUROBI_HOME=/opt/gurobi1301/linux64

   # Windows
   set GUROBI_HOME=C:\gurobi1301\win64
   ```

   Add this to your shell profile (`.bashrc`, `.zshrc`, etc.) to make it permanent.

#### Build Commands

```bash
# Build with Gurobi support
GUROBI_HOME=/path/to/gurobi cargo build --features gurobi-solver

# Run tests with Gurobi
GUROBI_HOME=/path/to/gurobi cargo test --features gurobi-solver

# Run in production with Gurobi
GUROBI_HOME=/path/to/gurobi SOLVER=gurobi cargo run --release --features gurobi-solver
```

### Performance Comparison

To compare solver performance on your workload:

```bash
# Test with GLPK
time SOLVER=glpk ./target/release/rust-glpk

# Test with HiGHS
time SOLVER=highs ./target/release/rust-glpk

# Test with Gurobi
time GUROBI_HOME=/path/to/gurobi SOLVER=gurobi ./target/release/rust-glpk

# Test with Hexaly
time HEXALY_HOME=/path/to/hexaly SOLVER=hexaly ./target/release/rust-glpk
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
- `SOLVER` - Solver backend: `glpk` (default), `highs`, `gurobi`, or `hexaly`
- `GUROBI_HOME` - Path to Gurobi installation (required for Gurobi solver)
- `HEXALY_HOME` - Path to Hexaly installation (required for Hexaly solver)
- `USE_PRESOLVE` - Enable/disable presolve optimization: `true` (default) or `false`

### Using .env file

Create a `.env` file in the project root:

```
PORT=8080
JSON_PAYLOAD_LIMIT=5242880
SOLVER=glpk
USE_PRESOLVE=true
# For Gurobi:
# GUROBI_HOME=/Library/gurobi1301/macos_universal2
# SOLVER=gurobi
```

### ⚡ Presolve Configuration

Presolve is an optimization technique that simplifies the problem before solving by:
- Eliminating fixed variables
- Removing redundant constraints
- Tightening variable bounds
- Reducing problem size

**Benefits**: Faster solve times, especially for large problems
**Tradeoff**: May eliminate fixed variables from solution (automatically handled by returning their fixed values)

Control presolve with the `USE_PRESOLVE` environment variable:

```bash
# Enable presolve (default - recommended for production)
USE_PRESOLVE=true cargo run

# Disable presolve (useful for debugging or comparing results)
USE_PRESOLVE=false cargo run
```

**Note**: GLPK uses its own presolve configuration internally and ignores this setting.

### 🛡️ Protected mode

Enable authentication for the `POST /solve` endpoint by setting the following variables in the enviroment:

- `PROTECT=true` (default: `false`)
- `API_TOKEN=****`

When enabled, all requests to /solve must include a valid API key in a X-API-Key header.

## 🐳 Deploying with Docker

### Docker Compose (Recommended)

The easiest way to run all solvers is with Docker Compose:

```bash
cd deploy
docker compose up -d
```

This starts three services:
- **glpk-solver** on port 9000
- **highs-solver** on port 9001
- **gurobi-solver** on port 9002 (commented out by default, requires license)

Access them at:
```bash
curl http://localhost:9000/health  # GLPK
curl http://localhost:9001/health  # HiGHS
curl http://localhost:9002/health  # Gurobi (if enabled)
```

#### Enabling Gurobi in Docker

Gurobi requires additional setup due to licensing. See the detailed guide:
- **[deploy/README-GUROBI.md](deploy/README-GUROBI.md)** - Complete Gurobi Docker setup instructions

Quick steps:
1. Copy your Gurobi installation to `deploy/gurobi/`
2. Ensure your license file is accessible
3. Uncomment the `gurobi-solver` service in `deploy/compose.yaml`
4. Update the license file path in the volume mount
5. Run `docker compose up gurobi-solver`

### Building Individual Images

You can also build individual Docker images:

```bash
# Build with GLPK (default) from deploy directory
cd deploy
docker build -f Dockerfile.multi -t glpk-api:glpk ..

# Build with HiGHS support
docker build -f Dockerfile.multi -t glpk-api:highs ..

# Build with Gurobi (requires setup - see deploy/README-GUROBI.md)
docker build -f Dockerfile.gurobi -t glpk-api:gurobi ..
```

Run individual containers:

```bash
# Run GLPK solver
docker run -p 9000:9000 -e SOLVER=glpk glpk-api:glpk

# Run HiGHS solver
docker run -p 9001:9000 -e SOLVER=highs glpk-api:highs

# Run Gurobi solver (requires license mount)
docker run -p 9002:9000 \
  -e SOLVER=gurobi \
  -e GUROBI_HOME=/opt/gurobi \
  -v ${HOME}/gurobi.lic:/opt/gurobi/gurobi.lic:ro \
  glpk-api:gurobi
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

# Test with Gurobi
GUROBI_HOME=/path/to/gurobi cargo test --features gurobi-solver
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

To add a new solver (e.g., COIN-OR, CPLEX):

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
       Gurobi,
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

### Gurobi linking errors
If you get undefined symbols for Gurobi functions:
1. Verify Gurobi is installed: `ls $GUROBI_HOME/lib/`
2. Check that `GUROBI_HOME` points to the correct directory
3. Ensure your Gurobi version is 10-12 (version 13+ may have compatibility issues with the current `grb` crate)
4. Try setting the library path explicitly:
   ```bash
   export DYLD_LIBRARY_PATH=$GUROBI_HOME/lib:$DYLD_LIBRARY_PATH  # macOS
   export LD_LIBRARY_PATH=$GUROBI_HOME/lib:$LD_LIBRARY_PATH      # Linux
   ```

### "Academic license - for non-commercial use only" error
This is just an informational message from Gurobi and doesn't prevent the solver from working. If you need commercial use, upgrade your Gurobi license.

### Enabling Gurobi debug logging
By default, Gurobi's console output is disabled for better performance. To enable verbose logging for debugging:
1. Edit `src/domain/solvers/gurobi_solver.rs`
2. Change `env.set(param::OutputFlag, 0)` to `env.set(param::OutputFlag, 1)`
3. Rebuild the project

This will show detailed solver output including warnings about small coefficients, presolve statistics, and solution progress.

### Controlling Gurobi thread usage
By default, Gurobi uses all available CPU cores. To limit thread count:
1. Edit `src/domain/solvers/gurobi_solver.rs`
2. Change `env.set(param::Threads, 0)` to `env.set(param::Threads, N)` where N is your desired thread count
3. Rebuild the project

For example, setting `Threads` to 4 will use only 4 threads. Setting to 0 (default) uses all available cores automatically.

## ⚠️ Current Limitations

- HiGHS requires `cmake` at build time
- Gurobi requires a commercial or academic license
- Gurobi version 13+ may not be fully supported yet (versions 10-12 recommended)
- Only one solver can be active per server instance
- Solver selection happens at server startup (not per-request)

## 🔗 References

- [Docker's Rust guide](https://docs.docker.com/language/rust/)
- [GLPK Documentation](https://www.gnu.org/software/glpk/)
- [HiGHS Documentation](https://highs.dev/)
- [Gurobi Documentation](https://www.gurobi.com/documentation/)
- [Gurobi Rust Bindings (grb crate)](https://docs.rs/grb/)
- [Linear Programming on Wikipedia](https://en.wikipedia.org/wiki/Linear_programming)