//! # M06: Input Validation
//!
//! Validates all external inputs at system boundary.
//! Every API handler calls validation before processing.
//!
//! ## Layer: L1 (Foundation)
//! ## Module: M06
//! ## Dependencies: M02 (errors), M04 (constants)
//!
//! ## Design Constraints
//! - C3: Phase wrapping via `rem_euclid(TAU)`
//! - C11: `is_finite()` check on ALL f64 inputs
//! - C12: String truncation via `chars().take()` not byte slicing (R15)

use std::f64::consts::TAU;

use super::m02_error_handling::{PvError, PvResult};
use super::m04_constants;

// ──────────────────────────────────────────────────────────────
// Numeric validators
// ──────────────────────────────────────────────────────────────

/// Validate and wrap a phase value into [0, 2π).
///
/// NaN and infinity are rejected. Negative or large values are wrapped.
///
/// # Errors
/// Returns [`PvError::NonFinite`] if the input is NaN or infinite.
pub fn validate_phase(phase: f64) -> PvResult<f64> {
    if !phase.is_finite() {
        return Err(PvError::NonFinite {
            field: "phase",
            value: phase,
        });
    }
    Ok(phase.rem_euclid(TAU))
}

/// Validate and clamp a frequency value to `[0.001, 10.0]`.
///
/// # Errors
/// Returns [`PvError::NonFinite`] if the input is NaN or infinite.
pub fn validate_frequency(freq: f64) -> PvResult<f64> {
    if !freq.is_finite() {
        return Err(PvError::NonFinite {
            field: "frequency",
            value: freq,
        });
    }
    Ok(freq.clamp(0.001, 10.0))
}

/// Validate and clamp a coupling strength to [0.0, 2.0].
///
/// # Errors
/// Returns [`PvError::NonFinite`] if the input is NaN or infinite.
pub fn validate_strength(strength: f64) -> PvResult<f64> {
    if !strength.is_finite() {
        return Err(PvError::NonFinite {
            field: "strength",
            value: strength,
        });
    }
    Ok(strength.clamp(0.0, 2.0))
}

/// Validate and clamp a weight to `[HEBBIAN_WEIGHT_FLOOR, 1.0]`.
///
/// # Errors
/// Returns [`PvError::NonFinite`] if the input is NaN or infinite.
pub fn validate_weight(weight: f64) -> PvResult<f64> {
    if !weight.is_finite() {
        return Err(PvError::NonFinite {
            field: "weight",
            value: weight,
        });
    }
    Ok(weight.clamp(m04_constants::HEBBIAN_WEIGHT_FLOOR, 1.0))
}

/// Validate and clamp a receptivity value to [0.0, 1.0].
///
/// # Errors
/// Returns [`PvError::NonFinite`] if the input is NaN or infinite.
pub fn validate_receptivity(receptivity: f64) -> PvResult<f64> {
    if !receptivity.is_finite() {
        return Err(PvError::NonFinite {
            field: "receptivity",
            value: receptivity,
        });
    }
    Ok(receptivity.clamp(0.0, 1.0))
}

/// Validate and clamp a `k_mod` value to `[K_MOD_MIN, K_MOD_MAX]`.
///
/// # Errors
/// Returns [`PvError::NonFinite`] if the input is NaN or infinite.
pub fn validate_k_mod(k_mod: f64) -> PvResult<f64> {
    if !k_mod.is_finite() {
        return Err(PvError::NonFinite {
            field: "k_mod",
            value: k_mod,
        });
    }
    Ok(k_mod.clamp(m04_constants::K_MOD_MIN, m04_constants::K_MOD_MAX))
}

// ──────────────────────────────────────────────────────────────
// String validators
// ──────────────────────────────────────────────────────────────

/// Maximum pane ID length.
const PANE_ID_MAX_LEN: usize = 128;

/// Valid characters for pane IDs: alphanumeric, dot, underscore, colon, hyphen.
const fn is_valid_pane_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-')
}

/// Validate a pane ID string.
///
/// Must be 1–128 characters, ASCII alphanumeric plus `._:-`.
///
/// # Errors
/// Returns [`PvError::EmptyString`], [`PvError::StringTooLong`], or [`PvError::InvalidChars`].
pub fn validate_pane_id(id: &str) -> PvResult<()> {
    if id.is_empty() {
        return Err(PvError::EmptyString { field: "pane_id" });
    }
    if id.len() > PANE_ID_MAX_LEN {
        return Err(PvError::StringTooLong {
            field: "pane_id",
            len: id.len(),
            max: PANE_ID_MAX_LEN,
        });
    }
    if let Some(c) = id.chars().find(|c| !is_valid_pane_id_char(*c)) {
        return Err(PvError::InvalidChars {
            field: "pane_id",
            reason: format!("invalid character: '{c}'"),
        });
    }
    Ok(())
}

/// Maximum persona string length.
const PERSONA_MAX_LEN: usize = 256;

/// Validate a persona string.
///
/// Must be non-empty, max 256 characters, valid UTF-8.
///
/// # Errors
/// Returns [`PvError::EmptyString`] or [`PvError::StringTooLong`].
pub fn validate_persona(persona: &str) -> PvResult<()> {
    if persona.is_empty() {
        return Err(PvError::EmptyString { field: "persona" });
    }
    let char_count = persona.chars().count();
    if char_count > PERSONA_MAX_LEN {
        return Err(PvError::StringTooLong {
            field: "persona",
            len: char_count,
            max: PERSONA_MAX_LEN,
        });
    }
    Ok(())
}

/// Maximum tool name length.
const TOOL_NAME_MAX_LEN: usize = 128;

/// Validate a tool name string.
///
/// Must be non-empty, max 128 characters.
///
/// # Errors
/// Returns [`PvError::EmptyString`] or [`PvError::StringTooLong`].
pub fn validate_tool_name(name: &str) -> PvResult<()> {
    if name.is_empty() {
        return Err(PvError::EmptyString { field: "tool_name" });
    }
    let char_count = name.chars().count();
    if char_count > TOOL_NAME_MAX_LEN {
        return Err(PvError::StringTooLong {
            field: "tool_name",
            len: char_count,
            max: TOOL_NAME_MAX_LEN,
        });
    }
    Ok(())
}

/// Maximum summary string length.
const SUMMARY_MAX_LEN: usize = 1024;

/// Validate a summary string.
///
/// May be empty. Max 1024 characters.
///
/// # Errors
/// Returns [`PvError::StringTooLong`] if the input exceeds `SUMMARY_MAX_LEN` characters.
pub fn validate_summary(summary: &str) -> PvResult<()> {
    let char_count = summary.chars().count();
    if char_count > SUMMARY_MAX_LEN {
        return Err(PvError::StringTooLong {
            field: "summary",
            len: char_count,
            max: SUMMARY_MAX_LEN,
        });
    }
    Ok(())
}

/// Truncate a string to `max_chars` characters (UTF-8 safe).
///
/// Uses `chars().take()` instead of byte slicing to avoid splitting multi-byte chars.
#[must_use]
pub fn truncate_string(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use std::f64::consts::PI;

    // ── Phase validation ──

    #[test]
    fn validate_phase_wraps_negative() {
        let p = validate_phase(-0.1).unwrap();
        assert!(p >= 0.0 && p < TAU);
        assert_relative_eq!(p, TAU - 0.1, epsilon = 1e-10);
    }

    #[test]
    fn validate_phase_wraps_large() {
        let p = validate_phase(TAU + 1.0).unwrap();
        assert_relative_eq!(p, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn validate_phase_zero() {
        let p = validate_phase(0.0).unwrap();
        assert_relative_eq!(p, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn validate_phase_pi() {
        let p = validate_phase(PI).unwrap();
        assert_relative_eq!(p, PI, epsilon = 1e-10);
    }

    #[test]
    fn validate_phase_rejects_nan() {
        assert!(validate_phase(f64::NAN).is_err());
    }

    #[test]
    fn validate_phase_rejects_infinity() {
        assert!(validate_phase(f64::INFINITY).is_err());
    }

    #[test]
    fn validate_phase_rejects_neg_infinity() {
        assert!(validate_phase(f64::NEG_INFINITY).is_err());
    }

    // ── Frequency validation ──

    #[test]
    fn validate_frequency_normal() {
        let f = validate_frequency(0.1).unwrap();
        assert_relative_eq!(f, 0.1);
    }

    #[test]
    fn validate_frequency_clamps_low() {
        let f = validate_frequency(-5.0).unwrap();
        assert_relative_eq!(f, 0.001, epsilon = 1e-10);
    }

    #[test]
    fn validate_frequency_clamps_high() {
        let f = validate_frequency(100.0).unwrap();
        assert_relative_eq!(f, 10.0);
    }

    #[test]
    fn validate_frequency_rejects_nan() {
        assert!(validate_frequency(f64::NAN).is_err());
    }

    // ── Strength validation ──

    #[test]
    fn validate_strength_normal() {
        let s = validate_strength(1.0).unwrap();
        assert_relative_eq!(s, 1.0);
    }

    #[test]
    fn validate_strength_clamps_negative() {
        let s = validate_strength(-1.0).unwrap();
        assert_relative_eq!(s, 0.0);
    }

    #[test]
    fn validate_strength_clamps_high() {
        let s = validate_strength(5.0).unwrap();
        assert_relative_eq!(s, 2.0);
    }

    #[test]
    fn validate_strength_rejects_nan() {
        assert!(validate_strength(f64::NAN).is_err());
    }

    // ── Weight validation ──

    #[test]
    fn validate_weight_normal() {
        let w = validate_weight(0.5).unwrap();
        assert_relative_eq!(w, 0.5);
    }

    #[test]
    fn validate_weight_clamps_to_floor() {
        let w = validate_weight(0.01).unwrap();
        assert_relative_eq!(w, m04_constants::HEBBIAN_WEIGHT_FLOOR);
    }

    #[test]
    fn validate_weight_clamps_high() {
        let w = validate_weight(5.0).unwrap();
        assert_relative_eq!(w, 1.0);
    }

    #[test]
    fn validate_weight_rejects_nan() {
        assert!(validate_weight(f64::NAN).is_err());
    }

    // ── Receptivity validation ──

    #[test]
    fn validate_receptivity_normal() {
        let r = validate_receptivity(0.8).unwrap();
        assert_relative_eq!(r, 0.8);
    }

    #[test]
    fn validate_receptivity_clamps_negative() {
        let r = validate_receptivity(-0.5).unwrap();
        assert_relative_eq!(r, 0.0);
    }

    #[test]
    fn validate_receptivity_clamps_above_one() {
        let r = validate_receptivity(1.5).unwrap();
        assert_relative_eq!(r, 1.0);
    }

    #[test]
    fn validate_receptivity_rejects_nan() {
        assert!(validate_receptivity(f64::NAN).is_err());
    }

    // ── K mod validation ──

    #[test]
    fn validate_k_mod_normal() {
        let k = validate_k_mod(1.0).unwrap();
        assert_relative_eq!(k, 1.0);
    }

    #[test]
    fn validate_k_mod_clamps_low() {
        let k = validate_k_mod(-10.0).unwrap();
        assert_relative_eq!(k, m04_constants::K_MOD_MIN);
    }

    #[test]
    fn validate_k_mod_clamps_high() {
        let k = validate_k_mod(10.0).unwrap();
        assert_relative_eq!(k, m04_constants::K_MOD_MAX);
    }

    #[test]
    fn validate_k_mod_rejects_nan() {
        assert!(validate_k_mod(f64::NAN).is_err());
    }

    // ── Pane ID validation ──

    #[test]
    fn validate_pane_id_normal() {
        assert!(validate_pane_id("fleet-alpha:left").is_ok());
    }

    #[test]
    fn validate_pane_id_with_dots() {
        assert!(validate_pane_id("claude.session-039").is_ok());
    }

    #[test]
    fn validate_pane_id_with_underscores() {
        assert!(validate_pane_id("my_sphere_1").is_ok());
    }

    #[test]
    fn validate_pane_id_rejects_empty() {
        assert!(validate_pane_id("").is_err());
    }

    #[test]
    fn validate_pane_id_rejects_too_long() {
        let long_id = "a".repeat(129);
        assert!(validate_pane_id(&long_id).is_err());
    }

    #[test]
    fn validate_pane_id_accepts_max_length() {
        let max_id = "a".repeat(128);
        assert!(validate_pane_id(&max_id).is_ok());
    }

    #[test]
    fn validate_pane_id_rejects_spaces() {
        assert!(validate_pane_id("has space").is_err());
    }

    #[test]
    fn validate_pane_id_rejects_special_chars() {
        assert!(validate_pane_id("has/slash").is_err());
        assert!(validate_pane_id("has@at").is_err());
        assert!(validate_pane_id("has#hash").is_err());
    }

    // ── Persona validation ──

    #[test]
    fn validate_persona_normal() {
        assert!(validate_persona("explorer").is_ok());
    }

    #[test]
    fn validate_persona_rejects_empty() {
        assert!(validate_persona("").is_err());
    }

    #[test]
    fn validate_persona_rejects_too_long() {
        let long = "a".repeat(257);
        assert!(validate_persona(&long).is_err());
    }

    #[test]
    fn validate_persona_accepts_unicode() {
        assert!(validate_persona("探検家").is_ok());
    }

    // ── Tool name validation ──

    #[test]
    fn validate_tool_name_normal() {
        assert!(validate_tool_name("Read").is_ok());
    }

    #[test]
    fn validate_tool_name_rejects_empty() {
        assert!(validate_tool_name("").is_err());
    }

    #[test]
    fn validate_tool_name_rejects_too_long() {
        let long = "a".repeat(129);
        assert!(validate_tool_name(&long).is_err());
    }

    // ── Summary validation ──

    #[test]
    fn validate_summary_normal() {
        assert!(validate_summary("read config file").is_ok());
    }

    #[test]
    fn validate_summary_empty_ok() {
        assert!(validate_summary("").is_ok());
    }

    #[test]
    fn validate_summary_rejects_too_long() {
        let long = "a".repeat(1025);
        assert!(validate_summary(&long).is_err());
    }

    // ── String truncation ──

    #[test]
    fn truncate_string_short() {
        assert_eq!(truncate_string("hello", 10), "hello");
    }

    #[test]
    fn truncate_string_exact() {
        assert_eq!(truncate_string("hello", 5), "hello");
    }

    #[test]
    fn truncate_string_long() {
        assert_eq!(truncate_string("hello world", 5), "hello");
    }

    #[test]
    fn truncate_string_empty() {
        assert_eq!(truncate_string("", 5), "");
    }

    #[test]
    fn truncate_string_unicode_safe() {
        let s = "日本語テスト";
        let t = truncate_string(s, 3);
        assert_eq!(t, "日本語");
    }

    #[test]
    fn truncate_string_zero_max() {
        assert_eq!(truncate_string("hello", 0), "");
    }
}
