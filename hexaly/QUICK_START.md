# Hexaly Quick Start

## TL;DR

```bash
# 1. Install Hexaly from https://www.hexaly.com/
# 2. Set environment variable
export HEXALY_HOME=/path/to/hexaly

# 3. Build and run
cargo build --features hexaly-solver
SOLVER=hexaly cargo run --features hexaly-solver
```

## Automated Setup

```bash
./setup-hexaly.sh
```

## Manual Setup

### macOS

```bash
# Set environment (adjust path for your version)
export HEXALY_HOME=/Applications/hexaly_13_0
echo 'export HEXALY_HOME=/Applications/hexaly_13_0' >> ~/.zshrc

# Build
cargo build --features hexaly-solver

# Run
SOLVER=hexaly cargo run --features hexaly-solver
```

### Linux

```bash
# Set environment (adjust path for your version)
export HEXALY_HOME=/opt/hexaly_13_0
echo 'export HEXALY_HOME=/opt/hexaly_13_0' >> ~/.bashrc

# Build
cargo build --features hexaly-solver

# Run
SOLVER=hexaly cargo run --features hexaly-solver
```

## Verify Installation

```bash
# Check Hexaly installation
ls -la $HEXALY_HOME/include/localsolver.h
ls -la $HEXALY_HOME/bin/

# Test build
cargo build --features hexaly-solver

# Run tests
cargo test --features hexaly-solver
```

## Common Issues

### "HEXALY_HOME not set"

```bash
export HEXALY_HOME=/path/to/hexaly
```

### "Include directory not found"

Verify Hexaly is installed correctly:
```bash
ls -la $HEXALY_HOME/
```

Should contain:
- `bin/` - Shared libraries
- `include/` - Header files

### "Library not loaded" (Runtime)

**macOS:**
```bash
export DYLD_LIBRARY_PATH=$HEXALY_HOME/bin:$DYLD_LIBRARY_PATH
```

**Linux:**
```bash
export LD_LIBRARY_PATH=$HEXALY_HOME/bin:$LD_LIBRARY_PATH
```

## API Usage

```bash
# Start server
HEXALY_HOME=/path/to/hexaly SOLVER=hexaly cargo run --features hexaly-solver

# Solve a problem
curl -X POST 'http://localhost:9000/solve' \
  -H 'Content-Type: application/json' \
  -d @problem.json
```

## Need Help?

See detailed documentation:
- [hexaly/README.md](README.md) - Full documentation
- [../HEXALY_INTEGRATION.md](../HEXALY_INTEGRATION.md) - Integration guide
- [../README.md](../README.md) - Main project README

## Key Files

```
hexaly-sys/
├── hexaly_wrapper.h      # C API header
├── hexaly_wrapper.cpp    # C++ wrapper implementation
├── build.rs              # Build script
└── src/lib.rs            # Generated FFI bindings

hexaly/
└── src/lib.rs            # Safe Rust wrapper

src/domain/solvers/
└── hexaly_solver.rs      # Solver implementation
```

## Performance Tips

Edit `src/domain/solvers/hexaly_solver.rs`:

```rust
// Set time limit (default: unlimited)
param.set_time_limit(60);  // 60 seconds

// Set threads (default: auto)
param.set_nb_threads(4);   // 4 threads

// Enable verbose output for debugging
param.set_verbosity(2);    // 0=quiet, 1=normal, 2=verbose
```
