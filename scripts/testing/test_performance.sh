#!/bin/bash

# Performance Testing Script with Industry Benchmarks
# Compares email search performance against REST API best practices

echo "═══════════════════════════════════════════════════════════════"
echo "  Email Search Performance Test"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Industry Benchmarks (from literature)
# Source: Google Web Performance Best Practices, Amazon API Gateway docs
EXCELLENT_THRESHOLD=100   # < 100ms = Excellent (Google's target for interactive responses)
GOOD_THRESHOLD=300        # < 300ms = Good (Acceptable for database queries)
ACCEPTABLE_THRESHOLD=1000 # < 1s = Acceptable (Maximum for user engagement)
POOR_THRESHOLD=3000       # < 3s = Poor (Users start abandoning)

echo "📚 Industry Benchmarks (REST API Response Times):"
echo "   🟢 Excellent:   < ${EXCELLENT_THRESHOLD}ms   (Google Web Performance target)"
echo "   🟡 Good:        < ${GOOD_THRESHOLD}ms   (Standard database query)"
echo "   🟠 Acceptable:  < ${ACCEPTABLE_THRESHOLD}ms   (Max for user engagement)"
echo "   🔴 Poor:        < ${POOR_THRESHOLD}ms   (Users abandon)"
echo ""
echo "References:"
echo "   - Google: 'Speed is a feature' - sub-100ms for interactive elements"
echo "   - Amazon: Every 100ms delay costs 1% in sales"
echo "   - Akamai: 2 second delay = 103% bounce rate increase"
echo ""
echo "───────────────────────────────────────────────────────────────"
echo ""

# Test Configuration
TEST_EMAIL="test@example.com"
WARMUP_RUNS=3
TEST_RUNS=10

echo "🔧 Test Configuration:"
echo "   Endpoint: GET /api/v1/contributor/customer?email=${TEST_EMAIL}"
echo "   Warmup runs: ${WARMUP_RUNS}"
echo "   Test runs: ${TEST_RUNS}"
echo ""

# Warmup (to populate caches, establish connections)
echo "🔥 Warming up (${WARMUP_RUNS} requests)..."
for i in $(seq 1 $WARMUP_RUNS); do
    curl -s -o /dev/null "http://localhost:8080/api/v1/contributor/customer?email=${TEST_EMAIL}"
done
echo "   ✓ Warmup complete"
echo ""

# Performance Test
echo "⚡ Running performance test (${TEST_RUNS} requests)..."
echo ""

TIMES=()
HTTP_CODES=()
SUCCESS_COUNT=0
TOTAL_TIME=0

for i in $(seq 1 $TEST_RUNS); do
    RESPONSE=$(curl -s -w "\n%{http_code}\n%{time_total}" "http://localhost:8080/api/v1/contributor/customer?email=${TEST_EMAIL}")

    HTTP_CODE=$(echo "$RESPONSE" | tail -n 2 | head -n 1)
    TIME_TOTAL=$(echo "$RESPONSE" | tail -n 1)

    # Convert to milliseconds
    TIME_MS=$(echo "$TIME_TOTAL * 1000" | bc | cut -d. -f1)

    TIMES+=($TIME_MS)
    HTTP_CODES+=($HTTP_CODE)

    if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "404" ]; then
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
        TOTAL_TIME=$((TOTAL_TIME + TIME_MS))
    fi

    printf "   Run %2d: %3dms (HTTP %s)\n" $i $TIME_MS $HTTP_CODE
done

echo ""
echo "───────────────────────────────────────────────────────────────"
echo ""

# Calculate Statistics
if [ $SUCCESS_COUNT -gt 0 ]; then
    AVG_TIME=$((TOTAL_TIME / SUCCESS_COUNT))

    # Sort times for percentile calculation
    IFS=$'\n' SORTED_TIMES=($(sort -n <<<"${TIMES[*]}"))
    unset IFS

    MIN_TIME=${SORTED_TIMES[0]}
    MAX_TIME=${SORTED_TIMES[-1]}

    # Calculate percentiles
    P50_IDX=$((SUCCESS_COUNT * 50 / 100))
    P95_IDX=$((SUCCESS_COUNT * 95 / 100))
    P99_IDX=$((SUCCESS_COUNT * 99 / 100))

    P50=${SORTED_TIMES[$P50_IDX]}
    P95=${SORTED_TIMES[$P95_IDX]}
    P99=${SORTED_TIMES[$P99_IDX]}

    echo "📊 Performance Statistics:"
    echo ""
    echo "   Requests:  ${TEST_RUNS} total, ${SUCCESS_COUNT} successful ($(echo "scale=1; $SUCCESS_COUNT * 100 / $TEST_RUNS" | bc)%)"
    echo ""
    echo "   Min:       ${MIN_TIME}ms"
    echo "   Max:       ${MAX_TIME}ms"
    echo "   Average:   ${AVG_TIME}ms"
    echo "   Median:    ${P50}ms"
    echo "   P95:       ${P95}ms"
    echo "   P99:       ${P99}ms"
    echo ""

    # Performance Rating
    echo "───────────────────────────────────────────────────────────────"
    echo ""
    echo "🏆 Performance Rating:"
    echo ""

    if [ $AVG_TIME -lt $EXCELLENT_THRESHOLD ]; then
        RATING="🟢 EXCELLENT"
        COMPARISON="Exceeds Google's interactive response target"
        DETAILS="Your API is in the top tier of web performance. Response times are comparable to in-memory caches and CDN edge responses."
    elif [ $AVG_TIME -lt $GOOD_THRESHOLD ]; then
        RATING="🟡 GOOD"
        COMPARISON="Meets industry standards for database queries"
        DETAILS="Solid performance for a database-backed API. Comparable to well-optimized REST APIs with connection pooling and query optimization."
    elif [ $AVG_TIME -lt $ACCEPTABLE_THRESHOLD ]; then
        RATING="🟠 ACCEPTABLE"
        COMPARISON="Within user engagement threshold"
        DETAILS="Acceptable for backend operations, but may feel sluggish for interactive UI elements. Consider caching for frequently accessed data."
    elif [ $AVG_TIME -lt $POOR_THRESHOLD ]; then
        RATING="🔴 POOR"
        COMPARISON="Approaching user abandonment threshold"
        DETAILS="Performance needs improvement. Users may experience noticeable delays. Investigate query optimization, indexing, and caching strategies."
    else
        RATING="⚫ CRITICAL"
        COMPARISON="Exceeds user patience threshold"
        DETAILS="Critical performance issues detected. Immediate optimization required. Check for N+1 queries, missing indexes, or connection issues."
    fi

    echo "   Rating:     ${RATING}"
    echo "   Assessment: ${COMPARISON}"
    echo ""
    echo "   ${DETAILS}"
    echo ""

    # Detailed Comparison
    echo "───────────────────────────────────────────────────────────────"
    echo ""
    echo "📈 Comparison with Industry Standards:"
    echo ""

    printf "   %-20s %6dms  " "Your Average:" $AVG_TIME
    if [ $AVG_TIME -lt $EXCELLENT_THRESHOLD ]; then
        echo "✓ Excellent"
    elif [ $AVG_TIME -lt $GOOD_THRESHOLD ]; then
        echo "✓ Good"
    elif [ $AVG_TIME -lt $ACCEPTABLE_THRESHOLD ]; then
        echo "○ Acceptable"
    else
        echo "✗ Needs improvement"
    fi

    printf "   %-20s %6dms  " "Google Target:" $EXCELLENT_THRESHOLD
    DIFF=$((AVG_TIME - EXCELLENT_THRESHOLD))
    if [ $DIFF -gt 0 ]; then
        echo "(+${DIFF}ms slower)"
    else
        echo "(-$((-DIFF))ms faster) ✓"
    fi

    printf "   %-20s %6dms  " "DB Query Standard:" $GOOD_THRESHOLD
    DIFF=$((AVG_TIME - GOOD_THRESHOLD))
    if [ $DIFF -gt 0 ]; then
        echo "(+${DIFF}ms slower)"
    else
        echo "(-$((-DIFF))ms faster) ✓"
    fi

    printf "   %-20s %6dms  " "User Engagement:" $ACCEPTABLE_THRESHOLD
    DIFF=$((AVG_TIME - ACCEPTABLE_THRESHOLD))
    if [ $DIFF -gt 0 ]; then
        echo "(+${DIFF}ms slower)"
    else
        echo "(-$((-DIFF))ms faster) ✓"
    fi

    echo ""

    # Optimization Recommendations
    echo "───────────────────────────────────────────────────────────────"
    echo ""
    echo "💡 Optimization Recommendations:"
    echo ""

    if [ $AVG_TIME -ge $EXCELLENT_THRESHOLD ] && [ $AVG_TIME -lt $GOOD_THRESHOLD ]; then
        echo "   1. ✓ Query optimization: Good (likely indexed)"
        echo "   2. ✓ Connection pooling: Effective"
        echo "   3. → Add caching layer to reach sub-100ms (Redis/Memcached)"
        echo "   4. → Consider CDN for static/repeated responses"
    elif [ $AVG_TIME -ge $GOOD_THRESHOLD ] && [ $AVG_TIME -lt $ACCEPTABLE_THRESHOLD ]; then
        echo "   1. → Verify database indexes on email/phone columns"
        echo "   2. → Check connection pool configuration"
        echo "   3. → Add query result caching (Redis)"
        echo "   4. → Profile slow queries with EXPLAIN ANALYZE"
    elif [ $AVG_TIME -ge $ACCEPTABLE_THRESHOLD ]; then
        echo "   1. ⚠ CRITICAL: Check database indexes"
        echo "   2. ⚠ Review query complexity (joins, subqueries)"
        echo "   3. ⚠ Verify network latency to database"
        echo "   4. ⚠ Check connection pool exhaustion"
        echo "   5. → Implement aggressive caching strategy"
    else
        echo "   ✓ Performance is excellent - no immediate optimizations needed"
        echo "   → Consider monitoring and alerting for regression detection"
    fi

    echo ""
    echo "───────────────────────────────────────────────────────────────"
    echo ""
    echo "📚 References & Further Reading:"
    echo ""
    echo "   • Google Web Performance: https://web.dev/performance/"
    echo "   • Amazon API Gateway Best Practices"
    echo "   • 'High Performance Browser Networking' - Ilya Grigorik"
    echo "   • PostgreSQL Performance Tuning: https://wiki.postgresql.org/wiki/Performance_Optimization"
    echo ""

else
    echo "❌ All requests failed - unable to calculate statistics"
    echo ""
    echo "Last response from server:"
    curl -s "http://localhost:8080/api/v1/contributor/customer?email=${TEST_EMAIL}"
    echo ""
fi

echo "═══════════════════════════════════════════════════════════════"
echo ""
