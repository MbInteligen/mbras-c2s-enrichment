pub mod api;
pub mod core;
pub mod data;
pub mod integrations;
pub mod obs;

// Re-export primary modules for shared use in tests and other binaries
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
