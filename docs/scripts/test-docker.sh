#!/bin/bash
# Docker-based testing for rust-c2s-api
# Runs the app in a container with a test database

set -e

echo "🐳 Starting Docker test environment..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Load .env if it exists
if [ -f .env ]; then
    echo -e "${YELLOW}Loading environment from .env${NC}"
    export $(cat .env | grep -v '^#' | xargs)
else
    echo -e "${RED}Warning: .env file not found. Using defaults.${NC}"
    echo "Copy .env.example to .env and configure it first."
    exit 1
fi

# Clean up function
cleanup() {
    echo ""
    echo -e "${YELLOW}Cleaning up...${NC}"
    docker-compose -f docker-compose.test.yml down -v
}

# Register cleanup on exit
trap cleanup EXIT

# Start services
echo ""
echo "Step 1: Starting services..."
docker-compose -f docker-compose.test.yml up -d

echo ""
echo "Step 2: Waiting for services to be healthy..."
sleep 5

# Wait for app to be ready
max_attempts=30
attempt=0
while [ $attempt -lt $max_attempts ]; do
    if curl -s http://localhost:8081/health > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Application is ready!${NC}"
        break
    fi
    attempt=$((attempt + 1))
    echo "Waiting for app... ($attempt/$max_attempts)"
    sleep 2
done

if [ $attempt -eq $max_attempts ]; then
    echo -e "${RED}✗ Application failed to start${NC}"
    echo "Check logs with: docker-compose -f docker-compose.test.yml logs app"
    exit 1
fi

# Run tests
echo ""
echo "Step 3: Running tests..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Run the local test script against Docker container
./test-local.sh http://localhost:8081

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${GREEN}✓ Docker testing complete!${NC}"
echo ""
echo "💡 Tips:"
echo "  - View logs: docker-compose -f docker-compose.test.yml logs -f app"
echo "  - Check database: docker-compose -f docker-compose.test.yml exec postgres psql -U test_user -d rust_c2s_test"
echo "  - Keep running: Press Ctrl+C to stop and cleanup"
echo ""

# Optional: Keep services running for manual testing
read -p "Keep services running for manual testing? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Services are running. Press Ctrl+C when done."
    trap - EXIT  # Disable auto-cleanup
    docker-compose -f docker-compose.test.yml logs -f app
fi
