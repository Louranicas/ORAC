# STDP Pattern — Hebbian Spike-Timing Dependent Plasticity

> Tool chain weight learning. Strengthens frequently co-occurring tool sequences, weakens rare ones.

## Constants

| Parameter | Value | Notes |
|-----------|-------|-------|
| LTP rate | 0.01 | Long-term potentiation (strengthening) |
| LTP burst multiplier | 3x | Applied when tool pair fires 3+ times in window |
| LTP newcomer multiplier | 2x | Applied for first-time tool pairs |
| LTD rate | 0.002 | Long-term depression (weakening) |
| Weight floor | 0.05 | Minimum weight, prevents full decay |
| Weight ceiling | 1.0 | Maximum weight |
| Timing window | 5s | Max delta_t for STDP to apply |

## Weight Update Rule

```
if delta_t > 0 (pre fires before post — causal):
    dw = +LTP_RATE * exp(-delta_t / tau)
    if burst: dw *= 3.0
    if newcomer: dw *= 2.0

if delta_t < 0 (post fires before pre — anti-causal):
    dw = -LTD_RATE * exp(delta_t / tau)

w_new = clamp(w_old + dw, WEIGHT_FLOOR, 1.0)
```

Where:
- `delta_t = t_post - t_pre` (seconds)
- `tau = 2.5` (time constant)
- `pre` = previous tool, `post` = current tool

## Data Structures

```rust
struct StdpTracker {
    /// Adjacency weights: (pre_tool, post_tool) -> weight
    weights: HashMap<(String, String), f64>,
    /// Last fire time per tool
    last_fire: HashMap<String, Instant>,
    /// Fire count in current window per pair
    burst_count: HashMap<(String, String), u32>,
}
```

## Dominant Chain Detection

A tool chain `(pre, post)` is **dominant** when:
```
(uses >= 3 && r_delta > 0.02) || (uses >= 5 && co_occurrence >= 0.15)
```

Where:
- `uses` = number of times this pair fired in session
- `r_delta` = change in order parameter r after chain fires
- `co_occurrence` = pair_count / total_tool_calls

## Decay

- Per-step multiplicative decay: `w *= 0.995`
- Applied on every field tick to all weights
- Floor enforced after decay: `w = max(w, WEIGHT_FLOOR)`

## Memory Pruning

- Weights with `w < WEIGHT_FLOOR + 0.001` for 200+ steps are pruned
- Cap: 500 weight entries per sphere
- Pruning runs every 200 field ticks

## Integration Points

- **PostToolUse hook**: Fires STDP update via `ClientFrame::HebbianPulse`
- **SYNTHEX bridge**: Writes accumulated weights via `POST /v3/hebbian`
- **Evolution chamber**: `tool_chain_coherence` fitness dimension reads mean weight
- **Field coupling**: Dominant chains modulate effective K between sphere pairs

## Lookup Performance

- `set_weight()`: O(degree) — adjacency index update
- `get_weight()`: O(degree) — adjacency index lookup
- Full Hebbian tick: O(N^2 x degree) — N spheres
