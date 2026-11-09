use contract::diff::{
    ChangeSeverity, EndpointDirection, analyze_diff_impact, analyze_impact, analyze_target_impact,
};
use contract::parser::parse;

#[test]
fn test_direct_operation_impact() {
    let old_src = r#"
contract Api {
    version: "1.0.0"

    model User {
        id: string
    }

    service UserService {
        GetUser(string): User;
        DeleteUser(string): void;
    }
}
"#;

    let new_src = r#"
contract Api {
    version: "1.0.0"

    model User {
        id: string
    }

    service UserService {
        GetUser(string): User;
    }
}
"#;

    let old_c = parse(old_src).unwrap();
    let new_c = parse(new_src).unwrap();

    let report = analyze_impact(&old_c, &new_c);
    assert!(report.has_breaking_impact());
    assert!(report.total_impacted_operations >= 1);

    let del_op = report
        .impacted_endpoints
        .iter()
        .find(|ep| ep.operation == "DeleteUser");
    assert!(del_op.is_some());
    let del = del_op.unwrap();
    assert_eq!(del.service, "UserService");
    assert_eq!(del.severity, ChangeSeverity::Breaking);
}

#[test]
fn test_transitive_nested_field_impact() {
    // Address (depth 0) -> User (depth 1) -> Order (depth 2) -> OrderService (depth 3)
    let old_src = r#"
contract OrderApi {
    version: "1.0.0"

    model Address {
        street: string
        zipCode: string
    }

    model User {
        name: string
        address: Address
    }

    model Order {
        orderId: string
        buyer: User
    }

    service OrderService {
        GetOrder(string): Order;
        CreateOrder(Order): string;
    }
}
"#;

    let new_src = r#"
contract OrderApi {
    version: "1.0.0"

    model Address {
        street: string
        // zipCode removed! (Breaking change in Address)
    }

    model User {
        name: string
        address: Address
    }

    model Order {
        orderId: string
        buyer: User
    }

    service OrderService {
        GetOrder(string): Order;
        CreateOrder(Order): string;
    }
}
"#;

    let old_c = parse(old_src).unwrap();
    let new_c = parse(new_src).unwrap();

    let report = analyze_impact(&old_c, &new_c);

    assert!(report.has_breaking_impact());
    assert_eq!(report.total_impacted_services, 1);
    assert_eq!(report.total_impacted_operations, 2);

    // Verify affected types set
    assert!(report.affected_types.contains(&"Address".to_string()));
    assert!(report.affected_types.contains(&"User".to_string()));
    assert!(report.affected_types.contains(&"Order".to_string()));

    // Verify GetOrder (Output)
    let get_op = report
        .impacted_endpoints
        .iter()
        .find(|ep| ep.operation == "GetOrder")
        .expect("GetOrder should be impacted");
    assert_eq!(get_op.direction, EndpointDirection::Output);
    assert_eq!(get_op.depth, 3);
    assert!(
        get_op
            .path
            .contains(&"OrderService.GetOrder (Output)".to_string())
    );

    // Verify CreateOrder (Input)
    let create_op = report
        .impacted_endpoints
        .iter()
        .find(|ep| ep.operation == "CreateOrder")
        .expect("CreateOrder should be impacted");
    assert_eq!(create_op.direction, EndpointDirection::Input);
    assert_eq!(create_op.depth, 3);
}

#[test]
fn test_multiple_services_blast_radius() {
    let old_src = r#"
contract MultiServiceApi {
    version: "1.0.0"

    model Account {
        accountId: string
        balance: int64
    }

    service BillingService {
        Charge(Account): void;
    }

    service AuthService {
        Verify(Account): boolean;
    }

    service ReportService {
        GetStatement(string): Account;
    }
}
"#;

    let new_src = r#"
contract MultiServiceApi {
    version: "1.0.0"

    model Account {
        accountId: string
        // balance removed
    }

    service BillingService {
        Charge(Account): void;
    }

    service AuthService {
        Verify(Account): boolean;
    }

    service ReportService {
        GetStatement(string): Account;
    }
}
"#;

    let old_c = parse(old_src).unwrap();
    let new_c = parse(new_src).unwrap();

    let report = analyze_impact(&old_c, &new_c);
    assert_eq!(report.total_impacted_services, 3);
    assert_eq!(report.total_impacted_operations, 3);

    let services: Vec<&str> = report
        .impacted_endpoints
        .iter()
        .map(|ep| ep.service.as_str())
        .collect();
    assert!(services.contains(&"BillingService"));
    assert!(services.contains(&"AuthService"));
    assert!(services.contains(&"ReportService"));

    assert!(report.blast_radius_score >= 30);
}

#[test]
fn test_target_symbol_exploration() {
    let src = r#"
contract App {
    version: "1.0.0"

    model Currency {
        code: string
    }

    model Money {
        amount: int64
        currency: Currency
    }

    model Product {
        name: string
        price: Money
    }

    service CatalogService {
        GetProduct(string): Product;
    }
}
"#;

    let contract = parse(src).unwrap();

    // Query impact of touching "Currency"
    let report = analyze_target_impact(&contract, "Currency");
    assert_eq!(report.total_impacted_operations, 1);
    let ep = &report.impacted_endpoints[0];
    assert_eq!(ep.service, "CatalogService");
    assert_eq!(ep.operation, "GetProduct");
    assert_eq!(ep.depth, 3); // Currency -> Money -> Product -> CatalogService.GetProduct

    assert!(report.affected_types.contains(&"Currency".to_string()));
    assert!(report.affected_types.contains(&"Money".to_string()));
    assert!(report.affected_types.contains(&"Product".to_string()));
}

#[test]
fn test_recursive_type_cycle_resilience() {
    let old_src = r#"
contract GraphApi {
    version: "1.0.0"

    model Node {
        id: string
        left: Node?
        right: Node?
        tag: string
    }

    service GraphService {
        GetRoot(): Node;
    }
}
"#;

    let new_src = r#"
contract GraphApi {
    version: "1.0.0"

    model Node {
        id: string
        left: Node?
        right: Node?
        // tag removed
    }

    service GraphService {
        GetRoot(): Node;
    }
}
"#;

    let old_c = parse(old_src).unwrap();
    let new_c = parse(new_src).unwrap();

    // Must terminate without stack overflow or infinite loop
    let report = analyze_impact(&old_c, &new_c);
    assert_eq!(report.total_impacted_operations, 1);
    assert_eq!(report.impacted_endpoints[0].service, "GraphService");
    assert_eq!(report.impacted_endpoints[0].operation, "GetRoot");
}

#[test]
fn test_type_alias_impact_chain() {
    let old_src = r#"
contract AliasApi {
    version: "1.0.0"

    type CustomerId: string;

    model Customer {
        id: CustomerId
        name: string
    }

    service CustomerService {
        GetCustomer(CustomerId): Customer;
    }
}
"#;

    let new_src = r#"
contract AliasApi {
    version: "1.0.0"

    type CustomerId: int64; // Changed underlying type

    model Customer {
        id: CustomerId
        name: string
    }

    service CustomerService {
        GetCustomer(CustomerId): Customer;
    }
}
"#;

    let old_c = parse(old_src).unwrap();
    let new_c = parse(new_src).unwrap();

    let report = analyze_impact(&old_c, &new_c);
    assert!(report.total_impacted_operations >= 1);
    assert_eq!(report.impacted_endpoints[0].service, "CustomerService");
    assert_eq!(report.impacted_endpoints[0].operation, "GetCustomer");
}

#[test]
fn test_filtered_diff_impact() {
    let old_src = r#"
contract Store {
    version: "1.0.0"

    model Product {
        id: string
        sku: string
    }

    model Review {
        id: string
        rating: int32
    }

    service ProductService {
        GetProduct(string): Product;
    }

    service ReviewService {
        GetReview(string): Review;
    }
}
"#;

    let new_src = r#"
contract Store {
    version: "1.0.0"

    model Product {
        id: string
    }

    model Review {
        id: string
    }

    service ProductService {
        GetProduct(string): Product;
    }

    service ReviewService {
        GetReview(string): Review;
    }
}
"#;

    let old_c = parse(old_src).unwrap();
    let new_c = parse(new_src).unwrap();

    // Filter specifically for "Product"
    let report = analyze_diff_impact(&old_c, &new_c, Some("Product"));
    assert_eq!(report.total_impacted_services, 1);
    assert_eq!(report.impacted_endpoints[0].service, "ProductService");
    assert!(!report.affected_types.contains(&"Review".to_string()));
}
