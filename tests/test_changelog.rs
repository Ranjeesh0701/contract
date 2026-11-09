use contract::diff::{ChangelogOptions, SemanticChangelog, generate_changelog};
use contract::parser::parse;

#[test]
fn test_changelog_categorization() {
    let old_src = r#"
contract UserService {
    version: "1.0.0"

    model User {
        id: string
        email: string
        legacyCode: string
    }

    service UserApi {
        GetUser(string): User;
        OldDelete(string): void;
    }
}
"#;

    let new_src = r#"
contract UserService {
    version: "2.0.0"

    model User {
        id: string
        // email removed -> BREAKING
        @deprecated("Use newCode instead")
        legacyCode: string
        nickname: string? // added optional -> SAFE
    }

    service UserApi {
        GetUser(string): User;
        // OldDelete removed -> BREAKING
        CreateUser(string): User; // added operation -> SAFE
    }
}
"#;

    let old_c = parse(old_src).unwrap();
    let new_c = parse(new_src).unwrap();

    let changelog = SemanticChangelog::generate(&old_c, &new_c);
    assert!(!changelog.is_compatible);
    assert!(changelog.breaking_changes.len() >= 2);
    assert!(!changelog.deprecations.is_empty());
    assert!(!changelog.additions.is_empty());

    let md = changelog.to_markdown(&ChangelogOptions::default());

    assert!(md.contains("# API Changelog: UserService (v1.0.0 → v2.0.0)"));
    assert!(md.contains("## 🚨 Breaking Changes"));
    assert!(md.contains("## ⚠️ Deprecations & Warnings"));
    assert!(md.contains("## 🟢 Backwards-Compatible Additions"));
    assert!(md.contains("## 📊 Summary"));
    assert!(md.contains("❌ **Breaking Changes Detected**"));
    assert!(md.contains("Use newCode instead"));
}

#[test]
fn test_changelog_fully_compatible() {
    let old_src = r#"
contract Catalog {
    version: "1.0.0"

    model Item {
        id: string
    }
}
"#;

    let new_src = r#"
contract Catalog {
    version: "1.1.0"

    model Item {
        id: string
        description: string?
    }

    model Tag {
        label: string
    }
}
"#;

    let old_c = parse(old_src).unwrap();
    let new_c = parse(new_src).unwrap();

    let changelog = SemanticChangelog::generate(&old_c, &new_c);
    assert!(changelog.is_compatible);
    assert_eq!(changelog.breaking_changes.len(), 0);
    assert!(changelog.additions.len() >= 2);

    let md = changelog.to_markdown(&ChangelogOptions::default());
    assert!(!md.contains("## 🚨 Breaking Changes"));
    assert!(md.contains("## 🟢 Backwards-Compatible Additions"));
    assert!(md.contains("✔ **Backwards-Compatible**"));
}

#[test]
fn test_changelog_no_changes() {
    let src = r#"
contract StaticService {
    version: "1.0.0"

    model Entity {
        id: string
    }
}
"#;

    let c1 = parse(src).unwrap();
    let c2 = parse(src).unwrap();

    let md = generate_changelog(&c1, &c2, None);
    assert!(md.contains("✔ No API changes detected between versions."));
}

#[test]
fn test_changelog_custom_title_and_options() {
    let old_src = r#"
contract Core {
    version: "1.0.0"
    model A { x: string }
}
"#;

    let new_src = r#"
contract Core {
    version: "1.1.0"
    model A { x: string; y: int32? }
}
"#;

    let c1 = parse(old_src).unwrap();
    let c2 = parse(new_src).unwrap();

    let opts = ChangelogOptions {
        title: Some("My Custom Release Notes - v1.1.0".to_string()),
        include_summary: false,
    };

    let md = generate_changelog(&c1, &c2, Some(opts));
    assert!(md.contains("# My Custom Release Notes - v1.1.0"));
    assert!(!md.contains("## 📊 Summary"));
}
