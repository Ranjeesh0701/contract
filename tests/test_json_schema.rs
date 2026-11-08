use contract::adapters::{default_registry, from_json_schema};
use contract::ir::Type;
use std::path::Path;

#[test]
fn test_parse_json_schema() {
    let schema_json = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "UserEvent",
  "description": "User event payload",
  "type": "object",
  "required": ["userId", "action"],
  "properties": {
    "userId": { "type": "string" },
    "action": {
      "type": "string",
      "enum": ["LOGIN", "LOGOUT", "PURCHASE"]
    },
    "score": { "type": "number" },
    "tags": {
      "type": "array",
      "items": { "type": "string" }
    }
  },
  "$defs": {
    "Location": {
      "type": "object",
      "required": ["country"],
      "properties": {
        "country": { "type": "string" },
        "city": { "type": "string" }
      }
    }
  }
}"#;

    let contract = from_json_schema(schema_json).expect("Should parse JSON schema");

    assert_eq!(contract.name, "UserEvent");
    assert_eq!(contract.description, Some("User event payload".to_string()));

    // Verify root object and $defs extracted
    let user_event = contract
        .types
        .iter()
        .find(|t| t.name() == "UserEvent")
        .expect("UserEvent record exists");
    if let contract::ir::TypeDefinition::Record(r) = user_event {
        let user_id = r.fields.iter().find(|f| f.name == "userId").unwrap();
        assert_eq!(user_id.field_type, Type::String);

        let score = r.fields.iter().find(|f| f.name == "score").unwrap();
        assert_eq!(score.field_type, Type::Optional(Box::new(Type::Float64)));

        let tags = r.fields.iter().find(|f| f.name == "tags").unwrap();
        assert_eq!(
            tags.field_type,
            Type::Optional(Box::new(Type::List(Box::new(Type::String))))
        );
    } else {
        panic!("Expected Record");
    }

    let location = contract
        .types
        .iter()
        .find(|t| t.name() == "Location")
        .expect("Location record exists");
    if let contract::ir::TypeDefinition::Record(r) = location {
        let country = r.fields.iter().find(|f| f.name == "country").unwrap();
        assert_eq!(country.field_type, Type::String);

        let city = r.fields.iter().find(|f| f.name == "city").unwrap();
        assert_eq!(city.field_type, Type::Optional(Box::new(Type::String)));
    } else {
        panic!("Expected Record");
    }

    // Verify registry auto-detection
    let registry = default_registry();
    let adapter = registry
        .find_adapter(Some(Path::new("event.json")), schema_json)
        .expect("Registry should find json_schema adapter");
    assert_eq!(adapter.name(), "json_schema");
}
