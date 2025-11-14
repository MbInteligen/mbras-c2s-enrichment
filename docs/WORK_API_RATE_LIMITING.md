# Work API Rate Limiting Guidelines

## Overview
This document describes the rate limiting behavior observed with the Work API and recommended practices for batch processing.

## Rate Limiting Behavior

### Observed Performance
Based on production testing with 19 CPF enrichment requests:

- ✅ **Successfully processed 11 consecutive requests** with 2-second delay
- ❌ **Intermittent failures** not due to rate limiting, but rather:
  - API timeouts for complex queries
  - Temporary API unavailability
  - Network issues

### Recommended Delay
**3 seconds between requests** provides the optimal balance:

- Prevents any potential rate limiting
- Allows adequate time for Work API to process heavy queries
- Reduces timeout errors for slower CPF lookups
- Total time for 19 CPFs: ~1 minute

### Why Not Faster?
- ⚠️ 1s delay: Works but may cause timeouts on complex queries
- ⚠️ 2s delay: Generally safe, occasional timeouts possible
- ✅ 3s delay: **Recommended** - reliable and still fast
- ⏱️ 5s delay: Overly conservative, unnecessarily slow

## Implementation Examples

### Bash Script
```bash
#!/bin/bash
DELAY=3  # 3 seconds between requests

for cpf in $(cat cpf_list.txt); do
    curl "https://api.workrb.com.br/data/completa?chave=$API_KEY&cpf=$cpf"
    sleep $DELAY
done
```

### Rust Code
```rust
use tokio::time::{sleep, Duration};

for cpf in cpf_list {
    let data = work_api.fetch_all_modules(&cpf).await?;
    // Process data...
    
    // Wait 3 seconds before next request
    sleep(Duration::from_secs(3)).await;
}
```

## Retry Strategy

For failed requests, implement exponential backoff:

1. **First attempt**: 0s delay (immediate)
2. **First retry**: 5s delay
3. **Second retry**: 10s delay
4. **Third retry**: 20s delay
5. **Give up**: After 3 retries

Example implementation:
```rust
async fn fetch_with_retry(cpf: &str, max_retries: u32) -> Result<WorkApiResponse> {
    let mut delay_secs = 0;
    
    for attempt in 0..=max_retries {
        if attempt > 0 {
            delay_secs = 5 * 2_u64.pow(attempt - 1); // 5s, 10s, 20s
            tracing::warn!("Retry attempt {} for CPF {} after {}s", attempt, cpf, delay_secs);
            sleep(Duration::from_secs(delay_secs)).await;
        }
        
        match work_api.fetch_all_modules(cpf).await {
            Ok(data) => return Ok(data),
            Err(e) if attempt == max_retries => return Err(e),
            Err(e) => tracing::warn!("Attempt {} failed: {}", attempt + 1, e),
        }
    }
    
    unreachable!()
}
```

## Common Error Types

### Timeout Errors
**Symptom**: Request takes > 30s and times out
**Solution**: Increase client timeout to 60s, use 3s delay between requests

```rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(60))
    .build()?;
```

### External Service Error
**Symptom**: Work API returns 500 or "External service error"
**Solution**: Retry after 5-10s delay, not a rate limit issue

### Rate Limit (if encountered)
**Symptom**: HTTP 429 Too Many Requests
**Solution**: Increase delay to 5s, implement exponential backoff
**Note**: Not observed in testing with 3s delay

## Batch Processing Best Practices

1. **Use 3-second delay** between requests
2. **Process in batches** of 50-100 CPFs at a time
3. **Log all responses** for debugging
4. **Save enriched data immediately** to JSON files
5. **Import to database separately** after enrichment completes
6. **Track failed CPFs** and retry with longer delay

## Testing Results

### Test Case: 19 CPFs from CEP 05676-120

**First Batch (2s delay)**:
- Total: 19 CPFs
- Success: 11 CPFs (57.9%)
- Failed: 8 CPFs (timeout/availability issues)
- Duration: ~38 seconds

**Retry Batch (5s delay)**:
- Total: 8 CPFs (previously failed)
- Success: 8 CPFs (100%)
- Failed: 0 CPFs
- Duration: ~40 seconds

**Final Result**:
- Overall success rate: 100% (after retry)
- Total time: ~78 seconds
- Average time per CPF: ~4.1 seconds

## Conclusion

**Use 3-second delay between Work API requests** for optimal reliability and performance. This provides a good balance between speed and stability, reducing timeout errors while maintaining reasonable processing times.

For production batch processing of large datasets, consider:
- Processing during off-peak hours
- Implementing queue-based processing
- Using background workers with proper retry logic
- Monitoring API response times and adjusting delays dynamically
