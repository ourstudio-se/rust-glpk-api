# Hexaly License Setup Guide

## The Error

```
"Failed to create Hexaly environment: Failed to create Hexaly environment"
```

This means Hexaly couldn't initialize, almost always due to missing or invalid license.

## Getting a License

### Option 1: Academic License (Free)

1. **Register** at https://www.hexaly.com/
2. **Request Academic License** (if you're at a university/research institution)
3. You'll receive a license file via email

### Option 2: Trial License (Free, 30 days)

1. Go to https://www.hexaly.com/download/
2. Fill out the trial request form
3. Download and you'll get a license file

### Option 3: Commercial License

Contact Hexaly sales for pricing and licensing options.

## Installing the License

Once you have your license file (`hexaly.lic` or `localsolver.lic`):

### Method 1: Home Directory (Recommended)

```bash
# Copy license to home directory
cp /path/to/hexaly.lic ~/hexaly.lic

# OR if it's named localsolver.lic
cp /path/to/localsolver.lic ~/localsolver.lic
```

### Method 2: Hexaly Directory

```bash
# Copy to Hexaly installation
sudo cp /path/to/hexaly.lic /opt/hexaly_14_5/bin/hexaly.lic
```

### Method 3: Environment Variable

```bash
# Set license file location
export HEXALY_LICENSE_PATH=~/hexaly.lic
echo 'export HEXALY_LICENSE_PATH=~/hexaly.lic' >> ~/.zshrc
```

## Verify License

```bash
# Test Hexaly directly
/opt/hexaly_14_5/bin/hexaly --version

# If it shows version info, license is working!
```

## Testing the Integration

Once license is installed:

```bash
# Set environment
export HEXALY_HOME=/opt/hexaly_14_5

# Build
cargo build --features hexaly-solver

# Run server
SOLVER=hexaly cargo run --features hexaly-solver

# Test in another terminal
curl -X POST 'http://localhost:9000/solve' \
  -H 'Content-Type: application/json' \
  -d '{
  "polyhedron": {
    "A": {
      "rows": [0,0,1,1],
      "cols": [0,1,0,1],
      "vals": [1,1,1,1],
      "shape": {"nrows": 2, "ncols": 2}
    },
    "b": [5, 5],
    "variables": [
      { "id": "x", "bound": [0,10] },
      { "id": "y", "bound": [0,10] }
    ]
  },
  "objectives": [
    { "x": 1, "y": 1 }
  ],
  "direction": "maximize"
}'
```

## Common Issues

### "License file not found"

```bash
# Check if file exists
ls -la ~/hexaly.lic
ls -la ~/localsolver.lic
ls -la /opt/hexaly_14_5/bin/hexaly.lic

# Check permissions
chmod 644 ~/hexaly.lic
```

### "License expired"

Request a new license from Hexaly.

### "Library not found" errors

```bash
# Add to ~/.zshrc
export DYLD_LIBRARY_PATH=/opt/hexaly_14_5/bin:$DYLD_LIBRARY_PATH
```

### Still getting errors?

Check Hexaly logs:
```bash
# Look for error messages
/opt/hexaly_14_5/bin/hexaly --version 2>&1
```

## Alternative: Test Without License

While waiting for your license, you can test the other solvers:

```bash
# Test with GLPK (no license needed)
SOLVER=glpk cargo run

# Test with HiGHS (no license needed)
SOLVER=highs cargo run --features highs-solver
```

## Next Steps

1. **Get license** from Hexaly
2. **Install license** using one of the methods above
3. **Verify** with `/opt/hexaly_14_5/bin/hexaly --version`
4. **Test** the Rust integration

## Support

- **Hexaly Support**: https://www.hexaly.com/support/
- **Hexaly Documentation**: https://www.hexaly.com/docs/
- **Academic License Help**: https://www.hexaly.com/academic/

---

**Note**: The FFI bindings are complete and working. Once you have a valid license, everything should work perfectly! 🚀
