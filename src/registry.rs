//! Tract registry — owns and indexes all white matter tracts.
//!
//! The kernel creates tracts via catalog wiring specs and registers them here.
//! Registry provides lookup by ID, region (efferent/afferent), and type.

use crate::tract::{WhiteMatterTract, TractType, RegionId};

/// Opaque tract identifier (index into registry storage).
pub type TractId = u32;

/// Registry that owns all white matter tracts and provides indexed access.
pub struct TractRegistry {
    tracts: Vec<WhiteMatterTract>,
}

impl TractRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tracts: Vec::new(),
        }
    }

    /// Create a registry with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            tracts: Vec::with_capacity(capacity),
        }
    }

    /// Register a tract and return its ID.
    pub fn register(&mut self, tract: WhiteMatterTract) -> TractId {
        let id = self.tracts.len() as TractId;
        self.tracts.push(tract);
        id
    }

    /// Get an immutable reference to a tract by ID.
    pub fn get(&self, id: TractId) -> Option<&WhiteMatterTract> {
        self.tracts.get(id as usize)
    }

    /// Get a mutable reference to a tract by ID.
    pub fn get_mut(&mut self, id: TractId) -> Option<&mut WhiteMatterTract> {
        self.tracts.get_mut(id as usize)
    }

    /// Total number of registered tracts.
    pub fn len(&self) -> usize {
        self.tracts.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tracts.is_empty()
    }

    /// All tract IDs with source matching `region` (efferent from region).
    pub fn tracts_from(&self, region: RegionId) -> Vec<TractId> {
        self.tracts
            .iter()
            .enumerate()
            .filter(|(_, t)| t.source_region == region)
            .map(|(i, _)| i as TractId)
            .collect()
    }

    /// All tract IDs with target matching `region` (afferent to region).
    pub fn tracts_to(&self, region: RegionId) -> Vec<TractId> {
        self.tracts
            .iter()
            .enumerate()
            .filter(|(_, t)| t.target_region == region)
            .map(|(i, _)| i as TractId)
            .collect()
    }

    /// All tract IDs of a given type.
    pub fn tracts_of_type(&self, tract_type: TractType) -> Vec<TractId> {
        self.tracts
            .iter()
            .enumerate()
            .filter(|(_, t)| t.tract_type == tract_type)
            .map(|(i, _)| i as TractId)
            .collect()
    }

    /// Find a tract by name.
    pub fn find_by_name(&self, name: &str) -> Option<TractId> {
        self.tracts
            .iter()
            .position(|t| t.name == name)
            .map(|i| i as TractId)
    }

    /// Iterate over all (TractId, &WhiteMatterTract) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (TractId, &WhiteMatterTract)> {
        self.tracts
            .iter()
            .enumerate()
            .map(|(i, t)| (i as TractId, t))
    }

    /// Mutable iterator over all (TractId, &mut WhiteMatterTract) pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (TractId, &mut WhiteMatterTract)> {
        self.tracts
            .iter_mut()
            .enumerate()
            .map(|(i, t)| (i as TractId, t))
    }
}

impl Default for TractRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tract::{TractConfig, DendriticZone};
    use crate::mapping::MappingKind;

    fn make_tract(name: &str, src: RegionId, tgt: RegionId, tt: TractType) -> WhiteMatterTract {
        WhiteMatterTract::new(TractConfig {
            name: name.to_string(),
            tract_type: tt,
            source_region: src,
            target_region: tgt,
            corridor_length: 10,
            myelination: 128,
            diameter: 64,
            excitatory_ratio: 128,
            default_zone: DendriticZone::Feedforward,
            source_n: 50,
            target_n: 50,
            mapping_kind: MappingKind::Topographic,
            density: 1.0,
            seed: 42,
        })
    }

    #[test]
    fn register_and_retrieve() {
        let mut reg = TractRegistry::new();
        let id = reg.register(make_tract("test", 0, 1, TractType::Association));
        assert_eq!(id, 0);
        assert_eq!(reg.len(), 1);

        let tract = reg.get(id).unwrap();
        assert_eq!(tract.name, "test");
    }

    #[test]
    fn efferent_afferent_lookup() {
        let mut reg = TractRegistry::new();
        reg.register(make_tract("a_to_b", 0, 1, TractType::Association));
        reg.register(make_tract("a_to_c", 0, 2, TractType::Projection));
        reg.register(make_tract("c_to_a", 2, 0, TractType::Projection));

        let from_a = reg.tracts_from(0);
        assert_eq!(from_a.len(), 2);

        let to_a = reg.tracts_to(0);
        assert_eq!(to_a.len(), 1);
    }

    #[test]
    fn type_filter() {
        let mut reg = TractRegistry::new();
        reg.register(make_tract("assoc", 0, 1, TractType::Association));
        reg.register(make_tract("proj1", 0, 2, TractType::Projection));
        reg.register(make_tract("proj2", 1, 2, TractType::Projection));

        let projections = reg.tracts_of_type(TractType::Projection);
        assert_eq!(projections.len(), 2);

        let associations = reg.tracts_of_type(TractType::Association);
        assert_eq!(associations.len(), 1);
    }

    #[test]
    fn find_by_name() {
        let mut reg = TractRegistry::new();
        reg.register(make_tract("arcuate", 0, 1, TractType::Association));
        reg.register(make_tract("corticostriatal", 1, 2, TractType::Projection));

        assert_eq!(reg.find_by_name("arcuate"), Some(0));
        assert_eq!(reg.find_by_name("corticostriatal"), Some(1));
        assert_eq!(reg.find_by_name("nonexistent"), None);
    }
}
