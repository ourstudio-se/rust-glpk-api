#!/bin/bash
#
# Run all three solvers: GLPK and HiGHS in Docker, Gurobi natively
#
# Usage: ./run-all-solvers.sh

set -e

echo "🚀 Starting all solvers..."
echo ""

# Check if docker compose is available
if ! command -v docker &> /dev/null; then
    echo "❌ Docker is not installed or not in PATH"
    exit 1
fi

# Start GLPK and HiGHS in Docker
echo "📦 Starting GLPK (port 9000) and HiGHS (port 9001) in Docker..."
cd deploy
docker compose up -d glpk-solver highs-solver
cd ..

# Wait a moment for containers to start
sleep 2

# Check if containers are running
if docker ps | grep -q rust-solver-api && docker ps | grep -q highs; then
    echo "✅ Docker containers started successfully"
else
    echo "⚠️  Warning: Some Docker containers may not have started"
fi

echo ""
echo "🔧 Starting Gurobi solver natively (port 9002)..."

# Check if Gurobi is installed
if [ ! -d "/Library/gurobi1301/macos_universal2" ]; then
    echo "❌ Gurobi not found at /Library/gurobi1301/macos_universal2"
    echo "   Please update the GUROBI_HOME path in this script or install Gurobi"
    exit 1
fi

# Start Gurobi solver in background
PORT=9002 \
GUROBI_HOME=/Library/gurobi1301/macos_universal2 \
SOLVER=gurobi \
USE_PRESOLVE=true \
cargo run --features gurobi-solver &

GUROBI_PID=$!

# Wait for Gurobi to start
sleep 3

echo ""
echo "✅ All solvers started!"
echo ""
echo "📍 Access points:"
echo "   GLPK:   http://localhost:9000"
echo "   HiGHS:  http://localhost:9001"
echo "   Gurobi: http://localhost:9002"
echo ""
echo "🧪 Test health endpoints:"
echo "   curl http://localhost:9000/health"
echo "   curl http://localhost:9001/health"
echo "   curl http://localhost:9002/health"
echo ""
echo "🛑 To stop all solvers:"
echo "   docker compose -f deploy/compose.yaml down  # Stop Docker containers"
echo "   kill $GUROBI_PID                             # Stop Gurobi (PID: $GUROBI_PID)"
echo ""
echo "💡 Gurobi is running in the foreground. Press Ctrl+C to stop it."
echo "   (Docker containers will continue running)"
echo ""

# Wait for Gurobi process
wait $GUROBI_PID
