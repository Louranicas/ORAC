# SYNTHEX Thermal Analysis Report

**Date:** 2026-04-03 14:24-14:26 UTC
**Analyst:** Claude Opus 4.6 (automated deep analysis)
**Endpoint:** localhost:8090/v3/thermal, /v3/diagnostics, /v3/health, /api/health
**Status:** THERMAL ANOMALY -- temperature above target, rising trend

---

## 1. Executive Summary

The SYNTHEX V3 thermal system is running significantly hot. Temperature was observed
rising from 0.494 to 0.765 over approximately 2 minutes, well above the 0.50 target.
The PID controller is responding (output 0.067 -> 0.232) but the cooling mechanisms
are insufficient to counteract the heat sources. Two sources are the primary drivers:

- **HS-004 CrossSync: pegged at 1.0 (maximum) across all samples** -- never moves
- **HS-002 Cascade: extremely volatile (0.133 -> 1.000)** -- highest weighted source

The system is in a state where the PID controller's adjustments (decay rate multiplier,
damping adjustment) cannot reach the heat sources fast enough to reduce temperature.

---

## 2. Time Series Data (5 samples, ~2min window)

```
Time       Temp    Target  Delta    PID      Trend
---------- ------- ------- -------- -------- --------
14:24:13   0.4941  0.5000  -0.0059  0.0670   --
14:24:35   0.7183  0.5000  +0.2183  0.2114   RISING
14:24:41   0.7183  0.5000  +0.2183  0.2091   STABLE
14:24:56   0.7390  0.5000  +0.2390  0.2197   RISING
14:26:13   0.7646  0.5000  +0.2646  0.2320   RISING
```

**Verdict:** Temperature is on a RISING trend, +0.271 over 2 minutes.
The brief stability at 14:24:41 was between ingest events, not actual cooling.

---

## 3. Heat Source Analysis

### 3.1 Readings Over Time

```
Time       Hebbian   Cascade   Resonance CrossSync
---------- --------- --------- --------- ---------
14:24:13   0.5436    0.1333    0.6720    1.0000
14:24:35   0.5518    0.7667    0.6720    1.0000
14:24:41   0.5518    0.7667    0.6720    1.0000
14:24:56   0.3581    1.0000    0.6576    1.0000
14:26:13   0.6381    0.8333    0.6576    1.0000
```

### 3.2 Weighted Contributions (reading x weight)

```
Time       Hebbian   Cascade   Resonance CrossSync  TOTAL
           (w=0.30)  (w=0.35)  (w=0.20)  (w=0.15)
---------- --------- --------- --------- --------- ------
14:24:13   0.1631    0.0467    0.1344    0.1500    0.4941
14:24:35   0.1655    0.2683    0.1344    0.1500    0.7183
14:24:56   0.1074    0.3500    0.1315    0.1500    0.7390
14:26:13   0.1914    0.2917    0.1315    0.1500    0.7646
```

### 3.3 Source Volatility

```
Source       Min     Max     Range   Weight  Max Contribution
------------ ------- ------- ------- ------- ----------------
Hebbian      0.3581  0.6381  0.2800  0.30    0.1914
Cascade      0.1333  1.0000  0.8667  0.35    0.3500  <-- DOMINANT
Resonance    0.6576  0.6720  0.0144  0.20    0.1344
CrossSync    1.0000  1.0000  0.0000  0.15    0.1500  <-- PEGGED
```

---

## 4. Root Cause Analysis

### 4.1 HS-004 CrossSync: Pegged at 1.0 (CRITICAL)

CrossSync has been at maximum (1.0) across every sample. Source code analysis reveals
it is fed by two paths:

**Path A (v3_ingest_handler):** Falls back to sphere count when `nexus_health` is absent:
```rust
let cross_sync = payload.get("nexus_health")
    .unwrap_or_else(|| {
        if spheres > 1 { (spheres as f64 / 10.0).clamp(0.1, 1.0) }
        else { 0.2 }
    });
```
With 10+ spheres, `spheres/10.0` saturates at 1.0.

**Path B (nexus_push_handler):** STDP shift events pass `new_w` directly:
```rust
let _ = sys.thermal().update_source("HS-004", new_w.clamp(0.0, 1.0));
```
Hebbian weights near 1.0 pass through unclamped.

**Root cause:** CrossSync saturates because sphere count >= 10 maps to 1.0 via the
`spheres/10` formula, and no `nexus_health` field is being sent by ORAC. Additionally,
STDP shift events with high weights keep resetting it to near-1.0.

**Contribution to overheating:** 0.15 constant added to every temperature reading.
With target at 0.50, this means 30% of the target is consumed by CrossSync alone,
leaving only 0.35 of headroom for the other three sources.

### 4.2 HS-002 Cascade: Extreme Volatility (HIGH)

Cascade oscillates wildly (0.133 to 1.000) because it is fed by two different code paths:

**Path A (v3_ingest_handler):** Optional `cascade_heat` from ORAC ingest payload
**Path B (nexus_push_handler):** Emergence events map severity to discrete heat values:
  - CRITICAL -> 0.9
  - HIGH -> 0.6
  - MEDIUM -> 0.3
  - LOW -> 0.1

**Root cause:** The Cascade source has no smoothing or decay. Each event overwrites the
previous value wholesale. A single CRITICAL emergence event instantly sets cascade heat
to 0.9. Combined with its highest weight (0.35), this drives temperature spikes.

**Contribution to overheating:** At maximum (1.0), Cascade contributes 0.350 to
temperature -- 70% of the target by itself.

### 4.3 HS-001 Hebbian: Moderate, Tracks Field Coherence (OK)

Hebbian tracks field coherence `r` (from Pane-Vortex Kuramoto field). Readings range
0.358-0.638 -- reasonable and roughly centered around target. This source is working
as designed.

### 4.4 HS-003 Resonance: Stable, Mild Overshoot (OK)

Resonance is fed by ME RALPH fitness (0.658-0.672). This is stable but slightly above
what the weight budget allows. With target=0.5, the ideal per-source reading would be
0.5/weight_sum = 0.5. Resonance at ~0.66 is modestly hot but not a problem in isolation.

---

## 5. PID Controller Analysis

### 5.1 Controller Parameters (from V3 config)

```
Kp = 0.5    (proportional gain)
Ki = 0.1    (integral gain)
Kd = 0.05   (derivative gain)
Target = 0.5
Integral clamp = 1.0
```

### 5.2 PID Response

```
Time       Error    PID Out  Decay Mult  Damp Adj   Maint Signal
---------- -------- -------- ----------- ---------- ------------
14:24:13   -0.006   0.067    1.067       -0.007     false
14:24:35   +0.218   0.211    1.211       -0.021     false
14:24:56   +0.239   0.220    1.220       -0.022     false
14:26:13   +0.265   0.232    1.232       -0.023     true
```

The PID is generating appropriate control signals:
- `decay_rate_multiplier` is increasing (1.07 -> 1.23), meaning faster STDP decay
- `damping_adjustment` is becoming more negative (-0.007 -> -0.023), reducing cascade
- `signal_maintenance` triggered at 14:26:13 (temp > 0.75 threshold)
- `trigger_pattern_gc` has NOT triggered (requires temp > 0.85)

### 5.3 Why PID Cannot Control Temperature

The PID output generates *recommendations*, not direct actuations. The cooling
adjustments work only through:

1. **CM-001 STDP Decay** -- increases `decay_rate_multiplier`, but this only affects
   Hebbian weights during the next decay cycle (every 3600s = 1 hour by default)
2. **CM-002 Cascade Damping** -- adjusts damping factor, but new events overwrite
   the Cascade reading entirely
3. **CM-003 Pattern GC** -- not triggered (temp < 0.85)
4. **CM-004 Maintenance Signal** -- just triggered, but external ME must act on it

**The feedback loop is broken.** The PID is responding correctly, but its output
cannot reach the heat sources fast enough. Cascade values are overwritten by the next
Nexus event, and CrossSync is hardcoded to 1.0 via the sphere fallback.

---

## 6. Diagnostics Summary

```
Probe                Value          Severity   Thresholds (warn/crit)
-------------------- -------------- ---------- ----------------------
PatternCount         1.0            Ok         50 / 75
CascadeAmplification 1.0e-132       Ok         150 / 500
Latency              10.0ms         Ok         500 / 1000
Synergy              0.753          Warning    0.9 / 0.7
```

- Overall health: 0.875 (1 warning out of 4 probes)
- Synergy is at 0.753 (WARNING) -- below the 0.9 warning threshold
- Circuit breakers: all 12 closed (no trips)
- Decay cycles: 9 completed (healthy)

---

## 7. Recommendations

### R1: Fix CrossSync Saturation (CRITICAL, ~5 LOC)

The `spheres / 10.0` fallback saturates at 1.0 when >= 10 spheres are registered.
Change the denominator or apply a logarithmic scaling:

**Option A (quick fix):** Change divisor from 10 to 20:
```rust
// Before:
(spheres as f64 / 10.0).clamp(0.1, 1.0)
// After:
(spheres as f64 / 20.0).clamp(0.1, 0.8)
```

**Option B (better):** Have ORAC send a real `nexus_health` aggregate value so the
fallback is never used. This requires adding a `nexus_health` field to the ORAC
ingest payload.

**Impact:** Reduces CrossSync from 1.0 to ~0.5-0.8, freeing 0.03-0.075 of thermal
headroom.

### R2: Add Exponential Moving Average to Cascade (HIGH, ~10 LOC)

Cascade readings should be smoothed with an EMA to prevent single events from
driving wild oscillations:

```rust
// In update_source for HS-002:
let alpha = 0.3; // smoothing factor
new_reading = alpha * incoming + (1.0 - alpha) * current_reading;
```

**Impact:** Prevents single CRITICAL emergence events from instantly setting Cascade
to 0.9. Temperature swings would be dampened by 70%.

### R3: Reduce Cascade Weight (MEDIUM, ~1 LOC)

Cascade at weight 0.35 is the single largest contributor. Given its volatility,
consider reducing to 0.25 and redistributing to Resonance (0.30):

```
Current:  Hebbian=0.30  Cascade=0.35  Resonance=0.20  CrossSync=0.15
Proposed: Hebbian=0.30  Cascade=0.25  Resonance=0.30  CrossSync=0.15
```

**Impact:** Reduces Cascade's maximum contribution from 0.350 to 0.250.

### R4: Tighten PID Gains (LOW, ~2 LOC)

The current Kp=0.5, Ki=0.1 produces output ~0.23 at error +0.26. This is proportional
but the cooling mechanisms are too slow. Consider:

- Increase Kp to 0.8 for faster proportional response
- Increase Ki to 0.2 for stronger integral accumulation (addresses sustained error)

### R5: Close the Cooling Feedback Loop (MEDIUM, ~15 LOC)

The PID's `damping_adjustment` modifies the cascade damping factor, but this only
affects the *next cascade processing* -- it does not reduce the thermal reading of
HS-002 directly. Add a mechanism where the PID output directly attenuates heat
source readings when temperature is above threshold:

```rust
// In tick_thermal or thermal_snapshot:
if pid_output > 0.1 {
    // Actively cool by clamping volatile sources
    let attenuation = 1.0 - (pid_output * 0.2).min(0.5);
    self.update_source("HS-002", current_hs002 * attenuation);
}
```

### R6: Reduce Decay Interval (LOW, config change)

Current decay interval is 3600s (1 hour). With 9 cycles in ~40+ hours of uptime,
the hourly decay is too infrequent to respond to thermal events. Consider reducing
to 600s (10 minutes) for faster thermal response via the Hebbian decay pathway.

---

## 8. Priority Matrix

```
Priority  Recommendation            Impact   Effort  Risk
--------- ------------------------- -------- ------- -----
P0        R1 Fix CrossSync fallback HIGH     5 LOC   LOW
P0        R2 EMA on Cascade         HIGH     10 LOC  LOW
P1        R3 Reduce Cascade weight  MEDIUM   1 LOC   LOW
P1        R5 Close cooling loop     MEDIUM   15 LOC  MED
P2        R4 Tighten PID gains      LOW      2 LOC   LOW
P2        R6 Reduce decay interval  LOW      config  LOW
```

Total estimated work: ~33 LOC + 1 config change.

---

## 9. Raw Data Archive

### Sample 1 (14:24:13 UTC)
```json
{"temperature":0.4941,"target":0.5,"pid_output":0.0670,
 "heat_sources":{"Hebbian":0.5436,"Cascade":0.1333,"Resonance":0.6720,"CrossSync":1.0},
 "adjustments":{"decay_rate_multiplier":1.067,"damping_adjustment":-0.007,"signal_maintenance":false,"trigger_pattern_gc":false}}
```

### Sample 2 (14:24:35 UTC)
```json
{"temperature":0.7183,"target":0.5,"pid_output":0.2114,
 "heat_sources":{"Hebbian":0.5518,"Cascade":0.7667,"Resonance":0.6720,"CrossSync":1.0},
 "adjustments":{"decay_rate_multiplier":1.211,"damping_adjustment":-0.021,"signal_maintenance":false,"trigger_pattern_gc":false}}
```

### Sample 3 (14:24:41 UTC)
```json
{"temperature":0.7183,"target":0.5,"pid_output":0.2091,
 "heat_sources":{"Hebbian":0.5518,"Cascade":0.7667,"Resonance":0.6720,"CrossSync":1.0},
 "adjustments":{"decay_rate_multiplier":1.209,"damping_adjustment":-0.021,"signal_maintenance":false,"trigger_pattern_gc":false}}
```

### Sample 4 (14:24:56 UTC)
```json
{"temperature":0.7390,"target":0.5,"pid_output":0.2197,
 "heat_sources":{"Hebbian":0.3581,"Cascade":1.0000,"Resonance":0.6576,"CrossSync":1.0},
 "adjustments":{"decay_rate_multiplier":1.220,"damping_adjustment":-0.022,"signal_maintenance":false,"trigger_pattern_gc":false}}
```

### Sample 5 (14:26:13 UTC)
```json
{"temperature":0.7646,"target":0.5,"pid_output":0.2320,
 "heat_sources":{"Hebbian":0.6381,"Cascade":0.8333,"Resonance":0.6576,"CrossSync":1.0},
 "adjustments":{"decay_rate_multiplier":1.232,"damping_adjustment":-0.023,"signal_maintenance":true,"trigger_pattern_gc":false}}
```

### Diagnostics (14:26:13 UTC)
```json
{"overall_health":0.875,"warning_count":1,"critical_count":0,
 "probes":[
   {"name":"PatternCount","value":1.0,"severity":"Ok"},
   {"name":"CascadeAmplification","value":1.0e-132,"severity":"Ok"},
   {"name":"Latency","value":10.0,"severity":"Ok"},
   {"name":"Synergy","value":0.753,"severity":"Warning"}]}
```

### V3 Health (14:26:13 UTC)
```json
{"operational":true,"decay_cycles":9,"breaker_summary":[12,0,0],
 "temperature":0.7646,"thermal_target":0.5,"diagnostics_health":0.875,
 "config":{"decay_rate":0.1,"decay_interval_secs":3600,"min_strength":0.1,
           "max_pathways":100,"cascade_damping_factor":0.6,
           "thermal_target":0.5,"thermal_kp":0.5,"thermal_ki":0.1}}
```
