use crate::diff::change::{Change, ChangeSeverity, DiffDirection, DiffReport};
use crate::diff::comparator::diff_with_direction;
use crate::ir::Contract;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogOptions {
    pub title: Option<String>,
    pub include_summary: bool,
}

impl Default for ChangelogOptions {
    fn default() -> Self {
        Self {
            title: None,
            include_summary: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticChangelog {
    pub contract_name: String,
    pub old_version: String,
    pub new_version: String,
    pub is_compatible: bool,
    pub breaking_changes: Vec<Change>,
    pub deprecations: Vec<Change>,
    pub additions: Vec<Change>,
}

impl SemanticChangelog {
    pub fn generate(old_contract: &Contract, new_contract: &Contract) -> Self {
        let diff_report: DiffReport =
            diff_with_direction(old_contract, new_contract, DiffDirection::Both);

        let mut breaking_changes = Vec::new();
        let mut deprecations = Vec::new();
        let mut additions = Vec::new();

        for change in diff_report.changes {
            match change.severity {
                ChangeSeverity::Breaking => breaking_changes.push(change),
                ChangeSeverity::Warning => deprecations.push(change),
                ChangeSeverity::Safe => additions.push(change),
            }
        }

        let is_compatible = breaking_changes.is_empty();

        Self {
            contract_name: new_contract.name.clone(),
            old_version: old_contract.version.clone(),
            new_version: new_contract.version.clone(),
            is_compatible,
            breaking_changes,
            deprecations,
            additions,
        }
    }

    pub fn to_markdown(&self, options: &ChangelogOptions) -> String {
        let mut out = String::new();

        let title = match &options.title {
            Some(t) => t.clone(),
            None => format!(
                "API Changelog: {} (v{} → v{})",
                self.contract_name, self.old_version, self.new_version
            ),
        };

        out.push_str(&format!("# {}\n\n", title));

        if self.breaking_changes.is_empty()
            && self.deprecations.is_empty()
            && self.additions.is_empty()
        {
            out.push_str("✔ No API changes detected between versions.\n");
            return out;
        }

        if !self.breaking_changes.is_empty() {
            out.push_str("## 🚨 Breaking Changes\n\n");
            for change in &self.breaking_changes {
                out.push_str(&format!("- **`{}`**: {}\n", change.path, change.message));
            }
            out.push('\n');
        }

        if !self.deprecations.is_empty() {
            out.push_str("## ⚠️ Deprecations & Warnings\n\n");
            for change in &self.deprecations {
                out.push_str(&format!("- **`{}`**: {}\n", change.path, change.message));
            }
            out.push('\n');
        }

        if !self.additions.is_empty() {
            out.push_str("## 🟢 Backwards-Compatible Additions\n\n");
            for change in &self.additions {
                out.push_str(&format!("- **`{}`**: {}\n", change.path, change.message));
            }
            out.push('\n');
        }

        if options.include_summary {
            out.push_str("## 📊 Summary\n\n");
            let status = if self.is_compatible {
                "✔ **Backwards-Compatible**".to_string()
            } else {
                format!(
                    "❌ **Breaking Changes Detected** ({} breaking)",
                    self.breaking_changes.len()
                )
            };
            out.push_str(&format!("- **Status:** {}\n", status));
            out.push_str(&format!(
                "- **Total Changes:** {} ({} breaking, {} warning(s), {} safe addition(s))\n",
                self.breaking_changes.len() + self.deprecations.len() + self.additions.len(),
                self.breaking_changes.len(),
                self.deprecations.len(),
                self.additions.len()
            ));
        }

        out
    }
}

pub fn generate_changelog(
    old_contract: &Contract,
    new_contract: &Contract,
    options: Option<ChangelogOptions>,
) -> String {
    let changelog = SemanticChangelog::generate(old_contract, new_contract);
    let opts = options.unwrap_or_default();
    changelog.to_markdown(&opts)
}
