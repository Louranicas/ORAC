//! # M02: Error Handling
//!
//! Unified error enum `PvError` with classification by retryability,
//! severity, and category. Uses `thiserror` for structured errors.
//!
//! ## Layer: L1 (Foundation)
//! ## Module: M02
//! ## Dependencies: None
//!
//! ## Error Categories
//! - Config (1000-1099): Configuration validation failures
//! - Validation (1100-1199): Input validation failures
//! - Field (1200-1299): Kuramoto field computation errors
//! - Bridge (1300-1399): External service communication failures
//! - Bus (1400-1499): IPC bus protocol errors
//! - Persistence (1500-1599): `SQLite`/WAL errors
//! - Governance (1600-1699): Proposal/voting errors

use std::fmt;

// ──────────────────────────────────────────────────────────────
// Result alias
// ──────────────────────────────────────────────────────────────

/// Convenience alias for `Result<T, PvError>` used throughout the crate.
pub type PvResult<T> = Result<T, PvError>;

// ──────────────────────────────────────────────────────────────
// Error enum
// ──────────────────────────────────────────────────────────────

/// Unified error type for `Pane-Vortex` V2.
///
/// Each variant carries a structured error code (see module docs) and enough
/// context for the caller to decide whether to retry, log, or propagate.
#[derive(Debug, thiserror::Error)]
pub enum PvError {
    // ── Config (1000-1099) ──
    /// Configuration file could not be loaded or parsed.
    #[error("[PV-1000] config load failed: {0}")]
    ConfigLoad(String),

    /// A configuration value failed validation.
    #[error("[PV-1001] config validation: {0}")]
    ConfigValidation(String),

    // ── Validation (1100-1199) ──
    /// Input value is NaN or infinite.
    #[error("[PV-1100] non-finite value: {field} = {value}")]
    NonFinite { field: &'static str, value: f64 },

    /// Input value is out of the acceptable range.
    #[error("[PV-1101] out of range: {field} = {value} (expected {min}..{max})")]
    OutOfRange {
        field: &'static str,
        value: f64,
        min: f64,
        max: f64,
    },

    /// String input is empty when a non-empty value is required.
    #[error("[PV-1102] empty string: {field}")]
    EmptyString { field: &'static str },

    /// String input exceeds maximum length.
    #[error("[PV-1103] string too long: {field} ({len} > {max})")]
    StringTooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },

    /// String contains invalid characters.
    #[error("[PV-1104] invalid characters in {field}: {reason}")]
    InvalidChars {
        field: &'static str,
        reason: String,
    },

    // ── Field (1200-1299) ──
    /// Sphere not found in the field.
    #[error("[PV-1200] sphere not found: {0}")]
    SphereNotFound(String),

    /// Sphere already exists (duplicate registration).
    #[error("[PV-1201] sphere already registered: {0}")]
    SphereAlreadyRegistered(String),

    /// Maximum sphere count reached.
    #[error("[PV-1202] sphere cap reached ({0})")]
    SphereCapReached(usize),

    /// Field computation produced invalid state.
    #[error("[PV-1203] field computation error: {0}")]
    FieldComputation(String),

    // ── Bridge (1300-1399) ──
    /// External service is unreachable.
    #[error("[PV-1300] bridge unreachable: {service} at {url}")]
    BridgeUnreachable { service: String, url: String },

    /// External service returned an error response.
    #[error("[PV-1301] bridge error: {service} returned {status}")]
    BridgeError { service: String, status: u16 },

    /// External service response could not be parsed.
    #[error("[PV-1302] bridge parse error: {service}: {reason}")]
    BridgeParse { service: String, reason: String },

    /// Bridge consent denied — sphere opted out of external modulation.
    #[error("[PV-1303] bridge consent denied: {service} for sphere {sphere}")]
    BridgeConsentDenied { service: String, sphere: String },

    // ── Bus (1400-1499) ──
    /// IPC bus socket error.
    #[error("[PV-1400] bus socket error: {0}")]
    BusSocket(String),

    /// IPC bus protocol violation (invalid NDJSON, unknown message type).
    #[error("[PV-1401] bus protocol error: {0}")]
    BusProtocol(String),

    /// IPC bus task not found.
    #[error("[PV-1402] bus task not found: {0}")]
    BusTaskNotFound(String),

    /// Cascade rate limit exceeded.
    #[error("[PV-1403] cascade rate limit exceeded: {per_minute} per minute")]
    CascadeRateLimit { per_minute: u32 },

    // ── Persistence (1500-1599) ──
    /// `SQLite` operation failed.
    #[error("[PV-1500] database error: {0}")]
    Database(String),

    /// Snapshot save/restore failed.
    #[error("[PV-1501] snapshot error: {0}")]
    Snapshot(String),

    // ── Governance (1600-1699) ──
    /// Proposal not found.
    #[error("[PV-1600] proposal not found: {0}")]
    ProposalNotFound(String),

    /// Voting is closed for this proposal.
    #[error("[PV-1601] voting closed: {0}")]
    VotingClosed(String),

    /// Quorum not reached.
    #[error("[PV-1602] quorum not reached: {votes}/{needed}")]
    QuorumNotReached { votes: usize, needed: usize },

    // ── Generic ──
    /// IO error wrapper.
    #[error("[PV-1900] io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("[PV-1901] json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Internal error that should never happen.
    #[error("[PV-1999] internal error: {0}")]
    Internal(String),
}

// ──────────────────────────────────────────────────────────────
// Error classification
// ──────────────────────────────────────────────────────────────

/// Error severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Informational — no action needed.
    Info,
    /// Warning — potential issue, system continues.
    Warning,
    /// Error — operation failed, retry may help.
    Error,
    /// Critical — system integrity at risk.
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Error classification for retry and alerting decisions.
///
/// Implementors categorise errors by retryability, severity, and numeric code.
/// The conductor and bridge layers use this to decide whether to retry, log,
/// escalate, or drop a failed operation.
pub trait ErrorClassifier: Send + Sync + std::fmt::Debug {
    /// Whether the error is transient and likely to succeed on retry.
    ///
    /// Returns `true` for network/IO/database errors that may be caused by
    /// temporary conditions (connection refused, busy database). Returns
    /// `false` for validation and logic errors that will never self-resolve.
    fn is_retryable(&self) -> bool;

    /// Severity level for logging and alerting thresholds.
    ///
    /// - `Info`: validation failures, not-found lookups
    /// - `Warning`: capacity limits, consent denials
    /// - `Error`: failed I/O, unreachable services
    /// - `Critical`: data integrity violations, internal errors
    fn severity(&self) -> ErrorSeverity;

    /// Numeric error code in the `PV-NNNN` range.
    ///
    /// Each error variant has a unique code. See the module-level docs for
    /// the code-range-to-category mapping.
    fn code(&self) -> u16;
}

impl ErrorClassifier for PvError {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::BridgeUnreachable { .. }
                | Self::BridgeError { .. }
                | Self::BusSocket(_)
                | Self::Database(_)
                | Self::Io(_)
        )
    }

    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: data integrity or system state
            Self::FieldComputation(_) | Self::Internal(_) => ErrorSeverity::Critical,
            // Error: operation failed
            Self::BridgeUnreachable { .. }
            | Self::BridgeError { .. }
            | Self::BusSocket(_)
            | Self::Database(_)
            | Self::Snapshot(_) => ErrorSeverity::Error,
            // Warning: capacity or consent
            Self::SphereCapReached(_)
            | Self::BridgeConsentDenied { .. }
            | Self::CascadeRateLimit { .. } => ErrorSeverity::Warning,
            // Info: validation, not found, etc.
            _ => ErrorSeverity::Info,
        }
    }

    fn code(&self) -> u16 {
        match self {
            Self::ConfigLoad(_) => 1000,
            Self::ConfigValidation(_) => 1001,
            Self::NonFinite { .. } => 1100,
            Self::OutOfRange { .. } => 1101,
            Self::EmptyString { .. } => 1102,
            Self::StringTooLong { .. } => 1103,
            Self::InvalidChars { .. } => 1104,
            Self::SphereNotFound(_) => 1200,
            Self::SphereAlreadyRegistered(_) => 1201,
            Self::SphereCapReached(_) => 1202,
            Self::FieldComputation(_) => 1203,
            Self::BridgeUnreachable { .. } => 1300,
            Self::BridgeError { .. } => 1301,
            Self::BridgeParse { .. } => 1302,
            Self::BridgeConsentDenied { .. } => 1303,
            Self::BusSocket(_) => 1400,
            Self::BusProtocol(_) => 1401,
            Self::BusTaskNotFound(_) => 1402,
            Self::CascadeRateLimit { .. } => 1403,
            Self::Database(_) => 1500,
            Self::Snapshot(_) => 1501,
            Self::ProposalNotFound(_) => 1600,
            Self::VotingClosed(_) => 1601,
            Self::QuorumNotReached { .. } => 1602,
            Self::Io(_) => 1900,
            Self::Json(_) => 1901,
            Self::Internal(_) => 1999,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// From impls for external error types
// ──────────────────────────────────────────────────────────────

impl From<figment::Error> for PvError {
    fn from(e: figment::Error) -> Self {
        Self::ConfigLoad(e.to_string())
    }
}

impl From<toml::de::Error> for PvError {
    fn from(e: toml::de::Error) -> Self {
        Self::ConfigLoad(e.to_string())
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Display formatting ──

    #[test]
    fn error_display_config_load() {
        let e = PvError::ConfigLoad("file not found".into());
        assert!(e.to_string().contains("PV-1000"));
        assert!(e.to_string().contains("file not found"));
    }

    #[test]
    fn error_display_non_finite() {
        let e = PvError::NonFinite {
            field: "phase",
            value: f64::NAN,
        };
        assert!(e.to_string().contains("PV-1100"));
        assert!(e.to_string().contains("phase"));
    }

    #[test]
    fn error_display_out_of_range() {
        let e = PvError::OutOfRange {
            field: "frequency",
            value: -1.0,
            min: 0.001,
            max: 10.0,
        };
        assert!(e.to_string().contains("PV-1101"));
    }

    #[test]
    fn error_display_empty_string() {
        let e = PvError::EmptyString { field: "pane_id" };
        assert!(e.to_string().contains("PV-1102"));
    }

    #[test]
    fn error_display_string_too_long() {
        let e = PvError::StringTooLong {
            field: "persona",
            len: 500,
            max: 256,
        };
        assert!(e.to_string().contains("PV-1103"));
    }

    #[test]
    fn error_display_sphere_not_found() {
        let e = PvError::SphereNotFound("missing-sphere".into());
        assert!(e.to_string().contains("PV-1200"));
    }

    #[test]
    fn error_display_bridge_unreachable() {
        let e = PvError::BridgeUnreachable {
            service: "synthex".into(),
            url: "localhost:8090".into(),
        };
        assert!(e.to_string().contains("PV-1300"));
        assert!(e.to_string().contains("synthex"));
    }

    #[test]
    fn error_display_cascade_rate_limit() {
        let e = PvError::CascadeRateLimit { per_minute: 10 };
        assert!(e.to_string().contains("PV-1403"));
    }

    #[test]
    fn error_display_quorum_not_reached() {
        let e = PvError::QuorumNotReached {
            votes: 2,
            needed: 5,
        };
        assert!(e.to_string().contains("2/5"));
    }

    // ── Error classification ──

    #[test]
    fn retryable_bridge_unreachable() {
        let e = PvError::BridgeUnreachable {
            service: "synthex".into(),
            url: "localhost:8090".into(),
        };
        assert!(e.is_retryable());
    }

    #[test]
    fn retryable_bridge_error() {
        let e = PvError::BridgeError {
            service: "me".into(),
            status: 503,
        };
        assert!(e.is_retryable());
    }

    #[test]
    fn retryable_bus_socket() {
        let e = PvError::BusSocket("connection refused".into());
        assert!(e.is_retryable());
    }

    #[test]
    fn retryable_database() {
        let e = PvError::Database("busy".into());
        assert!(e.is_retryable());
    }

    #[test]
    fn not_retryable_validation() {
        let e = PvError::NonFinite {
            field: "phase",
            value: f64::NAN,
        };
        assert!(!e.is_retryable());
    }

    #[test]
    fn not_retryable_sphere_not_found() {
        let e = PvError::SphereNotFound("gone".into());
        assert!(!e.is_retryable());
    }

    #[test]
    fn not_retryable_config() {
        let e = PvError::ConfigValidation("bad value".into());
        assert!(!e.is_retryable());
    }

    // ── Severity classification ──

    #[test]
    fn severity_critical_field_computation() {
        let e = PvError::FieldComputation("NaN in order parameter".into());
        assert_eq!(e.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn severity_critical_internal() {
        let e = PvError::Internal("impossible state".into());
        assert_eq!(e.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn severity_error_bridge() {
        let e = PvError::BridgeUnreachable {
            service: "x".into(),
            url: "y".into(),
        };
        assert_eq!(e.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn severity_warning_cap_reached() {
        let e = PvError::SphereCapReached(200);
        assert_eq!(e.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn severity_warning_consent_denied() {
        let e = PvError::BridgeConsentDenied {
            service: "synthex".into(),
            sphere: "test".into(),
        };
        assert_eq!(e.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn severity_info_validation() {
        let e = PvError::EmptyString { field: "x" };
        assert_eq!(e.severity(), ErrorSeverity::Info);
    }

    // ── Error codes ──

    #[test]
    fn error_codes_unique_per_variant() {
        let codes = vec![
            PvError::ConfigLoad(String::new()).code(),
            PvError::ConfigValidation(String::new()).code(),
            PvError::NonFinite {
                field: "",
                value: 0.0,
            }
            .code(),
            PvError::OutOfRange {
                field: "",
                value: 0.0,
                min: 0.0,
                max: 0.0,
            }
            .code(),
            PvError::EmptyString { field: "" }.code(),
            PvError::StringTooLong {
                field: "",
                len: 0,
                max: 0,
            }
            .code(),
            PvError::SphereNotFound(String::new()).code(),
            PvError::SphereAlreadyRegistered(String::new()).code(),
            PvError::SphereCapReached(0).code(),
            PvError::FieldComputation(String::new()).code(),
            PvError::BusSocket(String::new()).code(),
            PvError::BusProtocol(String::new()).code(),
            PvError::BusTaskNotFound(String::new()).code(),
            PvError::Database(String::new()).code(),
            PvError::Snapshot(String::new()).code(),
            PvError::ProposalNotFound(String::new()).code(),
            PvError::VotingClosed(String::new()).code(),
            PvError::Internal(String::new()).code(),
        ];
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len(), "duplicate error codes found");
    }

    // ── Error codes in range ──

    #[test]
    fn error_codes_in_expected_ranges() {
        assert_eq!(PvError::ConfigLoad(String::new()).code(), 1000);
        assert_eq!(
            PvError::NonFinite {
                field: "",
                value: 0.0
            }
            .code(),
            1100
        );
        assert_eq!(PvError::SphereNotFound(String::new()).code(), 1200);
        assert_eq!(
            PvError::BridgeUnreachable {
                service: String::new(),
                url: String::new()
            }
            .code(),
            1300
        );
        assert_eq!(PvError::BusSocket(String::new()).code(), 1400);
        assert_eq!(PvError::Database(String::new()).code(), 1500);
        assert_eq!(PvError::ProposalNotFound(String::new()).code(), 1600);
    }

    // ── From impls ──

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let pv_err: PvError = io_err.into();
        assert_eq!(pv_err.code(), 1900);
        assert!(pv_err.is_retryable()); // IO errors may be transient
    }

    #[test]
    fn from_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let pv_err: PvError = json_err.into();
        assert_eq!(pv_err.code(), 1901);
    }

    // ── ErrorSeverity Display ──

    #[test]
    fn severity_display() {
        assert_eq!(format!("{}", ErrorSeverity::Info), "INFO");
        assert_eq!(format!("{}", ErrorSeverity::Warning), "WARN");
        assert_eq!(format!("{}", ErrorSeverity::Error), "ERROR");
        assert_eq!(format!("{}", ErrorSeverity::Critical), "CRITICAL");
    }

    // ── PvResult alias ──

    #[test]
    fn pv_result_ok() {
        let r: PvResult<i32> = Ok(42);
        assert_eq!(r.unwrap(), 42);
    }

    #[test]
    fn pv_result_err() {
        let r: PvResult<i32> = Err(PvError::Internal("test".into()));
        assert!(r.is_err());
    }
}
