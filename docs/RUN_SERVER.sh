#!/bin/bash

# Quick server startup script

cd "$(dirname "$0")"

echo "🚀 Starting rust-c2s-api server..."
echo ""

# Check if .env exists
if [ ! -f .env ]; then
    echo "❌ Error: .env file not found"
    echo "Please copy .env.example to .env and fill in your credentials"
    exit 1
fi

# Load environment
export $(cat .env | grep -v '^#' | xargs)

# Check required vars
if [ -z "$WORK_API" ] || [ -z "$C2S_TOKEN" ] || [ -z "$DB_URL" ]; then
    echo "❌ Error: Missing required environment variables"
    echo "Please check your .env file"
    exit 1
fi

echo "✅ Environment loaded"
echo "   PORT: ${PORT:-3000}"
echo "   WORK_API: ${WORK_API:0:10}..."
echo "   C2S_TOKEN: ${C2S_TOKEN:0:10}..."
echo ""

# Build if needed
if [ ! -f target/release/rust-c2s-api ]; then
    echo "📦 Building release binary..."
    cargo build --release
fi

echo "🚀 Starting server on port ${PORT:-3000}..."
echo ""

# Run
./target/release/rust-c2s-api
