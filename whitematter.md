# whitematter

Central nervous system white matter tracts — the brain's internal wiring.

Three tract systems modeled after biological white matter:

- **Association** — intra-hemispheric long-range bundles (e.g., arcuate fasciculus, superior longitudinal fasciculus). Connect distant cortical regions within the same hemisphere.
- **Commissural** — inter-hemispheric connections (e.g., corpus callosum zones, anterior commissure). Variable bandwidth, polarity bias (inhibitory genu, excitatory splenium).
- **Projection** — cortical ↔ subcortical ↔ brainstem vertical hierarchy (e.g., thalamocortical, corticostriatal). Regulatory, gating-dominant.

This crate handles brain-internal wiring. Peripheral nerves (brain ↔ body) are handled by [`fibertract-rs`](https://github.com/Blackfall-Labs/fibertract-rs).

## Architecture

```
Source Region                                           Target Region
     │                                                       ▲
     │ fired neuron indices                                  │ TractSpike delivery
     ▼                                                       │
┌─────────────────────────────────────────────────────────────┐
│                    WhiteMatterTract                         │
│                                                             │
│  MappingLUT ──► Chemical Modulation ──► Health Attenuation  │
│  (source→target   (DA, ACh, NE, 5HT)   (0=severed,        │
│   neuron map)                            255=perfect)       │
│       │                                                     │
│       ▼                                                     │
│  Polarity Bias ──► DelayBuffer ──► Bandwidth Cap            │
│  (commissural       (myelination-     (diameter limits      │
│   excit/inhib)       dependent         concurrent spikes)   │
│                      conduction)                            │
└─────────────────────────────────────────────────────────────┘
```

## Modules

| Module | Purpose |
|--------|---------|
| `tract` | `WhiteMatterTract` — core unit with `transmit()` and `deliver()` |
| `delay` | `DelayBuffer` — microsecond-indexed ring buffer for in-flight spikes |
| `mapping` | `MappingLUT` — deterministic source→target neuron mapping (topographic, convergent, divergent, random) |
| `modulation` | `ChemicalLevels` + `modulate_current()` — neuromodulator effects on signal transmission |
| `adapt` | `adapt_tract()` — use-dependent myelination, diameter, and health changes during consolidation |
| `plasticity` | `update_plasticity()` — three-factor Hebbian learning (pre × post × dopamine) |
| `catalog` | Named tract definitions — 6 association bundles, projection tracts (thalamocortical, corticostriatal, etc.) |
| `registry` | `TractRegistry` — owns all tracts, indexed lookup by ID, region, type, or name |

## Usage

```rust
use whitematter::*;

// Create a tract
let tract = WhiteMatterTract::new(TractConfig {
    name: "arcuate_fasciculus".to_string(),
    tract_type: TractType::Association,
    source_region: 0,
    target_region: 1,
    corridor_length: 20,
    myelination: 128,
    diameter: 64,
    excitatory_ratio: 128,
    default_zone: DendriticZone::Feedforward,
    source_n: 256,
    target_n: 256,
    mapping_kind: MappingKind::Topographic,
    density: 0.6,
    seed: 42,
});

// Register it
let mut registry = TractRegistry::new();
let id = registry.register(tract);

// Transmit spikes through the tract
let fired_neurons = vec![0u32, 5, 12];
let chemicals = ChemicalLevels::baseline();
let time_us = 1000;

let tract = registry.get_mut(id).unwrap();
let enqueued = tract.transmit(&fired_neurons, 500, &chemicals, time_us);

// Deliver spikes at target after delay
let arrival_time = time_us + tract.delay_us as u64;
let spikes = tract.deliver(arrival_time);
```

## Signal Flow

1. Source region neurons fire → `transmit()` receives fired neuron indices
2. `MappingLUT` maps source neurons to target neurons
3. Base current is chemically modulated (DA, ACh, NE, 5HT)
4. Health-based attenuation applied (damaged tracts weaken signals)
5. Commissural tracts apply polarity bias (excitatory vs inhibitory ratio)
6. Spikes enter `DelayBuffer` with myelination-dependent arrival time
7. `deliver()` drains spikes that have arrived at current time
8. Target region receives `TractSpike` with target neuron index, dendritic zone, and current

## Chemical Modulation

| Neuromodulator | Effect | Tract Affinity |
|----------------|--------|----------------|
| **Dopamine** | Reward-modulated gain | Full effect on projection tracts, half on others |
| **Acetylcholine** | Signal-to-noise gating | Strong signals amplified, weak suppressed |
| **Norepinephrine** | Global arousal boost | Uniform across all tract types |
| **Serotonin** | Inhibitory dampening | Only dampens when elevated, no boost when low |

## Adaptation

During sleep consolidation, `adapt_tract()` modifies physical properties:

- **Myelination**: High traffic → faster conduction. Low traffic → slow demyelination.
- **Diameter**: Sustained high traffic → more bandwidth capacity.
- **Health**: Moderate use maintains health. Complete disuse → gradual degradation.

## Plasticity

Three-factor Hebbian rule via `update_plasticity()`:

1. **Pre-synaptic**: source neuron projected through this tract
2. **Post-synaptic**: target neuron actually fired
3. **Dopamine gate**: DA must be above baseline for plasticity to occur

High correlation (>30%) strengthens the tract (myelination bump). Low correlation with sufficient sample size weakens it (health penalty).

## Dependencies

- [`ternary-signal`](https://github.com/Blackfall-Labs/ternary-signal) — ternary signal types (polarity × magnitude × multiplier)
- `log` — structured logging

## Part of [Blackfall Labs](https://github.com/Blackfall-Labs)

Neuromorphic systems. Not machine learning.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
