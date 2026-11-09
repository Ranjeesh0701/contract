use contract::ir::Type;
use contract::parser::parse;

#[test]
fn test_parse_complete_contract_dsl() {
    let source = r#"
    contract PaymentService {
        version: "2.1.0"
        description: "Payment and ledger service"
    }

    enum Currency {
        USD,
        EUR,
        GBP
    }

    model Transaction {
        id: String
        amount: Float64
        currency: Currency
        description: String?
        tags: String[]
        metadata: Map<String, String>?
    }

    service PaymentApi {
        processTransaction(Transaction): Bool
    }
    "#;

    let contract = parse(source).expect("Failed to parse contract DSL");
    assert_eq!(contract.name, "PaymentService");
    assert_eq!(contract.version, "2.1.0");
    assert_eq!(contract.types.len(), 2);
    assert_eq!(contract.services.len(), 1);

    // Validate the parsed contract through semantic analysis
    assert!(contract.validate().is_ok());

    // Verify fields
    if let contract::ir::TypeDefinition::Record(rec) = &contract.types[1] {
        assert_eq!(rec.name, "Transaction");
        assert_eq!(rec.fields.len(), 6);
        assert_eq!(
            rec.fields[3].field_type,
            Type::Optional(Box::new(Type::String))
        );
        assert_eq!(rec.fields[4].field_type, Type::List(Box::new(Type::String)));
    } else {
        panic!("Expected Transaction to be a Record");
    }
}

#[test]
fn test_parse_nested_and_resilient_syntax() {
    let source = r#"
    /* Contract with block comment */
    # Also hash comment
    contract ResilientService {
        version: "1.0.0";
        description: "Testing resilience";

        model User {
            id: String;
            name: String,
            email: String?,
        }

        enum Role {
            Admin,
            Member;
        }

        service UserService {
            deleteUser(String): Void;
            ping(): Void
        }
    }
    "#;

    let contract = parse(source).expect("Failed to parse resilient contract DSL");
    assert_eq!(contract.name, "ResilientService");
    assert_eq!(contract.types.len(), 2);
    assert_eq!(contract.services.len(), 1);
    assert!(contract.validate().is_ok());

    let svc = &contract.services[0];
    assert_eq!(svc.operations.len(), 2);
    assert_eq!(svc.operations[0].output, None);
    assert_eq!(svc.operations[1].output, None);
}

#[test]
fn test_parse_dsl_docstrings_and_codegen_propagation() {
    let source = r#"
    /// Billing and checkout domain API
    contract BillingService {
        version: "1.0.0"
    }

    /// Status of a payment lifecycle
    enum PaymentStatus {
        /// Payment initiated but unconfirmed
        Pending,
        /// Payment settled successfully
        Settled
    }

    /// Invoiced line item
    model InvoiceItem {
        /// Product SKU identifier
        sku: String
        /// Unit price in cents
        priceCents: Int64
    }

    /// Alias for currency code
    type CurrencyCode: String

    /// Billing operations for merchant clients
    service BillingApi {
        /// Charge an invoice
        charge(InvoiceItem): Bool
    }
    "#;

    let contract = parse(source).expect("Should parse DSL with docstrings");
    assert_eq!(
        contract.description.as_deref(),
        Some("Billing and checkout domain API")
    );

    // Verify Enum & Variant docstrings
    let payment_status = match &contract.types[0] {
        contract::ir::TypeDefinition::Enum(e) => e,
        _ => panic!("Expected enum"),
    };
    assert_eq!(
        payment_status.description.as_deref(),
        Some("Status of a payment lifecycle")
    );
    assert_eq!(
        payment_status.variants[0].description.as_deref(),
        Some("Payment initiated but unconfirmed")
    );
    assert_eq!(
        payment_status.variants[1].description.as_deref(),
        Some("Payment settled successfully")
    );

    // Verify Model & Field docstrings
    let invoice_item = match &contract.types[1] {
        contract::ir::TypeDefinition::Record(r) => r,
        _ => panic!("Expected record"),
    };
    assert_eq!(
        invoice_item.description.as_deref(),
        Some("Invoiced line item")
    );
    assert_eq!(
        invoice_item.fields[0].description.as_deref(),
        Some("Product SKU identifier")
    );
    assert_eq!(
        invoice_item.fields[1].description.as_deref(),
        Some("Unit price in cents")
    );

    // Verify Alias docstrings
    let alias = match &contract.types[2] {
        contract::ir::TypeDefinition::Alias(a) => a,
        _ => panic!("Expected alias"),
    };
    assert_eq!(
        alias.description.as_deref(),
        Some("Alias for currency code")
    );

    // Verify Service & Operation docstrings
    assert_eq!(contract.services.len(), 1);
    let service = &contract.services[0];
    assert_eq!(
        service.description.as_deref(),
        Some("Billing operations for merchant clients")
    );
    assert_eq!(
        service.operations[0].description.as_deref(),
        Some("Charge an invoice")
    );

    // Verify codegen propagation to TypeScript
    let ts = contract::codegen::TypeScriptGenerator::generate(&contract);
    assert!(ts.contains("Status of a payment lifecycle"));
    assert!(ts.contains("Invoiced line item"));
    assert!(ts.contains("Product SKU identifier"));
    assert!(ts.contains("Charge an invoice"));

    // Verify codegen propagation to Python
    let py = contract::codegen::PythonGenerator::generate(&contract);
    assert!(py.contains("Status of a payment lifecycle"));
    assert!(py.contains("Invoiced line item"));
    assert!(py.contains("description=\"Product SKU identifier\""));

    // Verify codegen propagation to Rust
    let rs = contract::codegen::RustGenerator::generate(&contract);
    assert!(rs.contains("/// Invoiced line item"));
    assert!(rs.contains("/// Product SKU identifier"));
    assert!(rs.contains("/// Status of a payment lifecycle"));

    // Verify codegen propagation to Go
    let go = contract::codegen::GoGenerator::generate(&contract);
    assert!(go.contains("// Invoiced line item"));
    assert!(go.contains("// Product SKU identifier"));
}
