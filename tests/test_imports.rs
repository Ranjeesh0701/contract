use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use contract::parser::{
    ResolverError, resolve_contract_file, resolve_contract_str,
};

fn create_temp_dir(suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("contract_imports_test_{}_{}", suffix, nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_basic_relative_import() {
    let dir = create_temp_dir("basic");
    let types_path = dir.join("types.contract");
    let user_path = dir.join("user.contract");

    fs::write(
        &types_path,
        r#"
model Address {
    street: string
    city: string
}
"#,
    )
    .unwrap();

    fs::write(
        &user_path,
        r#"
import "types.contract";

contract UserService {
    version: "1.0.0"
}

model User {
    id: string
    address: Address
}
"#,
    )
    .unwrap();

    let contract = resolve_contract_file(&user_path).expect("Failed to resolve user contract");
    assert_eq!(contract.name, "UserService");
    assert_eq!(contract.version, "1.0.0");

    let type_names: Vec<&str> = contract.types.iter().map(|t| t.name()).collect();
    assert!(type_names.contains(&"Address"));
    assert!(type_names.contains(&"User"));

    // Validation should succeed without any unresolved reference errors
    contract.validate().expect("Contract validation failed");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_transitive_imports() {
    // A -> B -> C
    let dir = create_temp_dir("transitive");
    let c_path = dir.join("c.contract");
    let b_path = dir.join("b.contract");
    let a_path = dir.join("a.contract");

    fs::write(
        &c_path,
        r#"
model CModel {
    value: int64
}
"#,
    )
    .unwrap();

    fs::write(
        &b_path,
        r#"
import "c.contract";

model BModel {
    c: CModel
}
"#,
    )
    .unwrap();

    fs::write(
        &a_path,
        r#"
import "b.contract";

contract RootContract {
    version: "2.0.0"
}

model AModel {
    b: BModel
}
"#,
    )
    .unwrap();

    let contract = resolve_contract_file(&a_path).expect("Failed to resolve transitive imports");
    let type_names: Vec<&str> = contract.types.iter().map(|t| t.name()).collect();

    assert_eq!(type_names.len(), 3);
    assert!(type_names.contains(&"CModel"));
    assert!(type_names.contains(&"BModel"));
    assert!(type_names.contains(&"AModel"));

    contract.validate().expect("Contract validation failed");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_diamond_imports_deduplication() {
    // Root -> Left -> Shared
    // Root -> Right -> Shared
    let dir = create_temp_dir("diamond");
    let shared_path = dir.join("shared.contract");
    let left_path = dir.join("left.contract");
    let right_path = dir.join("right.contract");
    let root_path = dir.join("root.contract");

    fs::write(
        &shared_path,
        r#"
model SharedEntity {
    id: string
}
"#,
    )
    .unwrap();

    fs::write(
        &left_path,
        r#"
import "shared.contract";

model LeftEntity {
    shared: SharedEntity
}
"#,
    )
    .unwrap();

    fs::write(
        &right_path,
        r#"
import "shared.contract";

model RightEntity {
    shared: SharedEntity
}
"#,
    )
    .unwrap();

    fs::write(
        &root_path,
        r#"
import "left.contract";
import "right.contract";

contract DiamondRoot {
    version: "1.0.0"
}

model RootEntity {
    left: LeftEntity
    right: RightEntity
}
"#,
    )
    .unwrap();

    let contract = resolve_contract_file(&root_path).expect("Failed to resolve diamond imports");
    let type_names: Vec<&str> = contract.types.iter().map(|t| t.name()).collect();

    // SharedEntity should appear exactly once
    assert_eq!(
        type_names.iter().filter(|&&n| n == "SharedEntity").count(),
        1
    );
    assert!(type_names.contains(&"LeftEntity"));
    assert!(type_names.contains(&"RightEntity"));
    assert!(type_names.contains(&"RootEntity"));

    contract.validate().expect("Validation failed for diamond contract");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_circular_import_detection() {
    let dir = create_temp_dir("circular");
    let a_path = dir.join("circ_a.contract");
    let b_path = dir.join("circ_b.contract");

    fs::write(
        &a_path,
        r#"
import "circ_b.contract";

model ModelA {
    name: string
}
"#,
    )
    .unwrap();

    fs::write(
        &b_path,
        r#"
import "circ_a.contract";

model ModelB {
    count: int64
}
"#,
    )
    .unwrap();

    let err = resolve_contract_file(&a_path).expect_err("Should detect circular import");
    match err {
        ResolverError::CircularImport { cycle } => {
            assert!(cycle.len() >= 2);
            let display_cycle = cycle.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" -> ");
            assert!(display_cycle.contains("circ_a.contract"));
            assert!(display_cycle.contains("circ_b.contract"));
        }
        other => panic!("Expected CircularImport, got {:?}", other),
    }

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_type_conflict_detection() {
    let dir = create_temp_dir("conflict");
    let mod1_path = dir.join("mod1.contract");
    let mod2_path = dir.join("mod2.contract");
    let main_path = dir.join("main.contract");

    fs::write(
        &mod1_path,
        r#"
model ConflictingType {
    a: string
}
"#,
    )
    .unwrap();

    fs::write(
        &mod2_path,
        r#"
model ConflictingType {
    b: int32
}
"#,
    )
    .unwrap();

    fs::write(
        &main_path,
        r#"
import "mod1.contract";
import "mod2.contract";

model MainModel {
    status: string
}
"#,
    )
    .unwrap();

    let err = resolve_contract_file(&main_path).expect_err("Should detect type collision");
    match err {
        ResolverError::TypeConflict { type_name, .. } => {
            assert_eq!(type_name, "ConflictingType");
        }
        other => panic!("Expected TypeConflict, got {:?}", other),
    }

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_missing_file_error() {
    let dir = create_temp_dir("missing");
    let main_path = dir.join("main.contract");

    fs::write(
        &main_path,
        r#"
import "does_not_exist.contract";

model ValidModel {
    name: string
}
"#,
    )
    .unwrap();

    let err = resolve_contract_file(&main_path).expect_err("Should fail on missing file");
    match err {
        ResolverError::FileNotFound(path) => {
            assert!(path.display().to_string().contains("does_not_exist.contract"));
        }
        other => panic!("Expected FileNotFound, got {:?}", other),
    }

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_resolve_str_with_base_dir() {
    let dir = create_temp_dir("str_resolve");
    let common_path = dir.join("common.contract");

    fs::write(
        &common_path,
        r#"
enum UserRole {
    Admin,
    Member,
    Guest
}
"#,
    )
    .unwrap();

    let source = r#"
import "common.contract";

contract AdminService {
    version: "1.0.0"
}

model Profile {
    role: UserRole
}
"#;

    let contract = resolve_contract_str(source, &dir).expect("Failed to resolve from string");
    assert_eq!(contract.name, "AdminService");
    assert_eq!(contract.types.len(), 2);
    contract.validate().expect("Contract validation failed");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_nested_subdirectories_navigation() {
    let dir = create_temp_dir("nested");
    let domain_dir = dir.join("domain").join("users");
    let common_dir = dir.join("common");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::create_dir_all(&common_dir).unwrap();

    let id_contract = common_dir.join("id.contract");
    let user_contract = domain_dir.join("user.contract");
    let root_contract = dir.join("app.contract");

    fs::write(
        &id_contract,
        r#"
type UserId: string;
"#,
    )
    .unwrap();

    fs::write(
        &user_contract,
        r#"
import "../../common/id.contract";

model User {
    id: UserId
    username: string
}
"#,
    )
    .unwrap();

    fs::write(
        &root_contract,
        r#"
import "domain/users/user.contract";

contract AppContract {
    version: "1.0.0"
}
"#,
    )
    .unwrap();

    let contract = resolve_contract_file(&root_contract).expect("Failed to resolve nested navigation");
    let type_names: Vec<&str> = contract.types.iter().map(|t| t.name()).collect();

    assert!(type_names.contains(&"UserId"));
    assert!(type_names.contains(&"User"));
    contract.validate().expect("Validation failed for nested imports");

    let _ = fs::remove_dir_all(dir);
}
