//! C2S Lead Enrichment API Library
//!
//! This library provides the core functionality for the C2S Lead Enrichment API,
//! including database connections, external API integrations (Diretrix, Work API),
//! data models, and HTTP handlers.
//!
//! # Modules
//!
//! - `api`: API definitions.
//! - `core`: Core business logic.
//! - `data`: Data access layer.
//! - `integrations`: External service integrations.
//! - `obs`: Observability and logging.
//! - `cache_validator`: Cache validation utilities.
//! - `circuit_breaker`: Circuit breaker implementation.
//! - `config`: Configuration management.
//! - `db`: Database connection and pool management.
//! - `db_storage`: Database storage operations.
//! - `enrichment`: Lead enrichment logic.
//! - `errors`: Error handling types.
//! - `gateway_client`: C2S API client.
//! - `google_ads_handler`: Google Ads webhook handler.
//! - `google_ads_models`: Google Ads data models.
//! - `handlers`: HTTP request handlers.
//! - `models`: Core data models.
//! - `services`: External service clients (Diretrix, Work API).
//! - `webhook_handler`: C2S webhook handler.
//! - `webhook_models`: Webhook payload models.

pub mod api;
pub mod core;
pub mod data;
pub mod integrations;
pub mod obs;

// Re-export primary modules for shared use in tests and other binaries
pub mod cache_validator;
pub mod circuit_breaker;
pub mod config;
pub mod db;
pub mod db_storage;
pub mod enrichment;
pub mod errors;
pub mod gateway_client;
pub mod google_ads_handler;
pub mod google_ads_models;
pub mod handlers;
pub mod models;
pub mod services;
pub mod webhook_handler;
pub mod webhook_models;
