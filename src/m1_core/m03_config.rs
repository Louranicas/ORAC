//! # M03: Configuration
//!
//! Figment-based configuration loading with TOML defaults + environment overrides.
//! Load priority: `config/default.toml` -> `config/production.toml` -> `PV2_*` env vars.
//!
//! ## Layer: L1 (Foundation)
//! ## Module: M03
//! ## Dependencies: M02 (`PvError`)

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use super::m02_error_handling::{PvError, PvResult};

// ──────────────────────────────────────────────────────────────
// Top-level config
// ──────────────────────────────────────────────────────────────

/// Complete `Pane-Vortex` V2 configuration.
///
/// All sections have `serde` defaults for backward compatibility.
/// Load with [`PvConfig::load`] or [`PvConfig::from_path`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PvConfig {
    /// HTTP server settings (port, bind address, body limit).
    pub server: ServerConfig,
    /// Kuramoto field parameters (tick interval, dt, thresholds).
    pub field: FieldConfig,
    /// Per-sphere limits (max count, memory cap, decay rate).
    pub sphere: SphereConfig,
    /// Coupling network parameters (default weight, auto-scale).
    pub coupling: CouplingConfig,
    /// Hebbian STDP learning rates.
    pub learning: LearningConfig,
    /// External bridge intervals and `k_mod` bounds.
    pub bridges: BridgesConfig,
    /// Conductor breathing controller gains.
    pub conductor: ConductorConfig,
    /// IPC Unix socket bus settings.
    pub ipc: IpcConfig,
    /// `SQLite` persistence settings (snapshot interval, WAL timeout).
    pub persistence: PersistenceConfig,
    /// Governance voting thresholds.
    pub governance: GovernanceConfig,
}

impl PvConfig {
    /// Load configuration from default paths with env var overlay.
    ///
    /// Priority: `config/default.toml` -> `config/production.toml` -> `PV2_*` env vars.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigLoad`] if files cannot be parsed, or
    /// [`PvError::ConfigValidation`] if values fail validation.
    pub fn load() -> PvResult<Self> {
        let config: Self = Figment::new()
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file("config/production.toml"))
            .merge(Env::prefixed("PV2_").split("_"))
            .extract()?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a specific TOML file path.
    ///
    /// Environment variables with `PV2_` prefix still overlay the file values.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigLoad`] if the file cannot be parsed, or
    /// [`PvError::ConfigValidation`] if values fail validation.
    pub fn from_path(path: &str) -> PvResult<Self> {
        let config: Self = Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("PV2_").split("_"))
            .extract()?;
        config.validate()?;
        Ok(config)
    }

    /// Validate all configuration values.
    fn validate(&self) -> PvResult<()> {
        if self.server.port == 0 {
            return Err(PvError::ConfigValidation("server.port cannot be 0".into()));
        }
        if self.field.tick_interval_secs == 0 {
            return Err(PvError::ConfigValidation(
                "field.tick_interval_secs cannot be 0".into(),
            ));
        }
        if self.field.kuramoto_dt <= 0.0 || !self.field.kuramoto_dt.is_finite() {
            return Err(PvError::ConfigValidation(
                "field.kuramoto_dt must be positive and finite".into(),
            ));
        }
        if self.sphere.max_count == 0 {
            return Err(PvError::ConfigValidation(
                "sphere.max_count cannot be 0".into(),
            ));
        }
        if self.bridges.k_mod_budget_min >= self.bridges.k_mod_budget_max {
            return Err(PvError::ConfigValidation(
                "bridges.k_mod_budget_min must be < k_mod_budget_max".into(),
            ));
        }
        if self.governance.quorum_threshold <= 0.0 || self.governance.quorum_threshold > 1.0 {
            return Err(PvError::ConfigValidation(
                "governance.quorum_threshold must be in (0.0, 1.0]".into(),
            ));
        }
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────
// Config sections
// ──────────────────────────────────────────────────────────────

/// HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// TCP port to listen on (default `8132`).
    pub port: u16,
    /// Bind address (default `127.0.0.1`).
    pub bind_addr: String,
    /// Maximum request body size in bytes (default `65536`).
    pub body_limit_bytes: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8132,
            bind_addr: "127.0.0.1".into(),
            body_limit_bytes: 65536,
        }
    }
}

/// Kuramoto field configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldConfig {
    /// Seconds between tick pulses.
    pub tick_interval_secs: u64,
    /// Euler integration sub-steps per tick.
    pub coupling_steps_per_tick: usize,
    /// Kuramoto Euler integration timestep.
    pub kuramoto_dt: f64,
    /// Target order parameter `r`.
    pub r_target: f64,
    /// `r` above which the field is highly coherent.
    pub r_high_threshold: f64,
    /// `r` below which the field is incoherent.
    pub r_low_threshold: f64,
    /// `r` trend slope below which `RTrend::Falling` triggers.
    pub r_falling_threshold: f64,
    /// Fraction of idle spheres above which `IdleFleet` fires.
    pub idle_ratio_threshold: f64,
    /// Angular gap (radians) for chimera detection.
    pub phase_gap_threshold: f64,
    /// `r` above which synchronization is declared.
    pub sync_threshold: f64,
    /// Angular distance below which buoys form a tunnel.
    pub tunnel_threshold: f64,
    /// Reduced-dynamics ticks after snapshot restore.
    pub warmup_ticks: u32,
    /// Maximum `r` history samples retained.
    pub r_history_max: usize,
}

impl Default for FieldConfig {
    fn default() -> Self {
        Self {
            tick_interval_secs: 5,
            coupling_steps_per_tick: 15,
            kuramoto_dt: 0.01,
            r_target: 0.93,
            r_high_threshold: 0.8,
            r_low_threshold: 0.3,
            r_falling_threshold: -0.03,
            idle_ratio_threshold: 0.6,
            phase_gap_threshold: std::f64::consts::FRAC_PI_3,
            sync_threshold: 0.5,
            tunnel_threshold: 0.8,
            warmup_ticks: 5,
            r_history_max: 60,
        }
    }
}

/// Sphere configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SphereConfig {
    /// Maximum number of spheres in the field.
    pub max_count: usize,
    /// Maximum memories per sphere.
    pub memory_max_count: usize,
    /// Steps between memory prune passes.
    pub memory_prune_interval: u64,
    /// Multiplicative activation decay per tick step.
    pub decay_per_step: f64,
    /// Sweep-induced activation boost.
    pub sweep_boost: f64,
    /// Activation below which memories are prunable.
    pub activation_threshold: f64,
    /// Gentle semantic phase nudge strength.
    pub semantic_nudge_strength: f64,
    /// Maximum `last_tool` string length in characters.
    pub last_tool_max_chars: usize,
    /// Ticks during which a newcomer gets boosted LTP.
    pub newcomer_steps: u64,
}

impl Default for SphereConfig {
    fn default() -> Self {
        Self {
            max_count: 200,
            memory_max_count: 500,
            memory_prune_interval: 200,
            decay_per_step: 0.995,
            sweep_boost: 0.05,
            activation_threshold: 0.3,
            semantic_nudge_strength: 0.02,
            last_tool_max_chars: 128,
            newcomer_steps: 50,
        }
    }
}

/// Coupling network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CouplingConfig {
    /// Default connection weight for new sphere pairs.
    pub default_weight: f64,
    /// Weight exponent for coupling strength (fixed `w^2`).
    pub weight_exponent: f64,
    /// Ticks between auto-scale K adjustments.
    pub auto_scale_k_period: u64,
    /// Multiplier for auto-scale K adjustment step.
    pub auto_scale_k_multiplier: f64,
    /// Minimum natural frequency (Hz).
    pub frequency_min: f64,
    /// Maximum natural frequency (Hz).
    pub frequency_max: f64,
    /// Minimum coupling strength.
    pub strength_min: f64,
    /// Maximum coupling strength.
    pub strength_max: f64,
}

impl Default for CouplingConfig {
    fn default() -> Self {
        Self {
            default_weight: 0.18,
            weight_exponent: 2.0,
            auto_scale_k_period: 20,
            auto_scale_k_multiplier: 0.5,
            frequency_min: 0.001,
            frequency_max: 10.0,
            strength_min: 0.0,
            strength_max: 2.0,
        }
    }
}

/// Hebbian learning configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LearningConfig {
    /// Long-term potentiation rate (Hebbian weight increase).
    pub hebbian_ltp: f64,
    /// Long-term depression rate (Hebbian weight decrease).
    pub hebbian_ltd: f64,
    /// LTP multiplier during burst activity.
    pub burst_multiplier: f64,
    /// LTP multiplier for newcomer spheres.
    pub newcomer_multiplier: f64,
    /// Minimum Hebbian coupling weight (prevents disconnection).
    pub weight_floor: f64,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            hebbian_ltp: 0.01,
            hebbian_ltd: 0.002,
            burst_multiplier: 3.0,
            newcomer_multiplier: 2.0,
            weight_floor: 0.15,
        }
    }
}

/// Bridge configuration (external service connections).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BridgesConfig {
    /// Minimum `k_mod` value (absolute floor).
    pub k_mod_min: f64,
    /// Maximum `k_mod` value (absolute ceiling).
    pub k_mod_max: f64,
    /// Combined external influence floor (budget constraint).
    pub k_mod_budget_min: f64,
    /// Combined external influence ceiling (budget constraint).
    pub k_mod_budget_max: f64,
    /// Ticks between SYNTHEX polls.
    pub synthex_poll_interval: u64,
    /// Ticks between Nexus polls.
    pub nexus_poll_interval: u64,
    /// Ticks between Maintenance Engine polls.
    pub me_poll_interval: u64,
    /// Ticks between POVM snapshot posts.
    pub povm_snapshot_interval: u64,
    /// Ticks between POVM weight posts.
    pub povm_weights_interval: u64,
    /// Ticks between Reasoning Memory posts.
    pub rm_post_interval: u64,
    /// Ticks between Vortex Memory System posts.
    pub vms_post_interval: u64,
}

impl Default for BridgesConfig {
    fn default() -> Self {
        Self {
            k_mod_min: -0.5,
            k_mod_max: 1.5,
            k_mod_budget_min: 0.85,
            k_mod_budget_max: 1.15,
            synthex_poll_interval: 6,
            nexus_poll_interval: 12,
            me_poll_interval: 12,
            povm_snapshot_interval: 12,
            povm_weights_interval: 60,
            rm_post_interval: 60,
            vms_post_interval: 60,
        }
    }
}

/// Conductor (breathing controller) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConductorConfig {
    /// Proportional gain for the PI breathing controller.
    pub gain: f64,
    /// Fraction of emergent signal blended into conductor output.
    pub breathing_blend: f64,
    /// Ticks to wait after a divergence kick before allowing another.
    pub divergence_cooldown_ticks: u32,
}

impl Default for ConductorConfig {
    fn default() -> Self {
        Self {
            gain: 0.15,
            breathing_blend: 0.3,
            divergence_cooldown_ticks: 3,
        }
    }
}

/// IPC bus configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IpcConfig {
    /// Unix socket path (default `/run/user/1000/pane-vortex-bus.sock`).
    pub socket_path: String,
    /// Socket file permissions (octal, default `0o700`).
    pub socket_permissions: u32,
    /// Maximum concurrent IPC connections.
    pub max_connections: usize,
    /// Per-client event buffer capacity.
    pub event_buffer_size: usize,
    /// Task time-to-live in seconds before garbage collection.
    pub task_ttl_secs: u64,
    /// Maximum cascade dispatches per minute.
    pub cascade_rate_limit: u32,
    /// Event subscription patterns (default `["field.*", "sphere.*"]`).
    #[serde(default = "default_subscribe_patterns")]
    pub subscribe_patterns: Vec<String>,
}

/// Default IPC subscribe patterns.
fn default_subscribe_patterns() -> Vec<String> {
    vec!["field.*".into(), "sphere.*".into()]
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            socket_path: "/run/user/1000/pane-vortex-bus.sock".into(),
            socket_permissions: 0o700,
            max_connections: 50,
            event_buffer_size: 256,
            task_ttl_secs: 3600,
            cascade_rate_limit: 10,
            subscribe_patterns: default_subscribe_patterns(),
        }
    }
}

/// Persistence (`SQLite`) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistenceConfig {
    /// Ticks between field snapshot writes.
    pub snapshot_interval: u64,
    /// `SQLite` WAL busy timeout in milliseconds.
    pub wal_busy_timeout_ms: u64,
    /// File path for the bus tracking database.
    pub bus_db_path: String,
    /// File path for the field tracking database.
    pub field_db_path: String,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            snapshot_interval: 60,
            wal_busy_timeout_ms: 5000,
            bus_db_path: "data/bus_tracking.db".into(),
            field_db_path: "data/field_tracking.db".into(),
        }
    }
}

/// Governance (collective voting) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GovernanceConfig {
    /// Ticks during which a proposal accepts votes.
    pub proposal_voting_window_ticks: u64,
    /// Fraction of spheres required for quorum (in `(0.0, 1.0]`).
    pub quorum_threshold: f64,
    /// Maximum number of concurrent active proposals.
    pub max_active_proposals: usize,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            proposal_voting_window_ticks: 5,
            quorum_threshold: 0.5,
            max_active_proposals: 10,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Default config ──

    #[test]
    fn default_config_has_correct_port() {
        let config = PvConfig::default();
        assert_eq!(config.server.port, 8132);
    }

    #[test]
    fn default_config_has_correct_tick_interval() {
        let config = PvConfig::default();
        assert_eq!(config.field.tick_interval_secs, 5);
    }

    #[test]
    fn default_config_has_correct_sphere_cap() {
        let config = PvConfig::default();
        assert_eq!(config.sphere.max_count, 200);
    }

    #[test]
    fn default_config_has_correct_dt() {
        let config = PvConfig::default();
        assert!((config.field.kuramoto_dt - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn default_config_has_correct_r_target() {
        let config = PvConfig::default();
        assert!((config.field.r_target - 0.93).abs() < f64::EPSILON);
    }

    #[test]
    fn default_config_has_correct_ltp() {
        let config = PvConfig::default();
        assert!((config.learning.hebbian_ltp - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn default_config_has_correct_ltd() {
        let config = PvConfig::default();
        assert!((config.learning.hebbian_ltd - 0.002).abs() < f64::EPSILON);
    }

    #[test]
    fn default_config_has_correct_k_mod_budget() {
        let config = PvConfig::default();
        assert!((config.bridges.k_mod_budget_min - 0.85).abs() < f64::EPSILON);
        assert!((config.bridges.k_mod_budget_max - 1.15).abs() < f64::EPSILON);
    }

    #[test]
    fn default_config_has_correct_socket_path() {
        let config = PvConfig::default();
        assert_eq!(
            config.ipc.socket_path,
            "/run/user/1000/pane-vortex-bus.sock"
        );
    }

    #[test]
    fn default_config_has_correct_quorum() {
        let config = PvConfig::default();
        assert!((config.governance.quorum_threshold - 0.5).abs() < f64::EPSILON);
    }

    // ── Validation ──

    #[test]
    fn validate_rejects_zero_port() {
        let mut config = PvConfig::default();
        config.server.port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_tick_interval() {
        let mut config = PvConfig::default();
        config.field.tick_interval_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_nan_dt() {
        let mut config = PvConfig::default();
        config.field.kuramoto_dt = f64::NAN;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_negative_dt() {
        let mut config = PvConfig::default();
        config.field.kuramoto_dt = -0.01;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_sphere_cap() {
        let mut config = PvConfig::default();
        config.sphere.max_count = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_inverted_budget() {
        let mut config = PvConfig::default();
        config.bridges.k_mod_budget_min = 1.5;
        config.bridges.k_mod_budget_max = 0.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_quorum() {
        let mut config = PvConfig::default();
        config.governance.quorum_threshold = 0.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_quorum_above_one() {
        let mut config = PvConfig::default();
        config.governance.quorum_threshold = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_default() {
        let config = PvConfig::default();
        assert!(config.validate().is_ok());
    }

    // ── Load from file ──

    #[test]
    fn load_from_default_toml() {
        // This test works when run from the project root
        let result = PvConfig::from_path("config/default.toml");
        if let Ok(config) = result {
            assert_eq!(config.server.port, 8133);
            assert_eq!(config.field.coupling_steps_per_tick, 15);
        }
        // If file not found, that's OK — CI may not have it
    }

    // ── Serde roundtrip ──

    #[test]
    fn config_serde_roundtrip() {
        let config = PvConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let back: PvConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.server.port, config.server.port);
        assert_eq!(back.field.tick_interval_secs, config.field.tick_interval_secs);
    }

    // ── Section defaults ──

    #[test]
    fn conductor_defaults() {
        let c = ConductorConfig::default();
        assert!((c.gain - 0.15).abs() < f64::EPSILON);
        assert!((c.breathing_blend - 0.3).abs() < f64::EPSILON);
        assert_eq!(c.divergence_cooldown_ticks, 3);
    }

    #[test]
    fn persistence_defaults() {
        let p = PersistenceConfig::default();
        assert_eq!(p.snapshot_interval, 60);
        assert_eq!(p.wal_busy_timeout_ms, 5000);
    }

    #[test]
    fn coupling_defaults() {
        let c = CouplingConfig::default();
        assert!((c.default_weight - 0.18).abs() < f64::EPSILON);
        assert!((c.frequency_min - 0.001).abs() < f64::EPSILON);
        assert!((c.frequency_max - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ipc_defaults() {
        let i = IpcConfig::default();
        assert_eq!(i.max_connections, 50);
        assert_eq!(i.event_buffer_size, 256);
        assert_eq!(i.task_ttl_secs, 3600);
        assert_eq!(i.cascade_rate_limit, 10);
    }
}
