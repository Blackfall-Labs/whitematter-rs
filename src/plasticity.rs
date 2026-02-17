//! Three-factor Hebbian plasticity for white matter tract mappings.
//!
//! Adjusts mapping weights based on pre-synaptic and post-synaptic correlation,
//! gated by dopamine. Runs periodically during consolidation, not every spike.
//!
//! Three factors:
//! 1. **Pre-synaptic**: source neuron fired and projected through this tract
//! 2. **Post-synaptic**: target neuron actually spiked
//! 3. **Neuromodulator**: dopamine above baseline gates plasticity

use crate::tract::WhiteMatterTract;

/// Update plasticity on a tract's mapping based on spike correlation.
///
/// `source_fired` — which source neurons spiked during this consolidation window.
/// `target_fired` — which target neurons spiked during this consolidation window.
/// `da_level` — current dopamine level (128 = baseline, >128 enables plasticity).
///
/// When source AND target neurons are correlated (both fired), the mapping
/// connection strengthens (more reliable transmission). When uncorrelated,
/// connections weaken (use-dependent pruning).
///
/// Plasticity only occurs when DA > baseline (reward gating).
pub fn update_plasticity(
    tract: &mut WhiteMatterTract,
    source_fired: &[bool],
    target_fired: &[bool],
    da_level: u8,
) {
    // DA must be above baseline to gate plasticity
    let da_above = da_level as i32 - 128;
    if da_above <= 0 {
        return; // no reward signal, no plasticity
    }

    // Count correlated vs total projected connections
    let mut correlated = 0u32;
    let mut total_projected = 0u32;

    for (src_idx, targets) in tract.mapping.iter() {
        let src_active = source_fired
            .get(*src_idx as usize)
            .copied()
            .unwrap_or(false);

        if !src_active {
            continue;
        }

        for &tgt_idx in targets {
            total_projected += 1;

            let tgt_active = target_fired
                .get(tgt_idx as usize)
                .copied()
                .unwrap_or(false);

            if tgt_active {
                correlated += 1;
            }
        }
    }

    if total_projected == 0 {
        return;
    }

    let correlation_ratio = (correlated * 100) / total_projected;

    // High correlation (>30%): tract is useful, strengthen via myelination bump
    // Low correlation: tract is noisy, weaken via slight health penalty
    if correlation_ratio > 30 {
        // Strengthen: small myelination bump (faster = more reliable)
        let strength = ((da_above / 32) as u8).max(1); // 1-3
        tract.myelination = tract.myelination.saturating_add(strength);
    } else if correlation_ratio < 10 && total_projected > 5 {
        // Weaken: slight health penalty (enough projected to be statistically meaningful)
        tract.health = tract.health.saturating_sub(1);
    }
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
            source_n: 10,
            target_n: 10,
            mapping_kind: MappingKind::Topographic,
            density: 1.0,
            seed: 42,
        })
    }

    #[test]
    fn correlated_activity_strengthens() {
        let mut tract = test_tract();
        let old_myelin = tract.myelination;

        // All source neurons fire, all target neurons fire → high correlation
        let source_fired = vec![true; 10];
        let target_fired = vec![true; 10];

        update_plasticity(&mut tract, &source_fired, &target_fired, 200);

        assert!(
            tract.myelination > old_myelin,
            "correlated activity should strengthen: {} vs {}",
            tract.myelination, old_myelin
        );
    }

    #[test]
    fn uncorrelated_activity_weakens() {
        let mut tract = test_tract();

        // Source fires but targets do NOT fire → uncorrelated
        let source_fired = vec![true; 10];
        let target_fired = vec![false; 10];

        update_plasticity(&mut tract, &source_fired, &target_fired, 200);

        assert!(
            tract.health < 255,
            "uncorrelated activity should weaken: {}",
            tract.health
        );
    }

    #[test]
    fn no_plasticity_without_da() {
        let mut tract = test_tract();
        let old_myelin = tract.myelination;
        let old_health = tract.health;

        let source_fired = vec![true; 10];
        let target_fired = vec![true; 10];

        // DA at baseline (128) — no plasticity
        update_plasticity(&mut tract, &source_fired, &target_fired, 128);

        assert_eq!(tract.myelination, old_myelin);
        assert_eq!(tract.health, old_health);
    }

    #[test]
    fn no_activity_no_change() {
        let mut tract = test_tract();
        let old_myelin = tract.myelination;
        let old_health = tract.health;

        let source_fired = vec![false; 10];
        let target_fired = vec![true; 10];

        update_plasticity(&mut tract, &source_fired, &target_fired, 200);

        assert_eq!(tract.myelination, old_myelin);
        assert_eq!(tract.health, old_health);
    }
}
