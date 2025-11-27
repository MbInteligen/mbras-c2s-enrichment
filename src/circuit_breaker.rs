use failsafe::{backoff, failure_policy, Config};
use std::time::Duration;

/// Creates a circuit breaker for database operations to prevent cascading failures.
///
/// # Configuration
///
/// - **Failure threshold**: 5 consecutive failures triggers OPEN state.
/// - **Backoff**: Exponential backoff from 10s to 60s before attempting recovery.
///
/// # States
///
/// - **CLOSED**: Normal operation, requests pass through.
/// - **OPEN**: Too many failures, requests fail fast.
/// - **HALF_OPEN**: Testing if service recovered.
///
/// # Example
///
/// ```rust
/// use rust_c2s_api::circuit_breaker::create_db_circuit_breaker;
/// use failsafe::CircuitBreaker;
///
/// // let circuit_breaker = create_db_circuit_breaker();
/// //
/// // let result = circuit_breaker.call(async {
/// //     sqlx::query("SELECT * FROM users").fetch_all(&pool).await
/// // }).await;
/// ```
///
/// # Returns
///
/// * `impl failsafe::CircuitBreaker` - The configured circuit breaker instance.
pub fn create_db_circuit_breaker() -> impl failsafe::CircuitBreaker {
    let backoff_strategy = backoff::exponential(
        Duration::from_secs(10), // Initial delay
        Duration::from_secs(60), // Maximum delay
    );

    let failure_policy = failure_policy::consecutive_failures(5, backoff_strategy);

    Config::new().failure_policy(failure_policy).build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use failsafe::{CircuitBreaker, Error};

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let cb = create_db_circuit_breaker();

        // Simulate 5 consecutive failures
        for _ in 0..5 {
            let result: Result<(), Error<&str>> = cb.call(|| Err::<(), &str>("simulated error"));
            assert!(result.is_err());
        }

        // Next call should be rejected (circuit is open)
        let result: Result<(), Error<&str>> = cb.call(|| Ok::<(), &str>(()));

        // Should be circuit breaker rejection
        match result {
            Err(Error::Rejected) => {
                // Circuit is open, expected behavior
            }
            _ => panic!("Expected circuit to be open and reject requests"),
        }
    }

    #[test]
    fn test_circuit_breaker_allows_success() {
        let cb = create_db_circuit_breaker();

        let result: Result<i32, Error<&str>> = cb.call(|| Ok::<i32, &str>(42));

        assert_eq!(result.unwrap(), 42);
    }
}
