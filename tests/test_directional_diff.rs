use contract::diff::{ChangeSeverity, DiffDirection, diff, diff_with_direction};
use contract::parser::parse;

#[test]
fn test_response_payload_expansion_is_safe() {
    let old_src = r#"
    contract UserService {
        version: "1.0.0"

        model UserResponse {
            id: String;
            name: String;
        }

        service UserApi {
            getUser(String): UserResponse;
        }
    }
    "#;

    // Adding non-optional `email: String` to response payload
    let new_src = r#"
    contract UserService {
        version: "1.1.0"

        model UserResponse {
            id: String;
            name: String;
            email: String;
        }

        service UserApi {
            getUser(String): UserResponse;
        }
    }
    "#;

    let old = parse(old_src).unwrap();
    let new = parse(new_src).unwrap();

    let report = diff(&old, &new);
    assert!(
        report.is_compatible(),
        "Adding fields to response payloads must be backwards-compatible"
    );
    assert_eq!(report.breaking_changes().len(), 0);
    assert_eq!(report.changes.len(), 1);
    assert_eq!(report.changes[0].severity, ChangeSeverity::Safe);
    assert!(
        report.changes[0]
            .message
            .contains("response record 'UserResponse'")
    );
}

#[test]
fn test_request_payload_required_expansion_is_breaking() {
    let old_src = r#"
    contract UserService {
        version: "1.0.0"

        model CreateUserRequest {
            username: String;
        }

        service UserApi {
            createUser(CreateUserRequest): Void;
        }
    }
    "#;

    // Adding required `email: String` to request payload
    let new_src = r#"
    contract UserService {
        version: "1.1.0"

        model CreateUserRequest {
            username: String;
            email: String;
        }

        service UserApi {
            createUser(CreateUserRequest): Void;
        }
    }
    "#;

    let old = parse(old_src).unwrap();
    let new = parse(new_src).unwrap();

    let report = diff(&old, &new);
    assert!(
        !report.is_compatible(),
        "Adding required field to request payload must be breaking"
    );
    assert_eq!(report.breaking_changes().len(), 1);
    assert_eq!(
        report.breaking_changes()[0].severity,
        ChangeSeverity::Breaking
    );
    assert!(
        report.breaking_changes()[0]
            .message
            .contains("request record 'CreateUserRequest'")
    );
}

#[test]
fn test_request_payload_optional_expansion_is_safe() {
    let old_src = r#"
    contract UserService {
        version: "1.0.0"

        model CreateUserRequest {
            username: String;
        }

        service UserApi {
            createUser(CreateUserRequest): Void;
        }
    }
    "#;

    // Adding optional `bio: String?` to request payload
    let new_src = r#"
    contract UserService {
        version: "1.1.0"

        model CreateUserRequest {
            username: String;
            bio: String?;
        }

        service UserApi {
            createUser(CreateUserRequest): Void;
        }
    }
    "#;

    let old = parse(old_src).unwrap();
    let new = parse(new_src).unwrap();

    let report = diff(&old, &new);
    assert!(report.is_compatible());
    assert_eq!(report.breaking_changes().len(), 0);
    assert_eq!(report.changes[0].severity, ChangeSeverity::Safe);
}

#[test]
fn test_response_field_removal_is_breaking() {
    let old_src = r#"
    contract StoreService {
        version: "1.0.0"

        model ItemResponse {
            id: String;
            price: Float64;
        }

        service StoreApi {
            getItem(String): ItemResponse;
        }
    }
    "#;

    // Removing `price` from server response
    let new_src = r#"
    contract StoreService {
        version: "1.1.0"

        model ItemResponse {
            id: String;
        }

        service StoreApi {
            getItem(String): ItemResponse;
        }
    }
    "#;

    let old = parse(old_src).unwrap();
    let new = parse(new_src).unwrap();

    let report = diff(&old, &new);
    assert!(!report.is_compatible());
    assert_eq!(report.breaking_changes().len(), 1);
    assert!(
        report.breaking_changes()[0]
            .message
            .contains("response record 'ItemResponse'")
    );
}

#[test]
fn test_request_field_removal_is_warning() {
    let old_src = r#"
    contract StoreService {
        version: "1.0.0"

        model UpdateItemRequest {
            id: String;
            deprecatedParam: String?;
        }

        service StoreApi {
            updateItem(UpdateItemRequest): Void;
        }
    }
    "#;

    // Removing `deprecatedParam` from request
    let new_src = r#"
    contract StoreService {
        version: "1.1.0"

        model UpdateItemRequest {
            id: String;
        }

        service StoreApi {
            updateItem(UpdateItemRequest): Void;
        }
    }
    "#;

    let old = parse(old_src).unwrap();
    let new = parse(new_src).unwrap();

    let report = diff(&old, &new);
    assert!(
        report.is_compatible(),
        "Removing request field is backwards-compatible"
    );
    assert_eq!(report.breaking_changes().len(), 0);
    assert_eq!(report.warnings().len(), 1);
    assert!(
        report.warnings()[0]
            .message
            .contains("request record 'UpdateItemRequest' (clients may still send it)")
    );
}

#[test]
fn test_transitive_type_role_propagation() {
    let old_src = r#"
    contract OrderService {
        version: "1.0.0"

        model LineItem {
            sku: String;
        }

        model OrderResponse {
            items: LineItem[];
        }

        service OrderApi {
            getOrder(String): OrderResponse;
        }
    }
    "#;

    // Adding required field to LineItem (transitively an output-only type!)
    let new_src = r#"
    contract OrderService {
        version: "1.1.0"

        model LineItem {
            sku: String;
            quantity: Int32;
        }

        model OrderResponse {
            items: LineItem[];
        }

        service OrderApi {
            getOrder(String): OrderResponse;
        }
    }
    "#;

    let old = parse(old_src).unwrap();
    let new = parse(new_src).unwrap();

    let report = diff(&old, &new);
    assert!(
        report.is_compatible(),
        "Transitively output-only records are safe to expand"
    );
    assert_eq!(report.breaking_changes().len(), 0);
    assert_eq!(report.changes[0].severity, ChangeSeverity::Safe);
    assert!(
        report.changes[0]
            .message
            .contains("response record 'LineItem'")
    );
}

#[test]
fn test_directional_mode_diff() {
    let old_src = r#"
    contract Service {
        version: "1.0.0"
        model Req { id: String; oldParam: String?; }
        model Res { id: String; }
        service Api { call(Req): Res; }
    }
    "#;

    let new_src = r#"
    contract Service {
        version: "1.1.0"
        model Req { id: String; }
        model Res { id: String; extra: String; }
        service Api { call(Req): Res; }
    }
    "#;

    let old = parse(old_src).unwrap();
    let new = parse(new_src).unwrap();

    let report_both = diff_with_direction(&old, &new, DiffDirection::Both);
    assert_eq!(report_both.changes.len(), 2);

    let report_consumer = diff_with_direction(&old, &new, DiffDirection::Consumer);
    // Consumer cares about response addition and ignores server-side request param removal warning
    assert_eq!(report_consumer.changes.len(), 1);
    assert_eq!(report_consumer.changes[0].severity, ChangeSeverity::Safe);
}
