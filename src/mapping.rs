//! Source → target neuron mapping for white matter tracts.
//!
//! Ported and extended from sentinel's projection.rs MappingLUT.

/// How source neurons map to target neurons in a tract.
#[derive(Clone, Debug)]
pub enum MappingKind {
    /// Spatial coords preserved: source (x,y,z) maps to target (x,y,z) scaled.
    Topographic,
    /// Many-to-few: multiple source neurons converge onto fewer targets.
    Convergent,
    /// Few-to-many: each source neuron projects to multiple targets.
    Divergent,
    /// Random mapping with a fixed seed for reproducibility.
    Random { seed: u64 },
}

/// Precomputed lookup table mapping source neuron indices to target neuron indices.
///
/// Sparse: only neurons selected for projection have entries.
/// Built once during tract wiring.
pub struct MappingLUT {
    /// (source_index, Vec<target_index>) pairs, sorted by source_index.
    entries: Vec<(u32, Vec<u32>)>,
}

/// Deterministic LCG for reproducible mapping.
#[inline]
fn lcg_next(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 33) as u32
}

impl MappingLUT {
    /// Build a mapping LUT from source/target neuron counts and mapping kind.
    ///
    /// `density` controls what fraction of source neurons participate (0.0–1.0).
    /// Uses deterministic LCG seeded from `seed` for reproducibility.
    pub fn build(
        source_n: u32,
        target_n: u32,
        kind: &MappingKind,
        density: f32,
        seed: u64,
    ) -> Self {
        if source_n == 0 || target_n == 0 {
            return Self { entries: Vec::new() };
        }

        let mut rng = seed ^ 0xABCD_EF01_2345_6789;
        // LCG output range is 0..(2^31 - 1) due to >>33 shift.
        let density_threshold = (density.clamp(0.0, 1.0) * (1u64 << 31) as f32) as u32;
        let mut entries = Vec::new();

        for src_idx in 0..source_n {
            // Density gate
            if lcg_next(&mut rng) >= density_threshold {
                continue;
            }

            let targets = match kind {
                MappingKind::Topographic => {
                    // Proportional mapping: source index maps to scaled target index
                    let tgt = ((src_idx as u64 * target_n as u64) / source_n as u64) as u32;
                    vec![tgt.min(target_n - 1)]
                }
                MappingKind::Convergent => {
                    let tgt = lcg_next(&mut rng) % target_n;
                    vec![tgt]
                }
                MappingKind::Divergent => {
                    let fan_out = 2 + (lcg_next(&mut rng) % 3) as usize; // 2-4
                    (0..fan_out)
                        .map(|_| lcg_next(&mut rng) % target_n)
                        .collect()
                }
                MappingKind::Random { seed: rseed } => {
                    let mut local_rng = *rseed ^ (src_idx as u64);
                    let tgt = lcg_next(&mut local_rng) % target_n;
                    vec![tgt]
                }
            };

            if !targets.is_empty() {
                entries.push((src_idx, targets));
            }
        }

        Self { entries }
    }

    /// Look up target neurons for a given source neuron index.
    /// Returns empty slice if this source doesn't project.
    pub fn targets(&self, source_idx: u32) -> &[u32] {
        match self.entries.binary_search_by_key(&source_idx, |e| e.0) {
            Ok(pos) => &self.entries[pos].1,
            Err(_) => &[],
        }
    }

    /// Number of source neurons that have projection entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the LUT has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total number of individual connections (source→target pairs).
    pub fn total_connections(&self) -> usize {
        self.entries.iter().map(|(_, targets)| targets.len()).sum()
    }

    /// Iterate over all (source, target_list) entries.
    pub fn iter(&self) -> impl Iterator<Item = &(u32, Vec<u32>)> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topographic_proportional() {
        let lut = MappingLUT::build(100, 50, &MappingKind::Topographic, 1.0, 42);
        // All 100 source neurons should project
        assert_eq!(lut.len(), 100);
        // Source 0 → target 0
        assert_eq!(lut.targets(0), &[0]);
        // Source 99 → target 49 (99*50/100 = 49)
        assert_eq!(lut.targets(99), &[49]);
    }

    #[test]
    fn density_filter() {
        let lut = MappingLUT::build(1000, 100, &MappingKind::Convergent, 0.3, 99);
        // ~30% of 1000 = ~300
        assert!(
            lut.len() > 200 && lut.len() < 400,
            "expected ~300, got {}",
            lut.len()
        );
    }

    #[test]
    fn divergent_fan_out() {
        let lut = MappingLUT::build(10, 100, &MappingKind::Divergent, 1.0, 42);
        assert_eq!(lut.len(), 10);
        // Each should have 2-4 targets
        for (_, targets) in lut.iter() {
            assert!(
                targets.len() >= 2 && targets.len() <= 4,
                "expected 2-4 targets, got {}",
                targets.len()
            );
        }
    }

    #[test]
    fn empty_on_zero() {
        let lut = MappingLUT::build(0, 100, &MappingKind::Topographic, 1.0, 0);
        assert!(lut.is_empty());
    }

    #[test]
    fn missing_source_returns_empty() {
        let lut = MappingLUT::build(10, 10, &MappingKind::Topographic, 0.5, 42);
        // Source 999 doesn't exist
        assert!(lut.targets(999).is_empty());
    }
}
