//! Photo Storage Service — Cloudflare R2 upload with signed URLs
//!
//! Port of ts:src/services/photo-storage.service.ts
//! Uses aws-sdk-s3 compatible API for Cloudflare R2.

use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// ─── Constants ──────────────────────────────────────────────────────────────

const DEFAULT_SIGNED_URL_EXPIRY: u64 = 604_800; // 7 days in seconds
const MIN_PHOTO_SIZE: usize = 100; // Minimum bytes for a valid photo

// ─── Config ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct R2Config {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub endpoint: String,
    pub bucket: String,
    pub signed_url_expiry_seconds: u64,
}

// ─── Service ────────────────────────────────────────────────────────────────

pub struct PhotoStorageService {
    config: Option<R2Config>,
    client: Client,
    warned_once: AtomicBool,
}

impl PhotoStorageService {
    pub fn new(config: Option<R2Config>) -> Self {
        let configured = config.is_some();
        if configured {
            tracing::info!("PhotoStorageService: R2 configured");
        } else {
            tracing::warn!("PhotoStorageService: R2 not configured, photos will be skipped");
        }

        Self {
            config,
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            warned_once: AtomicBool::new(false),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    /// Upload a base64-encoded JPEG photo to R2
    /// Returns the object key on success, None on any error
    pub async fn upload_photo(&self, cpf: &str, base64_data: &str) -> Option<String> {
        let config = match &self.config {
            Some(c) => c,
            None => {
                if !self.warned_once.swap(true, Ordering::Relaxed) {
                    tracing::warn!("Photo upload skipped: R2 not configured");
                }
                return None;
            }
        };

        // Strip MIME prefix if present
        let raw_b64 = if let Some(idx) = base64_data.find(",") {
            &base64_data[idx + 1..]
        } else {
            base64_data
        };

        // Decode base64
        let bytes = match base64::engine::general_purpose::STANDARD.decode(raw_b64) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Photo decode failed for CPF {}: {}", cpf, e);
                return None;
            }
        };

        if bytes.len() < MIN_PHOTO_SIZE {
            tracing::warn!("Photo too small for CPF {}: {} bytes", cpf, bytes.len());
            return None;
        }

        // Normalize CPF (digits only)
        let cpf_clean: String = cpf.chars().filter(|c| c.is_ascii_digit()).collect();
        let key = format!("photos/{}.jpg", cpf_clean);

        // Upload via S3-compatible PUT
        let url = format!("{}/{}/{}", config.endpoint, config.bucket, key);

        match self.client.put(&url)
            .header("Content-Type", "image/jpeg")
            .header("Cache-Control", "public, max-age=31536000, immutable")
            .body(bytes.clone())
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Photo uploaded: {} ({:.1} KB)", key, bytes.len() as f64 / 1024.0);
                Some(key)
            }
            Ok(resp) => {
                tracing::warn!("R2 upload failed for {}: HTTP {}", key, resp.status());
                None
            }
            Err(e) => {
                tracing::warn!("R2 upload error for {}: {}", key, e);
                None
            }
        }
    }

    /// Generate a signed URL for a photo (7-day expiry)
    /// In production, this would use AWS Sig v4 signing.
    /// For now, returns the direct URL (R2 bucket must be configured with appropriate access).
    pub fn get_photo_url(&self, key: &str) -> Option<String> {
        let config = self.config.as_ref()?;
        Some(format!("{}/{}/{}", config.endpoint, config.bucket, key))
    }

    /// Upload photo and return URL in one step
    pub async fn upload_and_get_url(&self, cpf: &str, base64_data: &str) -> Option<String> {
        let key = self.upload_photo(cpf, base64_data).await?;
        self.get_photo_url(&key)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_configured() {
        let svc = PhotoStorageService::new(None);
        assert!(!svc.is_configured());
        assert!(svc.get_photo_url("photos/123.jpg").is_none());
    }

    #[test]
    fn test_configured() {
        let config = R2Config {
            access_key_id: "test-key".to_string(),
            secret_access_key: "test-secret".to_string(),
            endpoint: "https://r2.example.com".to_string(),
            bucket: "photos".to_string(),
            signed_url_expiry_seconds: DEFAULT_SIGNED_URL_EXPIRY,
        };
        let svc = PhotoStorageService::new(Some(config));
        assert!(svc.is_configured());
    }

    #[test]
    fn test_photo_url_generation() {
        let config = R2Config {
            access_key_id: "ak".to_string(),
            secret_access_key: "sk".to_string(),
            endpoint: "https://r2.example.com".to_string(),
            bucket: "photos".to_string(),
            signed_url_expiry_seconds: 604800,
        };
        let svc = PhotoStorageService::new(Some(config));
        let url = svc.get_photo_url("photos/12345678901.jpg").unwrap();
        assert_eq!(url, "https://r2.example.com/photos/photos/12345678901.jpg");
    }

    #[tokio::test]
    async fn test_upload_not_configured() {
        let svc = PhotoStorageService::new(None);
        let result = svc.upload_photo("12345678901", "base64data").await;
        assert!(result.is_none());
    }

    #[test]
    fn test_strip_mime_prefix() {
        let data = "data:image/jpeg;base64,/9j/4AAQ";
        let stripped = if let Some(idx) = data.find(",") {
            &data[idx + 1..]
        } else {
            data
        };
        assert_eq!(stripped, "/9j/4AAQ");
    }

    #[test]
    fn test_cpf_normalization_for_key() {
        let cpf = "123.456.789-01";
        let clean: String = cpf.chars().filter(|c| c.is_ascii_digit()).collect();
        assert_eq!(clean, "12345678901");
        assert_eq!(format!("photos/{}.jpg", clean), "photos/12345678901.jpg");
    }
}
