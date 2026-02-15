//! Fly.io Auto-Scaling Service
//!
//! Generic multi-machine auto-scaler. Scales machines up before heavy operations
//! and down after idle periods to save costs.
//!
//! Port of: ts-c2s-api/src/services/fly-scale.service.ts

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

/// Machine scale profile
#[derive(Debug, Clone)]
struct ScaleProfile {
    app_name: String,
    machine_id: Option<String>,
    api_token: String,
    up_cpu_kind: &'static str,
    up_cpus: u32,
    up_memory_mb: u32,
    down_cpu_kind: &'static str,
    down_cpus: u32,
    down_memory_mb: u32,
    idle_timeout: Duration,
    enabled: bool,
}

/// Per-machine state
struct MachineState {
    scale_in_progress: bool,
    current_scale: Option<String>, // "up" or "down"
    scale_down_handle: Option<tokio::task::JoinHandle<()>>,
}

impl MachineState {
    fn new() -> Self {
        Self {
            scale_in_progress: false,
            current_scale: None,
            scale_down_handle: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct FlyMachineConfig {
    guest: Option<FlyGuestConfig>,
}

#[derive(Debug, Deserialize)]
struct FlyGuestConfig {
    memory_mb: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FlyMachine {
    id: String,
    config: Option<FlyMachineConfig>,
}

pub struct FlyScaleService {
    profiles: HashMap<String, ScaleProfile>,
    states: Arc<Mutex<HashMap<String, MachineState>>>,
    client: reqwest::Client,
}

impl FlyScaleService {
    pub fn new(config: &crate::config::Config) -> Self {
        let mut profiles = HashMap::new();

        // CPF Lookup profile
        let fly_token = std::env::var("FLY_API_TOKEN").unwrap_or_default();
        let cpf_enabled = std::env::var("CPF_LOOKUP_AUTO_SCALE")
            .ok()
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

        profiles.insert("cpf-lookup".to_string(), ScaleProfile {
            app_name: "cpf-lookup-api".to_string(),
            machine_id: std::env::var("CPF_LOOKUP_MACHINE_ID").ok(),
            api_token: fly_token.clone(),
            up_cpu_kind: "shared",
            up_cpus: 8,
            up_memory_mb: 16384,
            down_cpu_kind: "shared",
            down_cpus: 1,
            down_memory_mb: 256,
            idle_timeout: Duration::from_secs(300), // 5 min
            enabled: cpf_enabled,
        });

        // Meilisearch profile
        let meili_token = config.meilisearch_fly_api_token
            .clone()
            .unwrap_or_else(|| fly_token.clone());

        profiles.insert("meilisearch".to_string(), ScaleProfile {
            app_name: config.meilisearch_app_name.clone(),
            machine_id: config.meilisearch_machine_id.clone(),
            api_token: meili_token,
            up_cpu_kind: "shared",
            up_cpus: 8,
            up_memory_mb: 16384,
            down_cpu_kind: "shared",
            down_cpus: 1,
            down_memory_mb: 2048,
            idle_timeout: Duration::from_secs(600), // 10 min
            enabled: config.meilisearch_auto_scale,
        });

        let mut states = HashMap::new();
        for name in profiles.keys() {
            states.insert(name.clone(), MachineState::new());
        }

        Self {
            profiles,
            states: Arc::new(Mutex::new(states)),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build reqwest client"),
        }
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.profiles.get(name).map(|p| p.enabled).unwrap_or(false)
    }

    /// Scale machine up to active config
    pub async fn scale_up(&self, name: &str) {
        let profile = match self.profiles.get(name) {
            Some(p) if p.enabled => p.clone(),
            _ => return,
        };

        let mut states = self.states.lock().await;
        let state = states.entry(name.to_string()).or_insert_with(MachineState::new);

        if state.scale_in_progress {
            tracing::debug!("Scale already in progress for {}", name);
            return;
        }

        // Cancel pending scale-down
        if let Some(handle) = state.scale_down_handle.take() {
            handle.abort();
            tracing::debug!("Cancelled pending scale-down for {}", name);
        }

        if state.current_scale.as_deref() == Some("up") {
            tracing::debug!("{} already scaled up", name);
            return;
        }

        state.scale_in_progress = true;
        drop(states);

        let machine_id = match self.get_machine_id(name, &profile).await {
            Some(id) => id,
            None => {
                let mut states = self.states.lock().await;
                if let Some(s) = states.get_mut(name) {
                    s.scale_in_progress = false;
                }
                return;
            }
        };

        tracing::info!(
            "Scaling UP {}: {}MB → {}MB",
            name,
            profile.down_memory_mb,
            profile.up_memory_mb
        );

        let url = format!(
            "https://api.machines.dev/v1/apps/{}/machines/{}/cordon",
            profile.app_name, machine_id
        );

        // Update machine config
        let update_url = format!(
            "https://api.machines.dev/v1/apps/{}/machines/{}",
            profile.app_name, machine_id
        );

        let body = serde_json::json!({
            "config": {
                "guest": {
                    "cpu_kind": profile.up_cpu_kind,
                    "cpus": profile.up_cpus,
                    "memory_mb": profile.up_memory_mb
                }
            }
        });

        match self.client
            .post(&update_url)
            .header("Authorization", format!("Bearer {}", profile.api_token))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Successfully scaled up {}", name);
                let mut states = self.states.lock().await;
                if let Some(s) = states.get_mut(name) {
                    s.current_scale = Some("up".to_string());
                    s.scale_in_progress = false;
                }
            }
            Ok(resp) => {
                tracing::error!("Failed to scale up {}: HTTP {}", name, resp.status());
                let mut states = self.states.lock().await;
                if let Some(s) = states.get_mut(name) {
                    s.scale_in_progress = false;
                }
            }
            Err(e) => {
                tracing::error!("Failed to scale up {}: {}", name, e);
                let mut states = self.states.lock().await;
                if let Some(s) = states.get_mut(name) {
                    s.scale_in_progress = false;
                }
            }
        }
    }

    /// Schedule a scale-down after idle timeout
    pub fn schedule_scale_down(&self, name: &str) {
        let profile = match self.profiles.get(name) {
            Some(p) if p.enabled => p.clone(),
            _ => return,
        };

        let states = self.states.clone();
        let client = self.client.clone();
        let name = name.to_string();
        let profiles = self.profiles.clone();

        tokio::spawn(async move {
            // Cancel any previous timer
            {
                let mut states = states.lock().await;
                if let Some(state) = states.get_mut(&name) {
                    if let Some(handle) = state.scale_down_handle.take() {
                        handle.abort();
                    }
                }
            }

            let idle = profile.idle_timeout;
            let states_inner = states.clone();
            let name_inner = name.clone();

            let handle = tokio::spawn(async move {
                tokio::time::sleep(idle).await;

                tracing::info!(
                    "Scaling DOWN {}: {}MB → {}MB (idle timeout)",
                    name_inner,
                    profile.up_memory_mb,
                    profile.down_memory_mb
                );

                // Get machine ID
                let machine_id = {
                    // Try stored machine_id first
                    profile.machine_id.clone()
                };

                if let Some(machine_id) = machine_id {
                    let update_url = format!(
                        "https://api.machines.dev/v1/apps/{}/machines/{}",
                        profile.app_name, machine_id
                    );

                    let body = serde_json::json!({
                        "config": {
                            "guest": {
                                "cpu_kind": profile.down_cpu_kind,
                                "cpus": profile.down_cpus,
                                "memory_mb": profile.down_memory_mb
                            }
                        }
                    });

                    match client
                        .post(&update_url)
                        .header("Authorization", format!("Bearer {}", profile.api_token))
                        .json(&body)
                        .send()
                        .await
                    {
                        Ok(resp) if resp.status().is_success() => {
                            tracing::info!("Successfully scaled down {}", name_inner);
                            let mut states = states_inner.lock().await;
                            if let Some(s) = states.get_mut(&name_inner) {
                                s.current_scale = Some("down".to_string());
                            }
                        }
                        Ok(resp) => {
                            tracing::error!(
                                "Failed to scale down {}: HTTP {}",
                                name_inner,
                                resp.status()
                            );
                        }
                        Err(e) => {
                            tracing::error!("Failed to scale down {}: {}", name_inner, e);
                        }
                    }
                }
            });

            let mut states = states.lock().await;
            if let Some(state) = states.get_mut(&name) {
                state.scale_down_handle = Some(handle);
            }
        });
    }

    /// Get machine ID (from config or auto-detect via API)
    async fn get_machine_id(&self, name: &str, profile: &ScaleProfile) -> Option<String> {
        if let Some(ref id) = profile.machine_id {
            return Some(id.clone());
        }

        // Auto-detect: list machines and pick the first one
        let url = format!(
            "https://api.machines.dev/v1/apps/{}/machines",
            profile.app_name
        );

        match self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", profile.api_token))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(machines) = resp.json::<Vec<FlyMachine>>().await {
                    if let Some(machine) = machines.first() {
                        tracing::info!(
                            "Auto-detected machine ID for {}: {}",
                            name,
                            machine.id
                        );
                        return Some(machine.id.clone());
                    }
                }
                tracing::warn!("No machines found for app {}", profile.app_name);
                None
            }
            Ok(resp) => {
                tracing::error!(
                    "Failed to list machines for {}: HTTP {}",
                    name,
                    resp.status()
                );
                None
            }
            Err(e) => {
                tracing::error!("Failed to list machines for {}: {}", name, e);
                None
            }
        }
    }
}
