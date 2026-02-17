//! Named tract catalog — biologically grounded white matter bundles.
//!
//! Provides declarative `TractWiringSpec` definitions for the three tract systems.
//! The kernel calls these catalog functions to get wiring specifications, then
//! constructs `WhiteMatterTract` instances with actual pool sizes and skull positions.

use crate::tract::{TractType, DendriticZone, RegionId};
use crate::mapping::MappingKind;

/// Declarative specification for wiring a tract.
///
/// The kernel uses these to construct `WhiteMatterTract` instances,
/// filling in runtime values (pool sizes, corridor lengths from skull).
pub struct TractWiringSpec {
    pub name: String,
    pub tract_type: TractType,
    pub source_region: RegionId,
    pub target_region: RegionId,
    pub mapping_kind: MappingKind,
    pub density: f32,
    pub initial_myelination: u8,
    pub excitatory_ratio: u8,
    pub default_zone: DendriticZone,
    pub diameter: u8,
}

/// Build a wiring spec with common defaults.
fn spec(
    name: &str,
    tract_type: TractType,
    source: RegionId,
    target: RegionId,
    mapping_kind: MappingKind,
    density: f32,
    myelination: u8,
    excitatory_ratio: u8,
    zone: DendriticZone,
    diameter: u8,
) -> TractWiringSpec {
    TractWiringSpec {
        name: name.to_string(),
        tract_type,
        source_region: source,
        target_region: target,
        mapping_kind,
        density,
        initial_myelination: myelination,
        excitatory_ratio,
        default_zone: zone,
        diameter,
    }
}

/// Association tracts — intra-hemispheric long-range bundles.
///
/// Region IDs are sentinel-assigned. The caller must provide a mapping
/// from region name → RegionId. This function takes a resolver closure.
///
/// Returns specs for 6 named association bundles:
/// - Arcuate fasciculus (language ↔ expression)
/// - Superior longitudinal fasciculus (spatial ↔ planning)
/// - Inferior longitudinal fasciculus (visual ↔ representation)
/// - Uncinate fasciculus (hippocampus ↔ reasoning)
/// - Cingulum bundle (hippocampus ↔ dynamics)
/// - Inferior fronto-occipital fasciculus (visual ↔ planning)
pub fn association_tracts(resolve: &dyn Fn(&str) -> Option<RegionId>) -> Vec<TractWiringSpec> {
    let mut tracts = Vec::new();

    let pairs: &[(&str, &str, &str, MappingKind, f32, u8, u8)] = &[
        // Arcuate fasciculus: language comprehension → speech production
        ("arcuate_fasciculus", "language", "expression",
         MappingKind::Topographic, 0.6, 100, 64),
        // Superior longitudinal: spatial → motor planning
        ("superior_longitudinal", "spatial", "planning",
         MappingKind::Topographic, 0.5, 80, 48),
        // Inferior longitudinal: visual → object/word recognition
        ("inferior_longitudinal", "visual", "representation",
         MappingKind::Convergent, 0.4, 90, 48),
        // Uncinate: hippocampus → value/reasoning (emotional memory)
        ("uncinate_fasciculus", "hippocampus", "reasoning",
         MappingKind::Convergent, 0.3, 60, 32),
        // Cingulum: hippocampus → dynamics (emotional regulation)
        ("cingulum_bundle", "hippocampus", "dynamics",
         MappingKind::Convergent, 0.3, 70, 32),
        // Inferior fronto-occipital: visual → executive planning
        ("inferior_fronto_occipital", "visual", "planning",
         MappingKind::Convergent, 0.3, 80, 32),
    ];

    for &(name, src, tgt, ref mapping, density, myelin, diameter) in pairs {
        if let (Some(s), Some(t)) = (resolve(src), resolve(tgt)) {
            tracts.push(spec(
                name,
                TractType::Association,
                s, t,
                mapping.clone(),
                density,
                myelin,
                128, // balanced (association tracts have no polarity bias)
                DendriticZone::Feedforward,
                diameter,
            ));
        }
    }

    tracts
}

/// Commissural tracts — inter-hemispheric connections.
///
/// Corpus callosum zones plus hippocampal and anterior commissures.
/// Each zone has a characteristic excitatory_ratio:
/// - Genu (~50): inhibitory-dominant (frontal interhemispheric inhibition)
/// - Anterior body (~80): inhibitory-leaning
/// - Posterior body (~140): slightly excitatory
/// - Isthmus (~160): excitatory-leaning
/// - Splenium (~180): excitatory-dominant (visual relay)
///
/// Commissural tracts require hemispheric mirroring (L ↔ R region pairs).
/// Until hemispheres are implemented, this returns an empty vec.
pub fn commissural_tracts(resolve: &dyn Fn(&str) -> Option<RegionId>) -> Vec<TractWiringSpec> {
    // Commissural tracts connect homologous L/R regions.
    // Hemispheric model is not yet architected (see MEMORY.md).
    // Return empty until bilateral regions exist.
    let _ = resolve;
    Vec::new()
}

/// Projection tracts — cortical ↔ subcortical ↔ brainstem vertical hierarchy.
///
/// Bidirectional pairs: ascending (subcortical → cortical) and
/// descending (cortical → subcortical).
pub fn projection_tracts(resolve: &dyn Fn(&str) -> Option<RegionId>) -> Vec<TractWiringSpec> {
    let mut tracts = Vec::new();

    // Thalamocortical ascending: thalamus → each cortical target
    let thalamic_targets: &[(&str, u8, u8)] = &[
        // (target_region, myelination, diameter)
        ("visual",         120, 48), // LGN → V1
        ("spatial",         80, 32), // pulvinar → parietal
        ("language",        80, 32), // MGN → auditory/language
        ("planning",        60, 24), // MD → prefrontal
    ];

    if let Some(thal) = resolve("thalamus") {
        for &(target, myelin, diameter) in thalamic_targets {
            if let Some(tgt) = resolve(target) {
                // Ascending: thalamus → cortex (feedforward)
                tracts.push(spec(
                    &format!("thalamocortical_{}", target),
                    TractType::Projection,
                    thal, tgt,
                    MappingKind::Topographic,
                    0.5,
                    myelin,
                    128, // balanced
                    DendriticZone::Feedforward,
                    diameter,
                ));

                // Descending: cortex → thalamus (feedback/gating)
                tracts.push(spec(
                    &format!("corticothalamic_{}", target),
                    TractType::Projection,
                    tgt, thal,
                    MappingKind::Convergent,
                    0.3,
                    myelin,
                    128,
                    DendriticZone::Feedback,
                    diameter,
                ));
            }
        }
    }

    // Corticostriatal: frontal → basal ganglia (action proposals)
    if let (Some(plan), Some(bg)) = (resolve("planning"), resolve("basal_ganglia")) {
        tracts.push(spec(
            "corticostriatal_planning",
            TractType::Projection,
            plan, bg,
            MappingKind::Convergent,
            0.4,
            80,
            128,
            DendriticZone::Feedforward,
            32,
        ));
    }

    // Pallidothalamic: basal ganglia → thalamus (disinhibition)
    if let (Some(bg), Some(thal)) = (resolve("basal_ganglia"), resolve("thalamus")) {
        tracts.push(spec(
            "pallidothalamic",
            TractType::Projection,
            bg, thal,
            MappingKind::Convergent,
            0.3,
            100,
            128,
            DendriticZone::Context,
            24,
        ));
    }

    // Corticopontine: frontal/parietal → cerebellum via brainstem (motor planning)
    if let (Some(plan), Some(expr)) = (resolve("planning"), resolve("expression")) {
        tracts.push(spec(
            "corticopontine",
            TractType::Projection,
            plan, expr,
            MappingKind::Divergent,
            0.3,
            60,
            128,
            DendriticZone::Feedforward,
            24,
        ));
    }

    // Cerebellothalamic: cerebellum → thalamus (correction signals)
    if let (Some(expr), Some(thal)) = (resolve("expression"), resolve("thalamus")) {
        tracts.push(spec(
            "cerebellothalamic",
            TractType::Projection,
            expr, thal,
            MappingKind::Convergent,
            0.3,
            100,
            128,
            DendriticZone::Context,
            24,
        ));
    }

    tracts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_resolve(name: &str) -> Option<RegionId> {
        match name {
            "language" => Some(0),
            "expression" => Some(1),
            "spatial" => Some(2),
            "planning" => Some(3),
            "visual" => Some(4),
            "representation" => Some(5),
            "hippocampus" => Some(6),
            "reasoning" => Some(7),
            "dynamics" => Some(8),
            "thalamus" => Some(9),
            "basal_ganglia" => Some(10),
            _ => None,
        }
    }

    #[test]
    fn association_catalog_produces_tracts() {
        let tracts = association_tracts(&mock_resolve);
        assert_eq!(tracts.len(), 6, "expected 6 association tracts, got {}", tracts.len());

        // All should be Association type
        for t in &tracts {
            assert_eq!(t.tract_type, TractType::Association);
            assert_eq!(t.excitatory_ratio, 128, "association tracts should be balanced");
        }
    }

    #[test]
    fn commissural_catalog_empty_without_hemispheres() {
        let tracts = commissural_tracts(&mock_resolve);
        assert!(tracts.is_empty(), "commissural tracts need hemispheric model");
    }

    #[test]
    fn projection_catalog_produces_tracts() {
        let tracts = projection_tracts(&mock_resolve);
        // Should have: 4 thalamocortical + 4 corticothalamic + corticostriatal +
        // pallidothalamic + corticopontine + cerebellothalamic = 12
        assert!(tracts.len() >= 10, "expected >=10 projection tracts, got {}", tracts.len());

        for t in &tracts {
            assert_eq!(t.tract_type, TractType::Projection);
        }
    }

    #[test]
    fn missing_regions_skipped_gracefully() {
        // Resolver that only knows thalamus and visual
        let sparse_resolve = |name: &str| -> Option<RegionId> {
            match name {
                "thalamus" => Some(0),
                "visual" => Some(1),
                _ => None,
            }
        };

        let tracts = projection_tracts(&sparse_resolve);
        // Only thalamocortical_visual + corticothalamic_visual should exist
        assert_eq!(tracts.len(), 2, "sparse resolve should only produce matching pairs, got {}", tracts.len());
    }
}
