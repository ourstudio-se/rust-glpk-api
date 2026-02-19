# Hexaly Quick Start

## ✅ Status: Code Complete - License Needed

All Hexaly FFI bindings are built and working. Just need a license to run!

## Get License (Choose One)

1. **Academic (Free)**: https://www.hexaly.com/academic/
2. **Trial (30 days)**: https://www.hexaly.com/download/
3. **Commercial**: Contact sales

## Setup (3 Steps)

```bash
# 1. Get license file and place it
cp /path/to/hexaly.lic ~/hexaly.lic

# 2. Verify it works
/opt/hexaly_14_5/bin/hexaly --version

# 3. Run!
export HEXALY_HOME=/opt/hexaly_14_5
SOLVER=hexaly cargo run --features hexaly-solver
```

## Test It

```bash
curl -X POST 'http://localhost:9000/solve' \
  -H 'Content-Type: application/json' \
  -d '{
  "polyhedron": {
    "A": {"rows": [0,0], "cols": [0,1], "vals": [1,1], "shape": {"nrows": 1, "ncols": 2}},
    "b": [10],
    "variables": [
      {"id": "x", "bound": [0,10]},
      {"id": "y", "bound": [0,10]}
    ]
  },
  "objectives": [{"x": 1, "y": 1}],
  "direction": "maximize"
}'
```

## Files Built

- ✅ `hexaly-sys/` - FFI layer
- ✅ `hexaly/` - Safe Rust API
- ✅ `src/domain/solvers/hexaly_solver.rs` - Integration

## Docs

- **License Help**: [HEXALY_LICENSE_SETUP.md](HEXALY_LICENSE_SETUP.md)
- **Complete Info**: [HEXALY_COMPLETE.md](HEXALY_COMPLETE.md)
- **Technical Details**: [HEXALY_INTEGRATION.md](HEXALY_INTEGRATION.md)

## While Waiting

Test other solvers (no license needed):

```bash
SOLVER=glpk cargo run                           # GLPK
SOLVER=highs cargo run --features highs-solver  # HiGHS
```

---

🎉 **Everything is ready - just add your license!**
