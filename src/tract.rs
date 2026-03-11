//! White matter tract — the core unit of inter-region communication.

use crate::delay::DelayBuffer;
use crate::mapping::{MappingKind, MappingLUT};
use crate::modulation::{modulate_current, ChemicalLevels};

/// Three white matter tract systems.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TractType {
    /// Intra-hemispheric long-range bundles.
    /// Connect distant cortical regions within the same hemisphere.
    Association,

    /// Inter-hemispheric connections (corpus callosum, anterior/hippocampal commissure).
    /// Variable bandwidth, polarity bias (inhibitory genu, excitatory splenium).
    Commissural,

    /// Cortex ↔ subcortical ↔ brainstem vertical hierarchy.
    /// Regulatory, gating-dominant.
    Projection,
}

/// Which dendritic zone a spike targets on the receiving neuron.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DendriticZone {
    /// Bottom-up / outside input.
    Feedforward,
    /// Lateral / same-region neighbors.
    Context,
    /// Top-down / modulatory.
    Feedback,
}

/// A spike propagating through a white matter tract.
#[derive(Clone, Copy, Debug)]
pub struct TractSpike {
    /// Which neuron in the source region fired.
    pub source_neuron: u32,
    /// Which neuron in the target region receives (from MappingLUT).
    pub target_neuron: u32,
    /// Which dendritic zone the spike targets.
    pub zone: DendriticZone,
    /// Signal current (attenuated, chemically modulated). Uses i32 for
    /// full Signal multiplier range (effective magnitude up to ±65,025).
    pub current: i32,
    /// When this spike arrives at the target (source_time + tract_delay).
    pub arrival_time_us: u64,
}

/// Opaque region identifier. Sentinel assigns these during boot.
pub type RegionId = u16;

/// Configuration for a white matter tract at construction time.
pub struct TractConfig {
    pub name: String,
    pub tract_type: TractType,
    pub source_region: RegionId,
    pub target_region: RegionId,
    /// Skull corridor length in voxel units.
    pub corridor_length: u16,
    /// Initial myelination level (0-255).
    pub myelination: u8,
    /// Signal capacity per cascade cycle.
    pub diameter: u8,
    /// Commissural polarity bias. 128 = balanced,
    /// >128 = excitatory-dominant, <128 = inhibitory-dominant.
    /// Ignored for non-commissural tracts.
    pub excitatory_ratio: u8,
    /// Default dendritic zone for spike delivery.
    pub default_zone: DendriticZone,
    /// Source and target pool neuron counts (for MappingLUT).
    pub source_n: u32,
    pub target_n: u32,
    /// Mapping strategy.
    pub mapping_kind: MappingKind,
    /// Fraction of source neurons that project (0.0–1.0).
    pub density: f32,
    /// Seed for deterministic mapping.
    pub seed: u64,
}

/// A white matter tract connecting two brain regions.
pub struct WhiteMatterTract {
    // -- Identity --
    pub name: String,
    pub tract_type: TractType,
    pub source_region: RegionId,
    pub target_region: RegionId,

    // -- Physical properties --
    /// Skull corridor length in voxel units.
    pub corridor_length: u16,
    /// Bandwidth capacity (spikes per cascade cycle).
    pub diameter: u8,
    /// Myelination level (0-255). Higher = faster conduction.
    pub myelination: u8,
    /// Tract health (0 = severed, 255 = perfect).
    pub health: u8,

    // -- Conduction --
    /// Base delay in microseconds, derived from corridor_length / velocity.
    pub delay_us: u32,

    // -- Polarity bias (commissural) --
    /// 128 = balanced. >128 = excitatory-dominant. <128 = inhibitory-dominant.
    pub excitatory_ratio: u8,

    // -- Default delivery zone --
    pub default_zone: DendriticZone,

    // -- Mapping --
    pub mapping: MappingLUT,

    // -- Adaptation state --
    /// Rolling average of recent traffic (0-255).
    pub traffic_density: u8,
    /// Total spikes ever transmitted.
    pub lifetime_spikes: u64,
    /// Spikes this consolidation period (reset on adapt).
    pub period_spikes: u64,

    // -- Transmission pipeline --
    delay_buffer: DelayBuffer,
}

/// Base conduction velocity by tract type (arbitrary units, microseconds per voxel).
/// Lower = faster.
fn base_velocity_us_per_voxel(tract_type: TractType) -> u32 {
    match tract_type {
        // Association: moderate speed
        TractType::Association => 200,
        // Commissural: varies, use moderate baseline
        TractType::Commissural => 250,
        // Projection: moderate-fast (thalamocortical relays)
        TractType::Projection => 150,
    }
}

/// Compute conduction delay from corridor length and myelination.
///
/// `delay = corridor_length * base_velocity / (1 + myelination_factor)`
///
/// Myelination up to 2x speedup. Minimum delay = 1us.
fn compute_delay(corridor_length: u16, tract_type: TractType, myelination: u8) -> u32 {
    let base = base_velocity_us_per_voxel(tract_type);
    // myelination 0 → divisor 256, myelination 255 → divisor 511 (~2x speedup)
    let divisor = 256 + myelination as u32;
    let delay = (corridor_length as u32 * base * 256) / divisor;
    delay.max(1)
}

impl WhiteMatterTract {
    /// Create a new tract from configuration.
    pub fn new(config: TractConfig) -> Self {
        let delay_us = compute_delay(
            config.corridor_length,
            config.tract_type,
            config.myelination,
        );

        // Delay buffer capacity: enough to hold max delay + margin.
        // Convert delay_us to a reasonable slot count.
        let buffer_capacity = (delay_us as usize / 50).max(64).min(4096);

        let mapping = MappingLUT::build(
            config.source_n,
            config.target_n,
            &config.mapping_kind,
            config.density,
            config.seed,
        );

        Self {
            name: config.name,
            tract_type: config.tract_type,
            source_region: config.source_region,
            target_region: config.target_region,
            corridor_length: config.corridor_length,
            diameter: config.diameter,
            myelination: config.myelination,
            health: 255,
            delay_us,
            excitatory_ratio: config.excitatory_ratio,
            default_zone: config.default_zone,
            mapping,
            traffic_density: 0,
            lifetime_spikes: 0,
            period_spikes: 0,
            delay_buffer: DelayBuffer::new(buffer_capacity),
        }
    }

    /// Transmit spikes from source region through this tract.
    ///
    /// `fired` is a slice of source neuron indices that spiked.
    /// `base_current` is the base signal current (from Signal.current()).
    /// `chemicals` provides neuromodulator levels for modulation.
    /// `time_us` is the current simulation time in microseconds.
    ///
    /// Returns the number of spikes enqueued.
    pub fn transmit(
        &mut self,
        fired: &[u32],
        base_current: i32,
        chemicals: &ChemicalLevels,
        time_us: u64,
    ) -> u32 {
        if self.health == 0 {
            return 0; // severed
        }

        // Chemical modulation
        let modulated = modulate_current(base_current, self.tract_type, chemicals);

        // Health-based attenuation
        let attenuated = (modulated as i64 * self.health as i64 / 255) as i32;

        if attenuated == 0 {
            return 0;
        }

        let arrival = time_us + self.delay_us as u64;
        let mut enqueued = 0u32;
        let capacity = self.diameter as u32;

        for &src_idx in fired {
            if enqueued >= capacity {
                // Bandwidth saturated — biologically valid drop
                log::trace!(
                    "tract '{}': bandwidth saturated, dropping spikes",
                    self.name
                );
                break;
            }

            let targets = self.mapping.targets(src_idx);
            for &tgt_idx in targets {
                if enqueued >= capacity {
                    break;
                }

                // Commissural polarity bias: flip current polarity for inhibitory-biased tracts
                let biased_current = if self.tract_type == TractType::Commissural {
                    apply_polarity_bias(attenuated, self.excitatory_ratio)
                } else {
                    attenuated
                };

                self.delay_buffer.enqueue(TractSpike {
                    source_neuron: src_idx,
                    target_neuron: tgt_idx,
                    zone: self.default_zone,
                    current: biased_current,
                    arrival_time_us: arrival,
                });

                enqueued += 1;
            }
        }

        self.lifetime_spikes += enqueued as u64;
        self.period_spikes += enqueued as u64;

        // Update traffic density (exponential moving average)
        let traffic_sample = (enqueued as u16).min(255) as u8;
        self.traffic_density = ema_u8(self.traffic_density, traffic_sample, 16);

        enqueued
    }

    /// Deliver all spikes due at `current_time_us`.
    pub fn deliver(&mut self, current_time_us: u64) -> Vec<TractSpike> {
        self.delay_buffer.drain_due(current_time_us)
    }

    /// Deliver all spikes due within the time range `[from_us, to_us]`.
    ///
    /// Unlike `deliver()` which only checks a single slot, this method
    /// visits all slots in the range, ensuring no spikes are missed
    /// when the caller advances in steps larger than the slot granularity.
    pub fn deliver_range(&mut self, from_us: u64, to_us: u64) -> Vec<TractSpike> {
        self.delay_buffer.drain_range(from_us, to_us)
    }

    /// Number of spikes currently in flight through this tract.
    pub fn in_flight(&self) -> usize {
        self.delay_buffer.in_flight()
    }

    /// Recompute conduction delay (call after myelination changes).
    pub fn recompute_delay(&mut self) {
        self.delay_us = compute_delay(self.corridor_length, self.tract_type, self.myelination);
    }
}

/// Apply commissural polarity bias.
///
/// `excitatory_ratio` controls the probability of sign preservation:
/// - 255 = always excitatory (preserve sign)
/// - 0 = always inhibitory (flip sign)
/// - 128 = balanced (no change)
///
/// Uses deterministic modulation based on current magnitude.
fn apply_polarity_bias(current: i32, excitatory_ratio: u8) -> i32 {
    if excitatory_ratio == 128 {
        return current; // balanced, no change
    }

    // Use magnitude bits as cheap deterministic selector
    let selector = (current.unsigned_abs() & 0xFF) as u8;
    if selector < excitatory_ratio {
        current // excitatory: preserve sign
    } else {
        -current // inhibitory: flip sign
    }
}

/// Exponential moving average for u8.
/// `alpha` is the denominator (higher = slower adaptation).
#[inline]
fn ema_u8(current: u8, sample: u8, alpha: u16) -> u8 {
    let result = (current as u16 * (alpha - 1) + sample as u16) / alpha;
    result.min(255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::MappingKind;
    use crate::modulation::ChemicalLevels;

    fn test_config(tract_type: TractType) -> TractConfig {
        TractConfig {
            name: "test_tract".to_string(),
            tract_type,
            source_region: 0,
            target_region: 1,
            corridor_length: 10,
            myelination: 128,
            diameter: 64,
            excitatory_ratio: 128,
            default_zone: DendriticZone::Feedforward,
            source_n: 100,
            target_n: 100,
            mapping_kind: MappingKind::Topographic,
            density: 1.0,
            seed: 42,
        }
    }

    #[test]
    fn tract_creation() {
        let tract = WhiteMatterTract::new(test_config(TractType::Association));
        assert_eq!(tract.health, 255);
        assert_eq!(tract.traffic_density, 0);
        assert!(tract.delay_us > 0);
        assert_eq!(tract.mapping.len(), 100);
    }

    #[test]
    fn transmit_and_deliver() {
        let mut tract = WhiteMatterTract::new(test_config(TractType::Association));
        let chemicals = ChemicalLevels::baseline();

        // Fire neurons 0, 1, 2
        let fired = vec![0u32, 1, 2];
        let enqueued = tract.transmit(&fired, 500, &chemicals, 0);
        assert_eq!(enqueued, 3);
        assert_eq!(tract.lifetime_spikes, 3);

        // Nothing at time 0 (spikes are delayed)
        let delivered = tract.deliver(0);
        assert!(delivered.is_empty());

        // Deliver at arrival time
        let delivered = tract.deliver(tract.delay_us as u64);
        assert_eq!(delivered.len(), 3);
    }

    #[test]
    fn severed_tract_transmits_nothing() {
        let mut tract = WhiteMatterTract::new(test_config(TractType::Association));
        tract.health = 0;
        let chemicals = ChemicalLevels::baseline();

        let enqueued = tract.transmit(&[0, 1, 2], 500, &chemicals, 0);
        assert_eq!(enqueued, 0);
    }

    #[test]
    fn bandwidth_saturation() {
        let mut config = test_config(TractType::Association);
        config.diameter = 2; // only 2 spikes per cycle
        let mut tract = WhiteMatterTract::new(config);
        let chemicals = ChemicalLevels::baseline();

        let enqueued = tract.transmit(&[0, 1, 2, 3, 4], 500, &chemicals, 0);
        assert_eq!(enqueued, 2); // capped at diameter
    }

    #[test]
    fn myelination_affects_delay() {
        let unmyel = compute_delay(10, TractType::Association, 0);
        let myel = compute_delay(10, TractType::Association, 255);
        assert!(
            myel < unmyel,
            "myelinated should be faster: {} vs {}",
            myel,
            unmyel
        );
    }

    #[test]
    fn health_attenuates() {
        let mut tract = WhiteMatterTract::new(test_config(TractType::Association));
        let chemicals = ChemicalLevels::baseline();

        // Full health
        tract.health = 255;
        tract.transmit(&[0], 1000, &chemicals, 0);
        let full = tract.deliver(tract.delay_us as u64);

        // Half health
        tract.health = 128;
        tract.transmit(&[0], 1000, &chemicals, tract.delay_us as u64 * 2);
        let half = tract.deliver(tract.delay_us as u64 * 3);

        assert!(
            full[0].current.abs() > half[0].current.abs(),
            "half health should attenuate: {} vs {}",
            full[0].current,
            half[0].current
        );
    }
}
