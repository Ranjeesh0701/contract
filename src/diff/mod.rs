pub mod change;
pub mod comparator;

pub use change::{Change, ChangeSeverity, DiffDirection, DiffReport};
pub use comparator::{diff, diff_with_direction};
