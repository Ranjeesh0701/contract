use codegen::{RustGenerator, TypeScriptGenerator};
use ir::{
    Contract, EnumType, EnumVariant, Field, Operation, Record, Service, Type, TypeDefinition,
};

fn sample_contract() -> Contract {
    Contract {
        name: "ShopContract".to_string(),
        version: "1.0.0".to_string(),
        description: Some("Shop domain types".to_string()),
        types: vec![
            TypeDefinition::Enum(EnumType {
                name: "OrderStatus".to_string(),
                variants: vec![
                    EnumVariant {
                        name: "Pending".to_string(),
                        description: Some("Order created, waiting payment".to_string()),
                    },
                    EnumVariant {
                        name: "Paid".to_string(),
                        description: None,
                    },
                ],
                description: Some("Status of an order".to_string()),
            }),
            TypeDefinition::Record(Record {
                name: "Order".to_string(),
                fields: vec![
                    Field {
                        name: "orderId".to_string(),
                        field_type: Type::String,
                        description: Some("Unique identifier".to_string()),
                    },
                    Field {
                        name: "status".to_string(),
                        field_type: Type::Reference("OrderStatus".to_string()),
                        description: None,
                    },
                    Field {
                        name: "notes".to_string(),
                        field_type: Type::Optional(Box::new(Type::String)),
                        description: None,
                    },
                ],
                description: Some("Customer order details".to_string()),
            }),
        ],
        services: vec![Service {
            name: "OrderService".to_string(),
            description: Some("Order processing operations".to_string()),
            operations: vec![Operation {
                name: "GetOrder".to_string(),
                input: Some(Type::String),
                output: Some(Type::Reference("Order".to_string())),
                description: Some("Fetches an order by ID".to_string()),
            }],
        }],
    }
}

#[test]
fn test_typescript_codegen() {
    let contract = sample_contract();
    let ts_code = TypeScriptGenerator::generate(&contract);

    assert!(ts_code.contains("export type OrderStatus ="));
    assert!(ts_code.contains("\"Pending\""));
    assert!(ts_code.contains("export interface Order {"));
    assert!(ts_code.contains("orderId: string;"));
    assert!(ts_code.contains("notes?: string;"));
    assert!(ts_code.contains("export interface OrderService {"));
    assert!(ts_code.contains("getOrder(input: string): Promise<Order>;"));
}

#[test]
fn test_rust_codegen() {
    let contract = sample_contract();
    let rust_code = RustGenerator::generate(&contract);

    assert!(rust_code.contains("pub enum OrderStatus {"));
    assert!(rust_code.contains("Pending,"));
    assert!(rust_code.contains("pub struct Order {"));
    assert!(rust_code.contains("#[serde(rename = \"orderId\")]"));
    assert!(rust_code.contains("pub order_id: String,"));
    assert!(rust_code.contains("pub notes: Option<String>,"));
}
