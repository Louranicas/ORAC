# ORAC Evolution Specification — RALPH Chamber

> 5-phase evolutionary loop. Feature-gated: `#[cfg(feature = "evolution")]`

## Overview

RALPH (Recognize-Analyze-Learn-Propose-Harvest) is an autonomous evolution chamber that discovers optimal configurations through multi-parameter mutation, fitness evaluation, and diversity-enforced selection.

**Feature gate**: `#[cfg(feature = "evolution")]` — entire module compiles out when disabled.

## 5-Phase Loop

```
  +-----------+     +-----------+     +---------+
  | Recognize |---->|  Analyze  |---->|  Learn  |
  +-----------+     +-----------+     +---------+
       ^                                   |
       |                                   v
  +-----------+     +-----------+
  |  Harvest  |<----|  Propose  |
  +-----------+     +-----------+
```

### Phase 1: Recognize

Detect emergent patterns in the live system.

- **Emergence detector**: Ring buffer of events, capacity 5000
- **TTL decay**: Events older than 300s are evicted
- **Pattern types**: tool chain dominance, phase clustering, K oscillation, sync anomalies
- **Input**: field state (r, K, phases), tool events, Hebbian weights
- **Output**: `Vec<EmergentPattern>` with confidence scores

### Phase 2: Analyze

Correlate patterns to discover causal pathways.

- **Correlation engine**: Pairwise pattern correlation with lag analysis
- **Pathway discovery**: Sequences of events that reliably precede fitness changes
- **Temporal window**: 60-tick sliding window
- **Output**: `Vec<Correlation>` with strength and direction

### Phase 3: Learn

Evaluate current configuration fitness.

- **Fitness tensor**: 12-dimensional weighted evaluation

| Dimension | Weight | Source |
|-----------|--------|--------|
| latency_p50 | 0.10 | metrics |
| latency_p99 | 0.15 | metrics |
| error_rate | 0.15 | metrics |
| throughput | 0.10 | metrics |
| sync_quality (r) | 0.10 | field |
| chimera_rate | 0.05 | field |
| memory_efficiency | 0.05 | POVM bridge |
| tool_chain_coherence | 0.10 | STDP weights |
| consent_compliance | 0.05 | policy |
| diversity_index | 0.05 | evolution |
| bridge_health | 0.05 | bridges |
| coupling_stability | 0.05 | field |

- **Fitness score**: Weighted sum, range [0.0, 1.0]
- **Fitness history**: Ring buffer of 100 evaluations for trend analysis

### Phase 4: Propose

Generate candidate configurations via mutation.

- **Multi-parameter mutation** (BUG-035 fix: NOT mono-parameter)
  - Each proposal mutates 2-5 parameters simultaneously
  - Mutation magnitude: Gaussian, sigma = 0.1 * parameter range
  - Correlated mutations: parameters discovered in Analyze phase mutate together

- **Mutable parameters**:

| Parameter | Range | Default |
|-----------|-------|---------|
| K (coupling) | [0.01, 50.0] | 2.42 |
| k_mod | [-0.5, 1.5] | 1.0 |
| STDP LTP rate | [0.001, 0.1] | 0.01 |
| STDP LTD rate | [0.0005, 0.05] | 0.002 |
| weight_floor | [0.01, 0.2] | 0.05 |
| sync_threshold | [0.3, 0.9] | 0.5 |
| tunnel_threshold | [0.5, 1.2] | 0.8 |
| tick_rate_ms | [50, 500] | 100 |
| circuit_breaker_threshold | [3, 20] | 5 |
| keepalive_interval_s | [10, 120] | 30 |

- **Diversity enforcement**:
  - Round-robin parameter selection across proposals
  - 10-generation cooldown: parameter cannot be primary mutation target for 10 gens after selection
  - 50% diversity threshold: if >50% of proposals share a mutation axis, force diversity injection
  - Population size: 8 candidates per generation

### Phase 5: Harvest

Select winning configuration, apply or rollback.

- **Tournament selection**: Best-of-3 from population
- **Elitism**: Top candidate always survives to next generation
- **Application**: Winner's parameters are hot-swapped into live config
- **Rollback trigger**: If fitness drops >10% within 30 ticks of application, revert to snapshot
- **Snapshot**: Atomic state capture before every application

## Snapshot + Rollback

```rust
struct EvolutionSnapshot {
    timestamp_ms: u64,
    generation: u32,
    params: ParameterSet,
    fitness: f64,
    field_state: FieldSnapshot,
}
```

- Snapshots stored in ring buffer, capacity 10
- Rollback restores: parameters, field coupling weights, circuit breaker states
- Rollback does NOT restore: session state, Hebbian weights (those are append-only)

## Emergence Detector

```rust
struct EmergenceDetector {
    events: VecDeque<TimestampedEvent>,  // ring buffer, cap 5000
    ttl: Duration,                        // 300s
    patterns: Vec<EmergentPattern>,
}
```

- Events are inserted on every field tick, tool use, and bridge response
- Expired events (age > TTL) are drained on each `detect()` call
- Pattern matching runs after drain, O(N) scan

## Convergence Criteria

Evolution loop pauses when:
- Fitness stable (variance < 0.001 over 50 generations)
- No emergent patterns detected for 100 ticks
- Manual pause via `orac-ctl evolution pause`

Loop resumes when:
- Fitness drops >5% from stable baseline
- New emergent pattern detected
- Manual resume via `orac-ctl evolution resume`
