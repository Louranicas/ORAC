# Circuit Breaker Pattern — ORAC

> FSM protecting bridge calls. Prevents cascade failures from unhealthy upstreams.

## State Machine

```
         success          failure_count >= threshold
  +--------+--------+        +--------+--------+
  |                  |        |                  |
  v                  |        v                  |
Closed ----fail----> Closed  Closed ----------> Open
  ^                                              |
  |                                              | cooldown expires
  |           success                            v
  +<-------------- HalfOpen <--------------------+
  |                    |
  |                    | failure
  |                    v
  |                  Open
  +---(reset)--------^
```

## States

| State | Behavior |
|-------|----------|
| **Closed** | All requests pass through. Failures counted. |
| **Open** | All requests immediately fail with `503`. No upstream calls. |
| **HalfOpen** | One probe request allowed. Success -> Closed. Failure -> Open. |

## Configuration

```rust
struct CircuitBreakerConfig {
    /// Failures before tripping to Open
    failure_threshold: u32,       // default: 5
    /// Duration to stay Open before probing
    cooldown: Duration,           // default: 30s
    /// Window for counting failures
    failure_window: Duration,     // default: 60s
    /// Consecutive successes in HalfOpen to close
    success_threshold: u32,       // default: 2
}
```

## Per-Bridge Defaults

| Bridge | failure_threshold | cooldown | failure_window |
|--------|-------------------|----------|----------------|
| SYNTHEX | 5 | 30s | 60s |
| ME | 5 | 30s | 60s |
| POVM | 3 | 20s | 30s |
| RM | 10 | 60s | 120s |

## Implementation

```rust
enum BreakerState {
    Closed {
        failure_count: u32,
        window_start: Instant,
    },
    Open {
        opened_at: Instant,
    },
    HalfOpen {
        success_count: u32,
    },
}

impl CircuitBreaker {
    /// Check if request is allowed
    pub fn allow(&self) -> bool { ... }

    /// Record success
    pub fn record_success(&mut self) { ... }

    /// Record failure
    pub fn record_failure(&mut self) { ... }

    /// Current state for metrics
    pub fn state(&self) -> &BreakerState { ... }
}
```

## Metrics

Exposed at `/metrics`:
- `orac_circuit_breaker_state{bridge}` — gauge: 0=closed, 1=open, 2=half_open
- `orac_circuit_breaker_trips_total{bridge}` — counter: transitions to Open
- `orac_circuit_breaker_probes_total{bridge,result}` — counter: HalfOpen probe outcomes

## Integration Points

- Each bridge client wraps calls with `breaker.allow()` / `breaker.record_*()`
- Open breaker returns `BridgeError::CircuitOpen` (maps to HTTP 503)
- Evolution chamber reads breaker state as fitness dimension (`bridge_health`)
- Hook server degrades gracefully when breakers are open (local-only processing)
