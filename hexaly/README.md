# Hexaly Rust Bindings

Safe Rust bindings for Hexaly (formerly LocalSolver) optimization library.

## Overview

This directory contains two crates that provide Rust bindings to Hexaly:

- **hexaly-sys**: Low-level FFI bindings to the Hexaly C++ API
- **hexaly**: Safe, idiomatic Rust wrapper around hexaly-sys

## Prerequisites

### 1. Install Hexaly

You need to have Hexaly (LocalSolver) installed on your system. Download it from:
https://www.hexaly.com/

### 2. Set Environment Variable

Set the `HEXALY_HOME` or `LOCALSOLVER_HOME` environment variable to point to your Hexaly installation:

```bash
# macOS/Linux
export HEXALY_HOME=/path/to/hexaly
# or
export LOCALSOLVER_HOME=/path/to/localsolver

# Add to your shell profile (~/.bashrc, ~/.zshrc, etc.)
echo 'export HEXALY_HOME=/path/to/hexaly' >> ~/.zshrc
```

On macOS, the typical installation path might be:
```bash
export HEXALY_HOME=/Applications/hexaly_13_0
```

On Linux:
```bash
export HEXALY_HOME=/opt/hexaly_13_0
```

### 3. Verify Installation

The Hexaly installation should have the following structure:
```
$HEXALY_HOME/
├── bin/           # Shared libraries (liblocalsolver.dylib on macOS, .so on Linux)
└── include/       # C++ header files
    └── localsolver.h
```

## Building

Once Hexaly is installed and the environment variable is set:

```bash
# Build with Hexaly solver support
cargo build --features hexaly-solver

# Build all solvers
cargo build --features highs-solver,gurobi-solver,hexaly-solver
```

## Usage

The solver is automatically available when built with the `hexaly-solver` feature.

### Via HTTP API

```bash
# Start the server
cargo run --features hexaly-solver

# Use Hexaly as solver
curl -X POST http://localhost:8080/solve?solver=hexaly \
  -H "Content-Type: application/json" \
  -d @problem.json
```

### Programmatically

```rust
use rust_glpk::domain::solver_factory::{SolverType, create_solver};

let solver = create_solver(SolverType::Hexaly);
let solutions = solver.solve(&polyhedron, &objectives, direction, use_presolve)?;
```

## Architecture

### hexaly-sys (FFI Layer)

The `hexaly-sys` crate provides:

1. **C Wrapper** (`hexaly_wrapper.h/cpp`): A C-compatible wrapper around Hexaly's C++ API
2. **Build Script** (`build.rs`): Compiles the C++ wrapper and generates Rust bindings using bindgen
3. **Raw FFI Bindings** (`src/lib.rs`): Unsafe Rust bindings to the C wrapper

### hexaly (Safe Wrapper)

The `hexaly` crate provides:

- `LocalSolver`: Main solver environment
- `Model`: Model builder for creating optimization problems
- `Expression`: Decision variables and expressions
- `Param`: Configuration parameters
- `State`: Solver state enum

Safe, idiomatic Rust API that manages memory and provides type safety.

## Platform Support

- **macOS**: Fully supported (tested on Apple Silicon and Intel)
- **Linux**: Fully supported (x86_64)
- **Windows**: Supported (may need library name adjustment in build.rs)

## Troubleshooting

### "HEXALY_HOME environment variable must be set"

Make sure you've exported the environment variable:
```bash
export HEXALY_HOME=/path/to/hexaly
cargo build --features hexaly-solver
```

### "Hexaly include directory not found"

Verify that `$HEXALY_HOME/include/localsolver.h` exists.

### Linker errors on macOS

If you get dylib loading errors at runtime, ensure the library path is in your dynamic linker search path:
```bash
export DYLD_LIBRARY_PATH=$HEXALY_HOME/bin:$DYLD_LIBRARY_PATH
```

Or use the rpath (already configured in build.rs):
```bash
cargo build --features hexaly-solver
# The rpath is automatically set during compilation
```

### Linker errors on Linux

```bash
export LD_LIBRARY_PATH=$HEXALY_HOME/bin:$LD_LIBRARY_PATH
```

## License

The Hexaly bindings are provided under MIT OR Apache-2.0. Note that Hexaly itself is proprietary software and requires a license from Hexaly Optimizer.

## Development

### Running Tests

```bash
# Test the FFI layer
cd hexaly-sys
cargo test

# Test the safe wrapper
cd hexaly
cargo test

# Test the solver integration
cd ..
cargo test --features hexaly-solver
```

### Debugging

To enable verbose output from Hexaly:

```rust
let param = ls.param();
param.set_verbosity(2); // 0=quiet, 1=normal, 2=verbose
```

## References

- [Hexaly Documentation](https://www.hexaly.com/docs/)
- [Hexaly C++ API Reference](https://www.hexaly.com/docs/last/cpp/)
