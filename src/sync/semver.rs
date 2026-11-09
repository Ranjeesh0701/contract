use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
}

impl SemVer {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        let s = trimmed
            .strip_prefix('v')
            .or_else(|| trimmed.strip_prefix('V'))
            .unwrap_or(trimmed);

        if s.is_empty() {
            return None;
        }

        let (version_part, prerelease) = match s.split_once('-') {
            Some((v, pre)) => (v, Some(pre.to_string())),
            None => (s, None),
        };

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return None;
        }

        let major = parts[0].parse::<u64>().ok()?;
        let minor = if parts.len() > 1 {
            parts[1].parse::<u64>().ok()?
        } else {
            0
        };
        let patch = if parts.len() > 2 {
            parts[2].parse::<u64>().ok()?
        } else {
            0
        };

        Some(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.prerelease {
            write!(f, "-{}", pre)?;
        }
        Ok(())
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparatorOp {
    Exact,
    Greater,
    GreaterEq,
    Less,
    LessEq,
    Caret,
    Tilde,
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparator {
    pub op: ComparatorOp,
    pub version: SemVer,
}

impl Comparator {
    pub fn matches(&self, ver: &SemVer) -> bool {
        if ver.prerelease.is_some() && self.version.prerelease.is_none() {
            return false;
        }

        match self.op {
            ComparatorOp::Wildcard => true,
            ComparatorOp::Exact => ver == &self.version,
            ComparatorOp::Greater => ver > &self.version,
            ComparatorOp::GreaterEq => ver >= &self.version,
            ComparatorOp::Less => ver < &self.version,
            ComparatorOp::LessEq => ver <= &self.version,
            ComparatorOp::Caret => {
                if ver < &self.version {
                    return false;
                }
                if self.version.major > 0 {
                    ver.major == self.version.major
                } else if self.version.minor > 0 {
                    ver.major == 0 && ver.minor == self.version.minor
                } else {
                    ver == &self.version
                }
            }
            ComparatorOp::Tilde => {
                if ver < &self.version {
                    return false;
                }
                ver.major == self.version.major && ver.minor == self.version.minor
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionReq {
    pub comparators: Vec<Comparator>,
}

impl VersionReq {
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        if trimmed == "*" || trimmed.is_empty() {
            return Ok(Self {
                comparators: vec![Comparator {
                    op: ComparatorOp::Wildcard,
                    version: SemVer::new(0, 0, 0),
                }],
            });
        }

        let mut comparators = Vec::new();
        for clause in trimmed.split(',') {
            let clause = clause.trim();
            if clause.is_empty() {
                continue;
            }

            let (op, ver_str) = if let Some(rest) = clause.strip_prefix(">=") {
                (ComparatorOp::GreaterEq, rest.trim())
            } else if let Some(rest) = clause.strip_prefix("<=") {
                (ComparatorOp::LessEq, rest.trim())
            } else if let Some(rest) = clause.strip_prefix('>') {
                (ComparatorOp::Greater, rest.trim())
            } else if let Some(rest) = clause.strip_prefix('<') {
                (ComparatorOp::Less, rest.trim())
            } else if let Some(rest) = clause.strip_prefix('^') {
                (ComparatorOp::Caret, rest.trim())
            } else if let Some(rest) = clause.strip_prefix('~') {
                (ComparatorOp::Tilde, rest.trim())
            } else if let Some(rest) = clause.strip_prefix('=') {
                (ComparatorOp::Exact, rest.trim())
            } else {
                (ComparatorOp::Caret, clause)
            };

            let version = SemVer::parse(ver_str)
                .ok_or_else(|| format!("Invalid semantic version '{}' in constraint", ver_str))?;

            comparators.push(Comparator { op, version });
        }

        if comparators.is_empty() {
            return Err("Empty version constraint".to_string());
        }

        Ok(Self { comparators })
    }

    pub fn matches(&self, ver: &SemVer) -> bool {
        self.comparators.iter().all(|c| c.matches(ver))
    }
}

pub fn resolve_best_version<'a, I>(req: &VersionReq, tags: I) -> Option<(&'a str, SemVer)>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut matching: Vec<(&'a str, SemVer)> = tags
        .into_iter()
        .filter_map(|raw_tag| {
            let tag_name = raw_tag.strip_suffix("^{}").unwrap_or(raw_tag);
            SemVer::parse(tag_name).map(|v| (raw_tag, v))
        })
        .filter(|(_, ver)| req.matches(ver))
        .collect();

    matching.sort_by(|a, b| a.1.cmp(&b.1));
    matching.pop()
}
