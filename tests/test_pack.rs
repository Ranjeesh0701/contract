use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use contract::sync::{BundleError, ContractBundle};

fn create_temp_dir(suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("contract_pack_test_{}_{}", suffix, nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_pack_single_file_contract() {
    let dir = create_temp_dir("single");
    let contract_path = dir.join("api.contract");
    let bundle_path = dir.join("api.bundle.json");
    let unpack_dir = dir.join("extracted");

    fs::write(
        &contract_path,
        r#"
contract SimpleApi {
    version: "1.0.0"

    model Item {
        id: string
        name: string
    }

    service ItemService {
        GetItem(string): Item;
    }
}
"#,
    )
    .unwrap();

    let bundle = ContractBundle::pack(&contract_path, false).expect("Pack failed");
    assert_eq!(bundle.manifest.name, "SimpleApi");
    assert_eq!(bundle.manifest.version, "1.0.0");
    assert_eq!(bundle.manifest.file_count, 1);
    assert_eq!(bundle.manifest.total_types, 1);
    assert_eq!(bundle.manifest.total_services, 1);
    assert!(!bundle.checksum.is_empty());

    bundle.write_to_file(&bundle_path).expect("Failed to write bundle file");

    let loaded = ContractBundle::read_from_file(&bundle_path).expect("Failed to read bundle file");
    assert_eq!(loaded.manifest.name, "SimpleApi");
    assert_eq!(loaded.checksum, bundle.checksum);

    loaded.unpack(&unpack_dir).expect("Failed to unpack bundle");
    let unpacked_contract = unpack_dir.join("api.contract");
    assert!(unpacked_contract.exists());
    let unpacked_content = fs::read_to_string(&unpacked_contract).unwrap();
    assert!(unpacked_content.contains("SimpleApi"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_pack_multi_file_modular_contract() {
    let dir = create_temp_dir("multi");
    let domain_dir = dir.join("domain");
    let common_dir = dir.join("common");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::create_dir_all(&common_dir).unwrap();

    let id_file = common_dir.join("id.contract");
    let user_file = domain_dir.join("user.contract");
    let root_file = dir.join("app.contract");

    fs::write(
        &id_file,
        r#"
type UserId: string;
"#,
    )
    .unwrap();

    fs::write(
        &user_file,
        r#"
import "../common/id.contract";

model User {
    id: UserId
    name: string
}
"#,
    )
    .unwrap();

    fs::write(
        &root_file,
        r#"
import "domain/user.contract";

contract AppService {
    version: "2.1.0"

    service UserService {
        GetUser(string): User;
    }
}
"#,
    )
    .unwrap();

    let bundle = ContractBundle::pack(&root_file, false).expect("Multi-file pack failed");
    assert_eq!(bundle.manifest.name, "AppService");
    assert_eq!(bundle.manifest.version, "2.1.0");
    assert_eq!(bundle.manifest.file_count, 3);
    assert_eq!(bundle.manifest.total_services, 1);

    // Types from common and domain must be fully resolved in root_contract
    let type_names: Vec<&str> = bundle.root_contract.types.iter().map(|t| t.name()).collect();
    assert!(type_names.contains(&"UserId"));
    assert!(type_names.contains(&"User"));

    // Extract to an isolated location and check directory structure
    let out_dir = dir.join("extracted_app");
    bundle.unpack(&out_dir).expect("Unpack multi-file failed");

    assert!(out_dir.join("app.contract").exists());
    assert!(out_dir.join("domain").join("user.contract").exists());
    assert!(out_dir.join("common").join("id.contract").exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_bundle_tamper_detection() {
    let dir = create_temp_dir("tamper");
    let contract_path = dir.join("tamper.contract");

    fs::write(
        &contract_path,
        r#"
contract SecureApi {
    version: "1.0.0"
}
"#,
    )
    .unwrap();

    let mut bundle = ContractBundle::pack(&contract_path, false).unwrap();
    assert!(bundle.verify().is_ok());

    // Tamper with file content inside bundle
    let key = bundle.files.keys().next().unwrap().clone();
    bundle.files.insert(key, "malicious tampered content".to_string());

    let err = bundle.verify().expect_err("Should detect tampering");
    match err {
        BundleError::ChecksumMismatch { expected, actual } => {
            assert_ne!(expected, actual);
        }
        other => panic!("Expected ChecksumMismatch, got {:?}", other),
    }

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_flatten_bundle_mode() {
    let dir = create_temp_dir("flatten");
    let helper_file = dir.join("helper.contract");
    let main_file = dir.join("main.contract");

    fs::write(
        &helper_file,
        r#"
model SharedInfo {
    info: string
}
"#,
    )
    .unwrap();

    fs::write(
        &main_file,
        r#"
import "helper.contract";

contract FlattenService {
    version: "1.0.0"

    model MainModel {
        shared: SharedInfo
    }
}
"#,
    )
    .unwrap();

    // Pack with flatten = true
    let bundle = ContractBundle::pack(&main_file, true).unwrap();
    assert!(bundle.root_contract.imports.is_empty());
    assert_eq!(bundle.root_contract.types.len(), 2);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_bundle_json_compatibility_with_diff() {
    let dir = create_temp_dir("compat");
    let file_v1 = dir.join("v1.contract");
    let file_v2 = dir.join("v2.contract");
    let bundle_v1 = dir.join("v1.bundle.json");
    let bundle_v2 = dir.join("v2.bundle.json");

    fs::write(
        &file_v1,
        r#"
contract Store {
    version: "1.0.0"

    model Item {
        id: string
        price: int64
    }
}
"#,
    )
    .unwrap();

    fs::write(
        &file_v2,
        r#"
contract Store {
    version: "1.1.0"

    model Item {
        id: string
        price: int64
        notes: string?
    }
}
"#,
    )
    .unwrap();

    ContractBundle::pack(&file_v1, false)
        .unwrap()
        .write_to_file(&bundle_v1)
        .unwrap();
    ContractBundle::pack(&file_v2, false)
        .unwrap()
        .write_to_file(&bundle_v2)
        .unwrap();

    let c1 = ContractBundle::read_from_file(&bundle_v1)
        .unwrap()
        .root_contract;
    let c2 = ContractBundle::read_from_file(&bundle_v2)
        .unwrap()
        .root_contract;

    let diff_report = contract::diff::diff(&c1, &c2);
    assert!(diff_report.is_compatible());
    assert_eq!(diff_report.changes.len(), 1);

    let _ = fs::remove_dir_all(dir);
}
