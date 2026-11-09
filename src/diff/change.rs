use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ValueEnum)]
pub enum DiffDirection {
    #[default]
    Both,
    Producer,
    Consumer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ChangeSeverity {
    Safe,
    Warning,
    Breaking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub severity: ChangeSeverity,
    pub path: String,
    pub message: String,
}

impl Change {
    pub fn breaking(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ChangeSeverity::Breaking,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ChangeSeverity::Warning,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn safe(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ChangeSeverity::Safe,
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffReport {
    pub changes: Vec<Change>,
}

impl DiffReport {
    pub fn is_compatible(&self) -> bool {
        !self
            .changes
            .iter()
            .any(|c| c.severity == ChangeSeverity::Breaking)
    }

    pub fn has_breaking_changes(&self) -> bool {
        !self.is_compatible()
    }

    pub fn breaking_changes(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|c| c.severity == ChangeSeverity::Breaking)
            .collect()
    }

    pub fn warnings(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|c| c.severity == ChangeSeverity::Warning)
            .collect()
    }

    pub fn has_warnings(&self) -> bool {
        self.changes
            .iter()
            .any(|c| c.severity == ChangeSeverity::Warning)
    }
}
