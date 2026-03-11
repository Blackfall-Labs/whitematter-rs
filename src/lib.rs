//! # whitematter
//!
//! Central nervous system white matter tracts — the brain's internal wiring.
//!
//! Three tract systems modeled after biological white matter:
//! - **Association**: intra-hemispheric long-range bundles (e.g., arcuate fasciculus)
//! - **Commissural**: inter-hemispheric connections (e.g., corpus callosum zones)
//! - **Projection**: cortical ↔ subcortical vertical hierarchy (e.g., thalamocortical)
//!
//! Signals propagate through tracts with myelination-dependent conduction velocity,
//! chemical modulation (DA, ACh, NE, 5HT), and use-dependent adaptation.
//!
//! This crate handles brain-internal wiring. Peripheral nerves (brain ↔ body)
//! are handled by `fibertract-rs`.

mod adapt;
pub mod catalog;
mod delay;
mod mapping;
mod modulation;
mod plasticity;
mod registry;
mod tract;

pub use adapt::{adapt_tract, AdaptConfig};
pub use catalog::TractWiringSpec;
pub use delay::DelayBuffer;
pub use mapping::{MappingKind, MappingLUT};
pub use modulation::{modulate_current, ChemicalLevels};
pub use plasticity::update_plasticity;
pub use registry::{TractId, TractRegistry};
pub use tract::{DendriticZone, TractConfig, TractSpike, TractType, WhiteMatterTract};
