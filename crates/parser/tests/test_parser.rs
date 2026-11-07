use ir::Type;
use parser::parse;

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
    if let ir::TypeDefinition::Record(rec) = &contract.types[1] {
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
