# GLPK Rust API

A simple REST API for solving linear programming problems using the GLPK (GNU Linear Programming Kit) library.

## 🚀 Quick Start

### Running Locally

```bash
cargo run
```

Your application will be available at http://localhost:9000.

### Using Docker

```bash
docker compose up --build
```

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

### Using .env file

Create a `.env` file in the project root:

```
PORT=8080
JSON_PAYLOAD_LIMIT=5242880
```

## 🐳 Deploying with Docker

### Build the image

```bash
docker build -t glpk-api .
```

### For different CPU architecture

```bash
docker build --platform=linux/amd64 -t glpk-api .
```

### Push to registry

```bash
docker push myregistry.com/glpk-api
```

## 🧪 Testing

Run the integration tests:

```bash
cargo test
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

## 🔗 References

- [Docker's Rust guide](https://docs.docker.com/language/rust/)
- [GLPK Documentation](https://www.gnu.org/software/glpk/)
- [Linear Programming on Wikipedia](https://en.wikipedia.org/wiki/Linear_programming)