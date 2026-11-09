use contract::sync::{SemVer, VersionReq, resolve_best_version};

#[test]
fn test_semver_parse_and_ordering() {
    let v1 = SemVer::parse("1.2.3").unwrap();
    let v2 = SemVer::parse("v1.2.4").unwrap();
    let v3 = SemVer::parse("v2.0.0").unwrap();
    let v_pre = SemVer::parse("1.2.3-alpha").unwrap();

    assert_eq!(v1, SemVer::new(1, 2, 3));
    assert!(v1 < v2);
    assert!(v2 < v3);
    assert!(v_pre < v1);
}

#[test]
fn test_caret_requirements() {
    let req = VersionReq::parse("^1.2.0").unwrap();

    assert!(req.matches(&SemVer::new(1, 2, 0)));
    assert!(req.matches(&SemVer::new(1, 2, 5)));
    assert!(req.matches(&SemVer::new(1, 9, 0)));
    assert!(!req.matches(&SemVer::new(1, 1, 9)));
    assert!(!req.matches(&SemVer::new(2, 0, 0)));
}

#[test]
fn test_tilde_requirements() {
    let req = VersionReq::parse("~1.2.0").unwrap();

    assert!(req.matches(&SemVer::new(1, 2, 0)));
    assert!(req.matches(&SemVer::new(1, 2, 9)));
    assert!(!req.matches(&SemVer::new(1, 3, 0)));
    assert!(!req.matches(&SemVer::new(1, 1, 9)));
}

#[test]
fn test_comparison_ranges() {
    let req = VersionReq::parse(">=1.0.0, <2.0.0").unwrap();

    assert!(req.matches(&SemVer::new(1, 0, 0)));
    assert!(req.matches(&SemVer::new(1, 5, 2)));
    assert!(!req.matches(&SemVer::new(0, 9, 9)));
    assert!(!req.matches(&SemVer::new(2, 0, 0)));
}

#[test]
fn test_resolve_best_version() {
    let tags = [
        "v1.0.0",
        "v1.1.0",
        "v1.2.0",
        "v1.2.4",
        "v2.0.0",
        "v2.1.0-beta",
        "not-a-tag",
    ];

    let req = VersionReq::parse("^1.2.0").unwrap();
    let (best_tag, best_ver) = resolve_best_version(&req, tags.iter().copied()).unwrap();
    assert_eq!(best_tag, "v1.2.4");
    assert_eq!(best_ver, SemVer::new(1, 2, 4));

    let req_tilde = VersionReq::parse("~1.1.0").unwrap();
    let (tilde_tag, _) = resolve_best_version(&req_tilde, tags.iter().copied()).unwrap();
    assert_eq!(tilde_tag, "v1.1.0");

    let req_major2 = VersionReq::parse("^2.0.0").unwrap();
    let (tag_v2, _) = resolve_best_version(&req_major2, tags.iter().copied()).unwrap();
    assert_eq!(tag_v2, "v2.0.0");
}
