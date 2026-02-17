//! Use-dependent adaptation of white matter tract properties.
//!
//! Called during sleep consolidation (not every tick). Adapts:
//! - Myelination: high traffic → faster conduction
//! - Diameter: sustained traffic → more bandwidth
//! - Health: moderate use maintains, disuse degrades

use crate::tract::WhiteMatterTract;

/// Configuration for tract adaptation rates.
pub struct AdaptConfig {
    /// Traffic density above which myelination increases (0-255).
    pub myelin_growth_threshold: u8,
    /// Traffic density below which myelination decays (0-255).
    pub myelin_decay_threshold: u8,
    /// Myelination growth rate per consolidation (added when above threshold).
    pub myelin_growth_rate: u8,
    /// Myelination decay rate per consolidation (subtracted when below threshold).
    pub myelin_decay_rate: u8,
    /// Traffic density above which diameter grows (0-255).
    pub diameter_growth_threshold: u8,
    /// Minimum period_spikes to avoid health decay.
    pub health_use_threshold: u64,
    /// Health decay per consolidation when unused.
    pub health_decay_rate: u8,
    /// Health recovery per consolidation when used.
    pub health_recovery_rate: u8,
}

impl Default for AdaptConfig {
    fn default() -> Self {
        Self {
            myelin_growth_threshold: 80,
            myelin_decay_threshold: 10,
            myelin_growth_rate: 2,
            myelin_decay_rate: 1,
            diameter_growth_threshold: 200,
            health_use_threshold: 100,
            health_decay_rate: 1,
            health_recovery_rate: 2,
        }
    }
}

/// Adapt a tract's physical properties based on recent traffic.
///
/// Called during sleep consolidation. Resets `period_spikes` after adaptation.
pub fn adapt_tract(tract: &mut WhiteMatterTract, config: &AdaptConfig) {
    // --- Myelination ---
    if tract.traffic_density > config.myelin_growth_threshold {
        tract.myelination = tract.myelination.saturating_add(config.myelin_growth_rate);
    } else if tract.traffic_density < config.myelin_decay_threshold {
        tract.myelination = tract.myelination.saturating_sub(config.myelin_decay_rate);
    }

    // --- Diameter ---
    if tract.traffic_density > config.diameter_growth_threshold {
        tract.diameter = tract.diameter.saturating_add(1);
    }
    // Diameter doesn't shrink — only grows with sustained high traffic.
    // Unused tracts lose health instead.

    // --- Health ---
    if tract.period_spikes >= config.health_use_threshold {
        // Used: recover health
        tract.health = tract.health.saturating_add(config.health_recovery_rate);
    } else if tract.period_spikes == 0 && tract.health > 0 {
        // Completely unused: degrade
        tract.health = tract.health.saturating_sub(config.health_decay_rate);
    }

    // Recompute delay (myelination may have changed)
    tract.recompute_delay();

    // Reset period counters
    tract.period_spikes = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tract::{TractConfig, TractType, DendriticZone, WhiteMatterTract};
    use crate::mapping::MappingKind;

    fn test_tract() -> WhiteMatterTract {
        WhiteMatterTract::new(TractConfig {
            name: "test".to_string(),
            tract_type: TractType::Association,
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
        })
    }

    #[test]
    fn high_traffic_myelinates() {
        let mut tract = test_tract();
        let config = AdaptConfig::default();
        let old_myelin = tract.myelination;

        tract.traffic_density = 120; // above threshold
        tract.period_spikes = 500;
        adapt_tract(&mut tract, &config);

        assert!(
            tract.myelination > old_myelin,
            "high traffic should myelinate: {} vs {}",
            tract.myelination, old_myelin
        );
    }

    #[test]
    fn low_traffic_demyelinates() {
        let mut tract = test_tract();
        let config = AdaptConfig::default();
        let old_myelin = tract.myelination;

        tract.traffic_density = 2; // below decay threshold
        tract.period_spikes = 5;
        adapt_tract(&mut tract, &config);

        assert!(
            tract.myelination < old_myelin,
            "low traffic should demyelinate: {} vs {}",
            tract.myelination, old_myelin
        );
    }

    #[test]
    fn unused_tract_degrades_health() {
        let mut tract = test_tract();
        let config = AdaptConfig::default();

        tract.period_spikes = 0;
        tract.traffic_density = 0;
        adapt_tract(&mut tract, &config);

        assert!(tract.health < 255, "unused tract should degrade: {}", tract.health);
    }

    #[test]
    fn used_tract_recovers_health() {
        let mut tract = test_tract();
        let config = AdaptConfig::default();
        tract.health = 200; // damaged

        tract.period_spikes = 500; // above use threshold
        tract.traffic_density = 50;
        adapt_tract(&mut tract, &config);

        assert!(tract.health > 200, "used tract should recover: {}", tract.health);
    }

    #[test]
    fn period_spikes_reset_after_adapt() {
        let mut tract = test_tract();
        let config = AdaptConfig::default();
        tract.period_spikes = 1000;
        adapt_tract(&mut tract, &config);
        assert_eq!(tract.period_spikes, 0);
    }
}
