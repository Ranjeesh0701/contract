use contract::codegen::{GoGenerator, PythonGenerator, RustGenerator, TypeScriptGenerator};
use contract::ir::{
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
                        deprecated: None,
                    },
                    EnumVariant {
                        name: "Paid".to_string(),
                        description: None,
                        deprecated: None,
                    },
                ],
                description: Some("Status of an order".to_string()),
                deprecated: None,
            }),
            TypeDefinition::Record(Record {
                name: "Order".to_string(),
                fields: vec![
                    Field {
                        name: "orderId".to_string(),
                        field_type: Type::String,
                        description: Some("Unique identifier".to_string()),
                        deprecated: None,
                    },
                    Field {
                        name: "status".to_string(),
                        field_type: Type::Reference("OrderStatus".to_string()),
                        description: None,
                        deprecated: None,
                    },
                    Field {
                        name: "notes".to_string(),
                        field_type: Type::Optional(Box::new(Type::String)),
                        description: None,
                        deprecated: None,
                    },
                ],
                description: Some("Customer order details".to_string()),
                deprecated: None,
            }),
        ],
        services: vec![Service {
            name: "OrderService".to_string(),
            description: Some("Order processing operations".to_string()),
            deprecated: None,
            operations: vec![Operation {
                name: "GetOrder".to_string(),
                input: Some(Type::String),
                output: Some(Type::Reference("Order".to_string())),
                description: Some("Fetches an order by ID".to_string()),
                deprecated: None,
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

    // Verify service trait & client generation
    assert!(rust_code.contains("pub trait OrderService {"));
    assert!(rust_code.contains("fn get_order(&self, input: String) -> Result<Order, Box<dyn std::error::Error + Send + Sync>>;"));
    assert!(rust_code.contains("pub struct OrderServiceClient {"));
    assert!(rust_code.contains("pub struct ShopContractClient {"));
    assert!(rust_code.contains("pub order_service: OrderServiceClient,"));
}

#[test]
fn test_python_codegen() {
    let contract = sample_contract();
    let py_code = PythonGenerator::generate(&contract);

    assert!(py_code.contains("class OrderStatus(str, Enum):"));
    assert!(py_code.contains("Pending = \"Pending\""));
    assert!(py_code.contains("class Order(BaseModel):"));
    assert!(
        py_code.contains(
            "order_id: str = Field(alias=\"orderId\", description=\"Unique identifier\")"
        )
    );
    assert!(py_code.contains("notes: Optional[str] = Field(default=None)"));
    assert!(py_code.contains("class OrderService(ABC):"));
    assert!(py_code.contains("def get_order(self, input: str) -> Order:"));
    assert!(py_code.contains("class OrderServiceClient(OrderService):"));
    assert!(py_code.contains("class ShopContractClient:"));
}

#[test]
fn test_python_codegen_selective_imports() {
    // Contract with only an enum (no services, no models, no optionals)
    let contract = contract::ir::Contract {
        name: "OnlyEnumContract".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        types: vec![contract::ir::TypeDefinition::Enum(contract::ir::EnumType {
            name: "SimpleStatus".to_string(),
            variants: vec![contract::ir::EnumVariant {
                name: "Active".to_string(),
                description: None,
                deprecated: None,
            }],
            description: None,
            deprecated: None,
        })],
        services: vec![],
    };

    let py_code = PythonGenerator::generate(&contract);

    assert!(py_code.contains("from enum import Enum"));
    assert!(!py_code.contains("from pydantic import"));
    assert!(!py_code.contains("from typing import Optional"));
    assert!(!py_code.contains("import urllib"));
    assert!(!py_code.contains("from abc import"));
}

#[test]
fn test_codegen_deprecation_tags_across_all_languages() {
    let contract = contract::ir::Contract {
        name: "LegacyService".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        types: vec![contract::ir::TypeDefinition::Record(contract::ir::Record {
            name: "LegacyUser".to_string(),
            fields: vec![contract::ir::Field {
                name: "oldField".to_string(),
                field_type: contract::ir::Type::String,
                description: Some("Old field description".to_string()),
                deprecated: Some("Use newField instead".to_string()),
            }],
            description: None,
            deprecated: Some("LegacyUser is deprecated".to_string()),
        })],
        services: vec![contract::ir::Service {
            name: "LegacyApi".to_string(),
            description: None,
            deprecated: None,
            operations: vec![contract::ir::Operation {
                name: "OldCall".to_string(),
                input: None,
                output: None,
                description: None,
                deprecated: Some("Use NewCall instead".to_string()),
            }],
        }],
    };

    let ts = TypeScriptGenerator::generate(&contract);
    assert!(ts.contains("@deprecated LegacyUser is deprecated"));
    assert!(ts.contains("@deprecated Use newField instead"));
    assert!(ts.contains("@deprecated Use NewCall instead"));

    let py = PythonGenerator::generate(&contract);
    assert!(py.contains("deprecated=True"));
    assert!(py.contains("old_field"));

    let go = GoGenerator::generate(&contract);
    assert!(go.contains("// Deprecated: LegacyUser is deprecated"));
    assert!(go.contains("// Deprecated: Use newField instead"));
    assert!(go.contains("// Deprecated: Use NewCall instead"));

    let rust = RustGenerator::generate(&contract);
    assert!(rust.contains("#[deprecated(note = \"LegacyUser is deprecated\")]"));
    assert!(rust.contains("#[deprecated(note = \"Use newField instead\")]"));
    assert!(rust.contains("#[deprecated(note = \"Use NewCall instead\")]"));
}

#[test]
fn test_go_codegen() {
    let contract = sample_contract();
    let go_code = GoGenerator::generate(&contract);

    assert!(go_code.contains("type OrderStatus string"));
    assert!(go_code.contains("OrderStatusPending OrderStatus = \"Pending\""));
    assert!(go_code.contains("type Order struct {"));
    assert!(go_code.contains("OrderID string `json:\"orderId\"`"));
    assert!(go_code.contains("Notes *string `json:\"notes,omitempty\"`"));
    assert!(go_code.contains("type OrderService interface {"));
    assert!(go_code.contains("GetOrder(ctx context.Context, input string) (*Order, error)"));
    assert!(go_code.contains("type OrderServiceClient struct {"));
    assert!(go_code.contains("type ShopContractClient struct {"));
}
