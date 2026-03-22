# ORAC Sidecar — Gold Standard Patterns

> 10 mandatory patterns. Every module must conform. No exceptions.

## P1: Builder Pattern (All Constructors)

All structs with 3+ fields use builder pattern. No public `new()` with many args.

```rust
pub struct ProxyConfig {
    port: u16,
    max_connections: usize,
    timeout_ms: u64,
}

impl ProxyConfig {
    pub fn builder() -> ProxyConfigBuilder {
        ProxyConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ProxyConfigBuilder {
    port: Option<u16>,
    max_connections: Option<usize>,
    timeout_ms: Option<u64>,
}

impl ProxyConfigBuilder {
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn max_connections(mut self, n: usize) -> Self {
        self.max_connections = Some(n);
        self
    }

    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    pub fn build(self) -> Result<ProxyConfig, OracError> {
        Ok(ProxyConfig {
            port: self.port.ok_or(OracError::MissingField("port"))?,
            max_connections: self.max_connections.unwrap_or(1024),
            timeout_ms: self.timeout_ms.unwrap_or(5000),
        })
    }
}
```

## P2: Interior Mutability (`&self` + `parking_lot::RwLock`)

Shared state uses `&self` methods with `parking_lot::RwLock`. Never `std::sync::RwLock`.

```rust
use parking_lot::RwLock;
use std::sync::Arc;

pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<AgentId, AgentState>>>,
}

impl AgentRegistry {
    /// Register an agent. Thread-safe via interior mutability.
    pub fn register(&self, id: AgentId, state: AgentState) -> Result<(), OracError> {
        let mut guard = self.agents.write();
        guard.insert(id, state);
        Ok(())
    }

    /// Get agent state. Returns owned clone, never a reference.
    pub fn get(&self, id: &AgentId) -> Option<AgentState> {
        let guard = self.agents.read();
        guard.get(id).cloned()
    }
}
```

## P3: Result Everywhere (Zero Panic Paths)

Every fallible operation returns `Result<T, OracError>`. Zero `unwrap()`, zero `expect()`, zero `panic!()`.

```rust
/// Route a request to the appropriate backend.
///
/// # Errors
///
/// Returns `OracError::NoRoute` if no backend matches the request path.
/// Returns `OracError::BackendDown` if the selected backend is unreachable.
pub fn route_request(&self, req: &InboundRequest) -> Result<BackendId, OracError> {
    let guard = self.routes.read();
    let backend = guard
        .get(&req.path)
        .ok_or_else(|| OracError::NoRoute(req.path.clone()))?;

    if !backend.is_healthy() {
        return Err(OracError::BackendDown(backend.id.clone()));
    }

    Ok(backend.id.clone())
}
```

## P4: Scoped Lock Guards (Explicit Drop Before Next Lock)

Never hold two locks simultaneously. Drop first guard explicitly before acquiring second.

```rust
pub fn transfer_state(&self, from: &AgentId, to: &AgentId) -> Result<(), OracError> {
    // Acquire first lock, extract data, drop guard
    let state = {
        let guard = self.agents.read();
        guard.get(from).cloned()
            .ok_or_else(|| OracError::NotFound(from.clone()))?
    }; // guard dropped here

    // Safe to acquire second lock
    {
        let mut guard = self.pending.write();
        guard.insert(to.clone(), state);
    } // guard dropped here

    Ok(())
}
```

## P5: FMA for Float Precision

Use `f64::mul_add()` for fused multiply-add. Never `a * b + c`.

```rust
/// Compute coupling strength with decay.
///
/// Uses FMA to avoid intermediate rounding: `weight * activation + bias`.
pub fn coupling_strength(weight: f64, activation: f64, bias: f64) -> f64 {
    weight.mul_add(activation, bias)
}

/// Exponential moving average update.
pub fn ema_update(current: f64, new_sample: f64, alpha: f64) -> f64 {
    alpha.mul_add(new_sample - current, current)
}
```

## P6: Timestamp Newtype (No `chrono`/`SystemTime`)

All timestamps use a newtype wrapping monotonic `Instant` or epoch millis `u64`.

```rust
/// Monotonic timestamp in milliseconds since ORAC start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Create from elapsed milliseconds.
    pub fn from_millis(ms: u64) -> Self {
        Self(ms)
    }

    /// Milliseconds since epoch.
    pub fn as_millis(self) -> u64 {
        self.0
    }

    /// Duration between two timestamps.
    pub fn elapsed_since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}
```

## P7: Owned Returns Through `RwLock` (`.cloned()`, Never `&T`)

Never return references into locked data. Always `.cloned()` or `.to_owned()`.

```rust
impl BridgeRegistry {
    /// Get bridge config. Returns owned clone — lock is released before caller uses data.
    pub fn get_bridge(&self, name: &str) -> Option<BridgeConfig> {
        self.bridges.read().get(name).cloned()
    }

    /// List all bridge names. Returns owned `Vec`, not an iterator over locked data.
    pub fn list_bridges(&self) -> Vec<String> {
        self.bridges.read().keys().cloned().collect()
    }
}
```

## P8: Doc Comments with Backticked Identifiers and `# Errors`

Every public item has doc comments. Identifiers use backticks. Fallible functions document `# Errors`.

```rust
/// Inject field context from `PaneVortex` into an outbound `AgentRequest`.
///
/// Reads the current sphere field from the `FieldCache` and attaches
/// `order_parameter`, `k_effective`, and `phase_distribution` as
/// HTTP headers on the request.
///
/// # Errors
///
/// Returns `OracError::FieldUnavailable` if the `FieldCache` has no
/// data newer than `STALE_THRESHOLD_MS`.
/// Returns `OracError::Serialization` if header encoding fails.
pub fn inject_field_context(
    cache: &FieldCache,
    request: &mut AgentRequest,
) -> Result<(), OracError> {
    // ...
}
```

## P9: Signal/Event Emission on State Transitions

Every state transition emits an event. Observers never poll.

```rust
/// Agent lifecycle states with event emission.
pub fn transition(&self, agent_id: &AgentId, new_status: AgentStatus) -> Result<(), OracError> {
    let old_status = {
        let mut guard = self.agents.write();
        let agent = guard.get_mut(agent_id)
            .ok_or_else(|| OracError::NotFound(agent_id.clone()))?;
        let old = agent.status;
        agent.status = new_status;
        old
    }; // lock dropped

    // Emit event after lock release
    self.event_bus.emit(OracEvent::AgentTransition {
        agent_id: agent_id.clone(),
        from: old_status,
        to: new_status,
        timestamp: Timestamp::now(),
    });

    Ok(())
}
```

## P10: Feature-Gated Layers (`#[cfg(feature = "...")]`)

Optional layers compile-gate behind cargo features. Core (L1) and Wire (L2) are always on.

```rust
// In lib.rs
pub mod m1_core;
pub mod m2_wire;

#[cfg(feature = "api")]
pub mod m3_hooks;

#[cfg(feature = "intelligence")]
pub mod m4_intelligence;

#[cfg(feature = "bridges")]
pub mod m5_bridges;

pub mod m6_coordination; // always on — orchestrates whatever is compiled

#[cfg(feature = "monitoring")]
pub mod m7_monitoring;

#[cfg(feature = "evolution")]
pub mod m8_evolution;
```

```toml
# Cargo.toml
[features]
default = ["api", "intelligence", "bridges", "monitoring", "evolution"]
api = []
intelligence = []
bridges = ["dep:reqwest"]
monitoring = ["dep:opentelemetry"]
evolution = ["intelligence", "monitoring"]
```
