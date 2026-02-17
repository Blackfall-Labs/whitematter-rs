//! Chemical modulation of white matter tract transmission.
//!
//! Neuromodulators (DA, ACh, NE, 5HT) modulate signal current as it travels
//! through a tract. The kernel passes current chemical levels at transmission
//! time — tracts do not store chemical state.

use crate::tract::TractType;

/// Neuromodulator levels passed in from the kernel's chemical substrate.
///
/// All levels are u8 with 128 = baseline (no modulation effect).
/// Above baseline = elevated, below = depleted.
#[derive(Clone, Copy, Debug)]
pub struct ChemicalLevels {
    /// Dopamine — reward-modulated gain. High DA boosts corticostriatal signals.
    pub da: u8,
    /// Acetylcholine — signal-to-noise ratio. High ACh amplifies strong signals,
    /// suppresses weak ones (gating).
    pub ach: u8,
    /// Norepinephrine — arousal gain. High NE boosts all transmission.
    pub ne: u8,
    /// Serotonin — inhibitory modulation. High 5HT dampens transmission.
    pub serotonin: u8,
}

impl ChemicalLevels {
    /// Baseline levels — all at 128, no modulation effect.
    pub fn baseline() -> Self {
        Self {
            da: 128,
            ach: 128,
            ne: 128,
            serotonin: 128,
        }
    }
}

/// Modulate a base current by chemical levels, informed by tract type.
///
/// Returns the modulated current (i32). Chemical effects are multiplicative:
///
/// - **DA**: Gain on projection tracts (corticostriatal reward signaling).
///   Association/commissural get weaker DA effect.
/// - **ACh**: Signal-to-noise. Strong signals (|current| > SNR threshold)
///   get amplified; weak signals get suppressed.
/// - **NE**: Global arousal multiplier across all tract types.
/// - **5HT**: Global dampening across all tract types.
///
/// Effects are applied sequentially. Order: DA → ACh → NE → 5HT.
pub fn modulate_current(base: i32, tract_type: TractType, chemicals: &ChemicalLevels) -> i32 {
    if base == 0 {
        return 0;
    }

    let mut current = base as i64;

    // --- DA: reward-modulated gain ---
    // Projection tracts get full DA effect (corticostriatal pathway).
    // Association/commissural get half DA effect.
    let da_offset = chemicals.da as i64 - 128; // -128..127
    let da_scale = match tract_type {
        TractType::Projection => 128 + da_offset / 2, // range: 64..191
        _ => 128 + da_offset / 4,                      // range: 96..159
    };
    current = (current * da_scale) / 128;

    // --- ACh: signal-to-noise gating ---
    let ach_offset = chemicals.ach as i64 - 128;
    let abs_current = current.unsigned_abs();
    // SNR threshold: signals below this are "weak"
    const SNR_THRESHOLD: u64 = 50;
    if abs_current > SNR_THRESHOLD {
        // Strong signal: amplify
        let ach_scale = 128 + ach_offset / 2;
        current = (current * ach_scale) / 128;
    } else {
        // Weak signal: suppress
        let ach_scale = 128 - ach_offset / 3;
        current = (current * ach_scale) / 128;
    }

    // --- NE: arousal gain ---
    let ne_offset = chemicals.ne as i64 - 128;
    let ne_scale = 128 + ne_offset / 3; // range: 85..170
    current = (current * ne_scale) / 128;

    // --- 5HT: inhibitory dampening ---
    let sht_offset = chemicals.serotonin as i64 - 128;
    // Only dampen when elevated (> 128). Low 5HT has no boost effect.
    if sht_offset > 0 {
        let sht_scale = 128 - sht_offset / 2; // range: 64..128
        current = (current * sht_scale) / 128;
    }

    current as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_no_modulation() {
        let chemicals = ChemicalLevels::baseline();
        // Baseline chemicals should not change current
        let result = modulate_current(500, TractType::Association, &chemicals);
        assert_eq!(result, 500);
    }

    #[test]
    fn high_da_boosts_projection() {
        let chemicals = ChemicalLevels {
            da: 200,
            ..ChemicalLevels::baseline()
        };
        let result = modulate_current(500, TractType::Projection, &chemicals);
        assert!(result > 500, "high DA should boost projection: {} vs 500", result);
    }

    #[test]
    fn low_da_suppresses() {
        let chemicals = ChemicalLevels {
            da: 50,
            ..ChemicalLevels::baseline()
        };
        let result = modulate_current(500, TractType::Association, &chemicals);
        assert!(result < 500, "low DA should suppress: {} vs 500", result);
    }

    #[test]
    fn high_ach_amplifies_strong_signal() {
        let chemicals = ChemicalLevels {
            ach: 200,
            ..ChemicalLevels::baseline()
        };
        let result = modulate_current(500, TractType::Association, &chemicals);
        assert!(result > 500, "high ACh should amplify strong signal: {} vs 500", result);
    }

    #[test]
    fn high_ach_suppresses_weak_signal() {
        let chemicals = ChemicalLevels {
            ach: 200,
            ..ChemicalLevels::baseline()
        };
        let result = modulate_current(10, TractType::Association, &chemicals);
        assert!(result < 10, "high ACh should suppress weak signal: {} vs 10", result);
    }

    #[test]
    fn high_ne_boosts_all() {
        let chemicals = ChemicalLevels {
            ne: 200,
            ..ChemicalLevels::baseline()
        };
        let result = modulate_current(500, TractType::Commissural, &chemicals);
        assert!(result > 500, "high NE should boost: {} vs 500", result);
    }

    #[test]
    fn high_serotonin_dampens() {
        let chemicals = ChemicalLevels {
            serotonin: 200,
            ..ChemicalLevels::baseline()
        };
        let result = modulate_current(500, TractType::Association, &chemicals);
        assert!(result < 500, "high 5HT should dampen: {} vs 500", result);
    }

    #[test]
    fn low_serotonin_no_boost() {
        let chemicals = ChemicalLevels {
            serotonin: 50,
            ..ChemicalLevels::baseline()
        };
        let result = modulate_current(500, TractType::Association, &chemicals);
        // Low 5HT should NOT boost — only elevated 5HT dampens
        assert_eq!(result, 500, "low 5HT should not boost: {}", result);
    }

    #[test]
    fn zero_current_stays_zero() {
        let chemicals = ChemicalLevels {
            da: 255,
            ach: 255,
            ne: 255,
            serotonin: 0,
        };
        let result = modulate_current(0, TractType::Projection, &chemicals);
        assert_eq!(result, 0);
    }
}
