use contract::adapters::{default_registry, from_protobuf};
use contract::ir::Type;
use std::path::Path;

#[test]
fn test_parse_protobuf_contract() {
    let proto_src = r#"
syntax = "proto3";

package store.v1;

// Status of the order
enum OrderStatus {
    PENDING = 0;
    PAID = 1;
    SHIPPED = 2;
}

// Order details
message Order {
    int64 id = 1;
    string customer_name = 2;
    OrderStatus status = 3;
    repeated string tags = 4;
    optional string notes = 5;
}

message GetOrderRequest {
    int64 id = 1;
}

service OrderService {
    rpc GetOrder (GetOrderRequest) returns (Order);
}
"#;

    let contract = from_protobuf(proto_src).expect("Should parse protobuf");

    assert_eq!(contract.name, "StoreV1");
    assert_eq!(contract.types.len(), 3);
    assert_eq!(contract.services.len(), 1);

    // Verify Enum
    let status_type = contract
        .types
        .iter()
        .find(|t| t.name() == "OrderStatus")
        .unwrap();
    if let contract::ir::TypeDefinition::Enum(e) = status_type {
        assert_eq!(e.variants.len(), 3);
        assert_eq!(e.variants[0].name, "PENDING");
        assert_eq!(e.variants[1].name, "PAID");
    } else {
        panic!("Expected EnumType");
    }

    // Verify Message
    let order_type = contract.types.iter().find(|t| t.name() == "Order").unwrap();
    if let contract::ir::TypeDefinition::Record(r) = order_type {
        assert_eq!(r.fields.len(), 5);

        let id_field = r.fields.iter().find(|f| f.name == "id").unwrap();
        assert_eq!(id_field.field_type, Type::Int64);

        let tags_field = r.fields.iter().find(|f| f.name == "tags").unwrap();
        assert_eq!(tags_field.field_type, Type::List(Box::new(Type::String)));

        let notes_field = r.fields.iter().find(|f| f.name == "notes").unwrap();
        assert_eq!(
            notes_field.field_type,
            Type::Optional(Box::new(Type::String))
        );
    } else {
        panic!("Expected Record");
    }

    // Verify Service
    let svc = &contract.services[0];
    assert_eq!(svc.name, "OrderService");
    assert_eq!(svc.operations.len(), 1);
    assert_eq!(svc.operations[0].name, "GetOrder");
    assert_eq!(
        svc.operations[0].input,
        Some(Type::Reference("GetOrderRequest".to_string()))
    );
    assert_eq!(
        svc.operations[0].output,
        Some(Type::Reference("Order".to_string()))
    );

    // Verify registry auto-detection
    let registry = default_registry();
    let adapter = registry
        .find_adapter(Some(Path::new("service.proto")), proto_src)
        .expect("Registry should find protobuf adapter");
    assert_eq!(adapter.name(), "protobuf");
}
