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
