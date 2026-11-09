pub mod change;
pub mod comparator;
pub mod impact;

pub use change::{Change, ChangeSeverity, DiffDirection, DiffReport};
pub use comparator::{diff, diff_with_direction};
pub use impact::{
    ContractGraph, EndpointDirection, ImpactReport, ImpactedEndpoint,
    analyze_diff_impact, analyze_impact, analyze_target_impact,
};
