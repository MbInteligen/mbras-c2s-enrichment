use hex;
use sha2::{Digest, Sha256};

/// Validates cached data integrity using SHA-256 checksums
///
/// This module provides protection against cache poisoning by:
/// 1. Generating a checksum when data is cached
/// 2. Validating the checksum when data is retrieved
/// 3. Rejecting corrupted or tampered data
///
/// # Security Model
///
/// - Uses SHA-256 for cryptographic hash generation
/// - Stores checksum alongside cached data
/// - Validates on retrieval to detect tampering
/// - Falls back to fresh fetch if validation fails

/// Wrapper for cached data with integrity validation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidatedCacheEntry {
    /// The actual cached data (JSON string)
    pub data: String,
    /// SHA-256 checksum of the data (hex encoded)
    pub checksum: String,
}

impl ValidatedCacheEntry {
    /// Creates a new validated cache entry with computed checksum
    ///
    /// # Example
    ///
    /// ```rust
    /// let entry = ValidatedCacheEntry::new(r#"{"name": "John"}"#.to_string());
    /// cache.insert(key, entry.serialize()).await;
    /// ```
    pub fn new(data: String) -> Self {
        let checksum = Self::compute_checksum(&data);
        Self { data, checksum }
    }

    /// Computes SHA-256 checksum of the data
    fn compute_checksum(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Validates the integrity of the cached data
    ///
    /// Returns true if the checksum matches, false if tampered
    pub fn is_valid(&self) -> bool {
        let computed = Self::compute_checksum(&self.data);
        computed == self.checksum
    }

    /// Serializes the entry for storage in cache
    ///
    /// Returns JSON string with both data and checksum
    pub fn serialize(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserializes and validates a cache entry
    ///
    /// Returns Some(data) if valid, None if corrupted or invalid JSON
    ///
    /// # Example
    ///
    /// ```rust
    /// if let Some(cached) = cache.get(&key).await {
    ///     if let Some(valid_data) = ValidatedCacheEntry::deserialize_and_validate(&cached) {
    ///         // Use valid_data safely
    ///     } else {
    ///         // Cache poisoned, refetch from source
    ///     }
    /// }
    /// ```
    pub fn deserialize_and_validate(serialized: &str) -> Option<String> {
        let entry: ValidatedCacheEntry = serde_json::from_str(serialized).ok()?;

        if entry.is_valid() {
            Some(entry.data)
        } else {
            // Checksum mismatch - cache poisoned
            tracing::warn!(
                "Cache validation failed: checksum mismatch. Expected: {}, Data length: {}",
                entry.checksum,
                entry.data.len()
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_validation() {
        let data = r#"{"name": "John", "age": 30}"#.to_string();
        let entry = ValidatedCacheEntry::new(data.clone());

        assert!(entry.is_valid());
        assert_eq!(entry.data, data);
    }

    #[test]
    fn test_serialize_deserialize() {
        let data = r#"{"test": "data"}"#.to_string();
        let entry = ValidatedCacheEntry::new(data.clone());

        let serialized = entry.serialize();
        let deserialized = ValidatedCacheEntry::deserialize_and_validate(&serialized);

        assert_eq!(deserialized, Some(data));
    }

    #[test]
    fn test_tampered_data_rejected() {
        let data = r#"{"original": "data"}"#.to_string();
        let entry = ValidatedCacheEntry::new(data);

        // Tamper with the data
        let mut tampered = entry;
        tampered.data = r#"{"tampered": "data"}"#.to_string();

        assert!(!tampered.is_valid());
    }

    #[test]
    fn test_tampered_cache_returns_none() {
        let data = r#"{"original": "data"}"#.to_string();
        let entry = ValidatedCacheEntry::new(data);

        let serialized = entry.serialize();

        // Manually tamper with the serialized data
        let tampered = serialized.replace("original", "hacked");

        let result = ValidatedCacheEntry::deserialize_and_validate(&tampered);
        assert_eq!(result, None);
    }

    #[test]
    fn test_checksum_consistency() {
        let data = "test data".to_string();
        let entry1 = ValidatedCacheEntry::new(data.clone());
        let entry2 = ValidatedCacheEntry::new(data);

        assert_eq!(entry1.checksum, entry2.checksum);
    }
}
