use contract::sync::{DriftStatus, SyncEngine};
use std::fs;
use std::path::PathBuf;

fn setup_temp_workspace() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("contract_sync_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_sync_and_drift_detection() {
    let temp_dir = setup_temp_workspace();

    // 1. Create upstream service schema
    let upstream_schema = r#"
    contract UserService {
        version: "1.0.0"
    }

    model User {
        id: Int64
        name: String
    }

    service UserApi {
        getUser(Int64): User
    }
    "#;
    let upstream_path = temp_dir.join("upstream.contract");
    fs::write(&upstream_path, upstream_schema).unwrap();

    // 2. Create client manifest (contract.toml)
    let manifest_content = r#"
    [project]
    name = "my-web-client"

    [dependencies]
    users = { path = "upstream.contract" }

    [[generate]]
    contract = "users"
    lang = "ts"
    out = "generated/users.ts"
    "#;
    let manifest_path = temp_dir.join("contract.toml");
    fs::write(&manifest_path, manifest_content).unwrap();

    // 3. Run sync
    let report = SyncEngine::sync(&temp_dir, false).expect("Sync should succeed");
    assert_eq!(report.synced_contracts.len(), 1);
    assert_eq!(report.generated_files.len(), 1);

    // Verify lockfile was written
    let lockfile_path = temp_dir.join("contract.lock");
    assert!(lockfile_path.exists());

    // Verify TypeScript client was generated
    let generated_ts = fs::read_to_string(temp_dir.join("generated/users.ts")).unwrap();
    assert!(generated_ts.contains("export interface User {"));
    assert!(generated_ts.contains("export class UserApiClient"));

    // 4. Test drift: currently UpToDate
    let drift_reports = SyncEngine::check_drift(&temp_dir).unwrap();
    assert_eq!(drift_reports.len(), 1);
    match &drift_reports[0].status {
        DriftStatus::UpToDate => {}
        _ => panic!("Expected UpToDate"),
    }

    // 5. Update upstream with safe change (new optional field)
    let updated_schema = r#"
    contract UserService {
        version: "1.1.0"
    }

    model User {
        id: Int64
        name: String
        avatar: String?
    }

    service UserApi {
        getUser(Int64): User
    }
    "#;
    fs::write(&upstream_path, updated_schema).unwrap();

    // 6. Test drift: should detect safe drift
    let drift_reports2 = SyncEngine::check_drift(&temp_dir).unwrap();
    match &drift_reports2[0].status {
        DriftStatus::Drifted { is_breaking, .. } => {
            assert!(!*is_breaking, "Expected safe drift");
        }
        _ => panic!("Expected Drifted status"),
    }

    // Clean up
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_manifest_output_alias_compatibility() {
    let toml_with_output = r#"
    [project]
    name = "alias-test"

    [dependencies]
    auth = { path = "auth.contract" }

    [[generate]]
    contract = "auth"
    lang = "typescript"
    output = "src/sdk/auth.ts"

    [[generate]]
    contract = "auth"
    lang = "python"
    out = "clients/auth.py"
    "#;

    let manifest: contract::sync::Manifest =
        toml::from_str(toml_with_output).expect("Should parse manifest with out/output aliases");

    assert_eq!(manifest.generate.len(), 2);
    assert_eq!(manifest.generate[0].out, "src/sdk/auth.ts");
    assert_eq!(manifest.generate[1].out, "clients/auth.py");
}

#[test]
fn test_sync_with_extension_based_proto_dependency() {
    let temp_dir = std::env::temp_dir().join(format!(
        "contract_sync_proto_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).unwrap();

    let proto_schema = r#"
    syntax = "proto3";
    package greeter;

    message HelloRequest {
        string name = 1;
    }

    message HelloReply {
        string message = 1;
    }

    service Greeter {
        rpc SayHello (HelloRequest) returns (HelloReply);
    }
    "#;
    let proto_path = temp_dir.join("service.proto");
    fs::write(&proto_path, proto_schema).unwrap();

    let manifest_content = r#"
    [project]
    name = "greeter-client"

    [dependencies]
    greeter = { path = "service.proto" }

    [[generate]]
    contract = "greeter"
    lang = "ts"
    output = "generated/greeter.ts"
    "#;
    let manifest_path = temp_dir.join("contract.toml");
    fs::write(&manifest_path, manifest_content).unwrap();

    let report = SyncEngine::sync(&temp_dir, false).expect("Sync should succeed for proto dependency");
    assert_eq!(report.synced_contracts.len(), 1);
    assert_eq!(report.generated_files.len(), 1);

    let generated_ts = fs::read_to_string(temp_dir.join("generated/greeter.ts")).unwrap();
    assert!(generated_ts.contains("export interface HelloRequest"));
    assert!(generated_ts.contains("export interface HelloReply"));

    let _ = fs::remove_dir_all(temp_dir);
}
