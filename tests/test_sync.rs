use contract::sync::{DriftStatus, SyncEngine};
use std::fs;
use std::path::{Path, PathBuf};

fn setup_temp_workspace() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("contract_sync_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_sync_and_drift_detection() {
    let temp_dir = setup_temp_workspace();

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

    let report = SyncEngine::sync(&temp_dir, false).expect("Sync should succeed");
    assert_eq!(report.synced_contracts.len(), 1);
    assert_eq!(report.generated_files.len(), 1);

    let lockfile_path = temp_dir.join("contract.lock");
    assert!(lockfile_path.exists());

    let generated_ts = fs::read_to_string(temp_dir.join("generated/users.ts")).unwrap();
    assert!(generated_ts.contains("export interface User {"));
    assert!(generated_ts.contains("export class UserApiClient"));

    let drift_reports = SyncEngine::check_drift(&temp_dir).unwrap();
    assert_eq!(drift_reports.len(), 1);
    match &drift_reports[0].status {
        DriftStatus::UpToDate => {}
        _ => panic!("Expected UpToDate"),
    }

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

    let drift_reports2 = SyncEngine::check_drift(&temp_dir).unwrap();
    match &drift_reports2[0].status {
        DriftStatus::Drifted { is_breaking, .. } => {
            assert!(!*is_breaking, "Expected safe drift");
        }
        _ => panic!("Expected Drifted status"),
    }

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_sync_with_url_file_dependency() {
    let temp_dir = std::env::temp_dir().join(format!(
        "contract_sync_url_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).unwrap();

    let remote_schema = r#"
    contract ExternalApi {
        version: "1.0.0"
    }
    model Health {
        status: String
    }
    "#;
    let schema_file = temp_dir.join("external.contract");
    fs::write(&schema_file, remote_schema).unwrap();

    let manifest_content = format!(
        r#"
    [project]
    name = "url-consumer"

    [dependencies]
    external = {{ url = "file://{}" }}

    [[generate]]
    contract = "external"
    lang = "ts"
    out = "sdk/external.ts"
    "#,
        schema_file.display()
    );
    let manifest_path = temp_dir.join("contract.toml");
    fs::write(&manifest_path, manifest_content).unwrap();

    let report = SyncEngine::sync(&temp_dir, false).expect("Sync should succeed for url dependency");
    assert_eq!(report.synced_contracts.len(), 1);
    assert_eq!(report.generated_files.len(), 1);

    let lockfile = contract::sync::Lockfile::load_from_file(temp_dir.join("contract.lock")).unwrap();
    let locked = lockfile.contracts.get("external").expect("External should be locked");
    assert!(locked.source.starts_with("url:file://"));

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_sync_with_git_dependency() {
    let base_temp = std::env::temp_dir().join(format!(
        "contract_sync_git_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let git_repo_dir = base_temp.join("remote-repo");
    let consumer_dir = base_temp.join("consumer-app");

    fs::create_dir_all(&git_repo_dir).unwrap();
    fs::create_dir_all(&consumer_dir).unwrap();

    // 1. Initialize local git repository
    let run_git = |args: &[&str], dir: &Path| {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("Git command should execute");
        assert!(status.success(), "Git command {:?} failed", args);
    };

    run_git(&["init", "-b", "main"], &git_repo_dir);
    run_git(&["config", "user.name", "Contract Test"], &git_repo_dir);
    run_git(&["config", "user.email", "test@contract.dev"], &git_repo_dir);

    let contract_dsl = r#"
    contract InventoryService {
        version: "1.0.0"
    }

    model StockItem {
        sku: String
        quantity: Int64
    }
    "#;
    fs::write(git_repo_dir.join("inventory.contract"), contract_dsl).unwrap();
    run_git(&["add", "inventory.contract"], &git_repo_dir);
    run_git(&["commit", "-m", "Initial commit"], &git_repo_dir);

    // 2. Configure consumer contract.toml pointing to local git repo
    let manifest_content = format!(
        r#"
    [project]
    name = "inventory-consumer"

    [dependencies]
    inventory = {{ git = "{}", path = "inventory.contract", branch = "main" }}

    [[generate]]
    contract = "inventory"
    lang = "ts"
    out = "src/inventory.ts"
    "#,
        git_repo_dir.display()
    );
    fs::write(consumer_dir.join("contract.toml"), manifest_content).unwrap();

    // 3. Sync from git
    let report = SyncEngine::sync(&consumer_dir, false).expect("Sync should succeed for git dependency");
    assert_eq!(report.synced_contracts.len(), 1);
    assert_eq!(report.generated_files.len(), 1);

    let generated_code = fs::read_to_string(consumer_dir.join("src/inventory.ts")).unwrap();
    assert!(generated_code.contains("export interface StockItem"));

    let lockfile = contract::sync::Lockfile::load_from_file(consumer_dir.join("contract.lock")).unwrap();
    let locked = lockfile.contracts.get("inventory").expect("Inventory should be locked");
    assert!(locked.source.starts_with("git:"));
    assert!(locked.source.contains("inventory.contract"));

    // 4. Check drift: initially UpToDate
    let drift_reports = SyncEngine::check_drift(&consumer_dir).unwrap();
    assert_eq!(drift_reports.len(), 1);
    assert!(matches!(drift_reports[0].status, contract::sync::DriftStatus::UpToDate));

    let _ = fs::remove_dir_all(base_temp);
}

#[test]
fn test_artifact_integrity_and_tamper_detection() {
    let temp_dir = std::env::temp_dir().join(format!(
        "contract_artifact_integrity_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).unwrap();

    let contract_dsl = r#"
    contract OrderService {
        version: "1.0.0"
    }

    model Order {
        id: String
        amount: Float64
    }
    "#;
    let contract_path = temp_dir.join("orders.contract");
    fs::write(&contract_path, contract_dsl).unwrap();

    let manifest_content = r#"
    [project]
    name = "orders-app"

    [dependencies]
    orders = { path = "orders.contract" }

    [[generate]]
    contract = "orders"
    lang = "ts"
    out = "src/sdk/orders.ts"
    "#;
    let manifest_path = temp_dir.join("contract.toml");
    fs::write(&manifest_path, manifest_content).unwrap();

    let report = SyncEngine::sync(&temp_dir, false).expect("Sync should succeed");
    assert_eq!(report.generated_files.len(), 1);

    let lockfile =
        contract::sync::Lockfile::load_from_file(temp_dir.join("contract.lock")).unwrap();
    assert_eq!(lockfile.artifacts.len(), 1);
    let locked_art = lockfile
        .artifacts
        .get("src/sdk/orders.ts")
        .expect("Artifact should be locked");
    assert_eq!(locked_art.contract, "orders");
    assert_eq!(locked_art.lang, "ts");
    assert!(!locked_art.sha256.is_empty());

    let art_reports = SyncEngine::check_artifacts(&temp_dir).unwrap();
    assert_eq!(art_reports.len(), 1);
    assert_eq!(
        art_reports[0].status,
        contract::sync::ArtifactDriftStatus::UpToDate
    );

    let generated_file = temp_dir.join("src/sdk/orders.ts");
    fs::write(&generated_file, "// Tampered content by rogue process").unwrap();

    let tampered_reports = SyncEngine::check_artifacts(&temp_dir).unwrap();
    assert_eq!(tampered_reports.len(), 1);
    match &tampered_reports[0].status {
        contract::sync::ArtifactDriftStatus::Modified {
            expected_sha256,
            actual_sha256,
        } => {
            assert_eq!(expected_sha256, &locked_art.sha256);
            assert_ne!(expected_sha256, actual_sha256);
        }
        other => panic!("Expected Modified status, got {:?}", other),
    }

    fs::remove_file(&generated_file).unwrap();
    let missing_reports = SyncEngine::check_artifacts(&temp_dir).unwrap();
    assert_eq!(missing_reports.len(), 1);
    assert_eq!(
        missing_reports[0].status,
        contract::sync::ArtifactDriftStatus::MissingOnDisk
    );

    SyncEngine::sync(&temp_dir, false).expect("Resync should succeed");
    assert!(generated_file.exists());
    let restored_reports = SyncEngine::check_artifacts(&temp_dir).unwrap();
    assert_eq!(
        restored_reports[0].status,
        contract::sync::ArtifactDriftStatus::UpToDate
    );

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

    let report =
        SyncEngine::sync(&temp_dir, false).expect("Sync should succeed for proto dependency");
    assert_eq!(report.synced_contracts.len(), 1);
    assert_eq!(report.generated_files.len(), 1);

    let generated_ts = fs::read_to_string(temp_dir.join("generated/greeter.ts")).unwrap();
    assert!(generated_ts.contains("export interface HelloRequest"));
    assert!(generated_ts.contains("export interface HelloReply"));

    let _ = fs::remove_dir_all(temp_dir);
}
