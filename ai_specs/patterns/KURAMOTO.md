# Kuramoto Pattern — Phase Oscillator Dynamics

> Coupled oscillators for fleet synchronization. Each sphere is an oscillator with natural frequency, phase, and coupling.

## Order Parameter

The order parameter `r` measures global synchronization:

```
r * exp(i * psi) = (1/N) * sum(exp(i * theta_j))
```

Where:
- `r` in [0, 1]: 0 = incoherent, 1 = perfect sync
- `psi`: mean phase angle
- `theta_j`: phase of sphere j
- `N`: number of spheres

## Phase Update

Per tick, each sphere's phase evolves:

```
d(theta_i)/dt = omega_i + (K / N) * sum(w_ij * sin(theta_j - theta_i))
```

Where:
- `omega_i`: natural frequency of sphere i
- `K`: global coupling strength
- `w_ij`: Hebbian weight between spheres i and j (from STDP)
- Sum runs over all spheres j != i

## Key Parameters

| Parameter | Range | Default | Notes |
|-----------|-------|---------|-------|
| K (coupling) | [0.01, 50.0] | 2.42 | Global coupling strength |
| k_mod | [-0.5, 1.5] | 1.0 | Per-sphere coupling modifier |
| effective_K | computed | K * k_mod | Actual coupling applied |
| SYNC_THRESHOLD | - | 0.5 | r above this = synchronized |
| TUNNEL_THRESHOLD | - | 0.8 rad | Phase diff below this = tunneled |
| auto_scale_k multiplier | - | 0.5 | K scaling factor |
| weight exponent | - | w^2 | Fixed quadratic (was 1+k_mod) |

## Per-Status K Modulation

Coupling modifier depends on sphere status pairs:

| Status Pair | k_mod Multiplier | Rationale |
|-------------|-------------------|-----------|
| Working <-> Working | 1.2x | Active spheres should sync |
| Idle <-> Working | 0.5x | Idle sphere shouldn't drag workers |
| Idle <-> Idle | 0.8x | Mild coupling for idle fleet |
| Blocked <-> Any | 0.0x | Blocked sphere decouples entirely |

## Chimera Detection

A chimera is a split field: some spheres synchronized, others incoherent.

**Detection algorithm** (O(N log N)):
1. Sort spheres by phase
2. Compute phase gaps between adjacent spheres
3. Gaps > pi/3 indicate cluster boundaries
4. If 2+ clusters exist with sizes > 1: chimera detected
5. Iteration-guarded cluster loop (max N iterations)

**Response**:
- Emit `field.chimera` event on PV2 bus
- Inject field summary into next `UserPromptSubmit` hook response
- Evolution chamber records as fitness signal

## Auto-Scale K

K adapts to maintain useful synchronization without over-coupling:

```rust
fn auto_scale_k(r: f64, current_k: f64) -> f64 {
    let target_r = 0.85;
    let error = target_r - r;
    let adjustment = error * 0.5;  // multiplier
    clamp(current_k + adjustment, 0.01, 50.0)
}
```

- If r too high (>0.99): reduce K to allow differentiation
- If r too low (<0.5): increase K to restore coherence
- Critical insight: "Synchronization without differentiation = conformity"

## Cached Field

`AppState.cached_field` stores pre-computed field state to avoid redundant O(N^2 x B^2) computation per API request. Invalidated on:
- Sphere registration/deregistration
- Phase update (tick)
- K modification

## Ghost Traces

When a sphere deregisters:
- Ghost trace created with final phase, tool count, duration
- FIFO buffer, max 20 ghosts
- Accessible via `GET /field/ghosts`
- Ghosts do NOT participate in coupling (excluded from phase update sum)

## Semantic Phase Mapping

Tool categories map to phase regions for meaningful coupling:

| Category | Phase | Tools |
|----------|-------|-------|
| Read | 0 | Read, Glob, Grep |
| Write | pi/2 | Edit, Write |
| Execute | pi | Bash, Skill |
| Communicate | 3*pi/2 | WebFetch, mcp__* |

Spheres using similar tools naturally cluster in phase space.

## Activation

Per-sphere activation tracks engagement:
- Decay: `activation *= 0.995` per tick
- Boost: `activation += 0.05` on tool use
- Threshold: spheres below 0.3 activation are candidates for pruning
