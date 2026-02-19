#!/bin/bash

# Setup script for Hexaly integration
# This script helps configure the environment for building with Hexaly support

set -e

echo "=== Hexaly Setup Script ==="
echo ""

# Check if HEXALY_HOME or LOCALSOLVER_HOME is already set
if [ -n "$HEXALY_HOME" ]; then
    HEXALY_PATH="$HEXALY_HOME"
    echo "✓ HEXALY_HOME is set to: $HEXALY_PATH"
elif [ -n "$LOCALSOLVER_HOME" ]; then
    HEXALY_PATH="$LOCALSOLVER_HOME"
    echo "✓ LOCALSOLVER_HOME is set to: $HEXALY_PATH"
else
    echo "⚠ HEXALY_HOME or LOCALSOLVER_HOME environment variable is not set"
    echo ""
    echo "Please enter the path to your Hexaly installation:"
    echo "(e.g., /Applications/hexaly_13_0 or /opt/hexaly_13_0)"
    read -r HEXALY_PATH

    if [ ! -d "$HEXALY_PATH" ]; then
        echo "✗ Directory not found: $HEXALY_PATH"
        exit 1
    fi

    export HEXALY_HOME="$HEXALY_PATH"
fi

# Verify installation structure
echo ""
echo "Verifying Hexaly installation..."

if [ ! -d "$HEXALY_PATH/include" ]; then
    echo "✗ Include directory not found: $HEXALY_PATH/include"
    echo "  Please ensure Hexaly is properly installed"
    exit 1
fi

if [ ! -d "$HEXALY_PATH/bin" ]; then
    echo "✗ Bin directory not found: $HEXALY_PATH/bin"
    echo "  Please ensure Hexaly is properly installed"
    exit 1
fi

if [ ! -f "$HEXALY_PATH/include/localsolver.h" ]; then
    echo "✗ Header file not found: $HEXALY_PATH/include/localsolver.h"
    echo "  Please ensure Hexaly is properly installed"
    exit 1
fi

echo "✓ Include directory found"
echo "✓ Bin directory found"
echo "✓ Header files found"

# Check for library files
echo ""
echo "Checking for library files..."

if [ "$(uname)" == "Darwin" ]; then
    # macOS
    if ls "$HEXALY_PATH/bin/"*.dylib 1> /dev/null 2>&1; then
        echo "✓ Found library files (macOS):"
        ls "$HEXALY_PATH/bin/"*.dylib | head -3
    else
        echo "⚠ Warning: No .dylib files found in $HEXALY_PATH/bin"
    fi
elif [ "$(uname)" == "Linux" ]; then
    # Linux
    if ls "$HEXALY_PATH/bin/"*.so 1> /dev/null 2>&1; then
        echo "✓ Found library files (Linux):"
        ls "$HEXALY_PATH/bin/"*.so | head -3
    else
        echo "⚠ Warning: No .so files found in $HEXALY_PATH/bin"
    fi
fi

# Generate shell configuration
echo ""
echo "Environment configuration:"
echo ""
echo "export HEXALY_HOME=\"$HEXALY_PATH\""

if [ "$(uname)" == "Darwin" ]; then
    echo "export DYLD_LIBRARY_PATH=\"\$HEXALY_HOME/bin:\$DYLD_LIBRARY_PATH\""
elif [ "$(uname)" == "Linux" ]; then
    echo "export LD_LIBRARY_PATH=\"\$HEXALY_HOME/bin:\$LD_LIBRARY_PATH\""
fi

echo ""
echo "Add these lines to your shell profile (~/.bashrc, ~/.zshrc, etc.) to make them permanent"
echo ""

# Offer to build
echo "Ready to build with Hexaly support."
echo ""
read -p "Build now with Hexaly solver? (y/n) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Building with Hexaly solver..."
    cargo build --features hexaly-solver

    if [ $? -eq 0 ]; then
        echo ""
        echo "✓ Build successful!"
        echo ""
        echo "You can now use Hexaly as a solver:"
        echo "  cargo run --features hexaly-solver"
        echo "  curl -X POST 'http://localhost:8080/solve?solver=hexaly' ..."
    else
        echo ""
        echo "✗ Build failed. Please check the error messages above."
        exit 1
    fi
else
    echo ""
    echo "To build later, run:"
    echo "  cargo build --features hexaly-solver"
fi

echo ""
echo "=== Setup Complete ==="
