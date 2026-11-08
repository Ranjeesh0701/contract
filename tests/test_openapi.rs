use contract::adapters::{default_registry, from_openapi_json};
use contract::ir::Type;

fn sample_openapi() -> &'static str {
    r##"
    {
      "openapi": "3.0.0",
      "info": {
        "title": "PetstoreApi",
        "version": "1.0.0",
        "description": "Sample OpenAPI contract"
      },
      "paths": {
        "/pets": {
          "get": {
            "operationId": "listPets",
            "responses": {
              "200": {
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/Pet" }
                    }
                  }
                }
              }
            }
          }
        }
      },
      "components": {
        "schemas": {
          "PetStatus": {
            "type": "string",
            "enum": ["available", "pending", "sold"]
          },
          "Pet": {
            "type": "object",
            "required": ["id", "name"],
            "properties": {
              "id": { "type": "integer" },
              "name": { "type": "string" },
              "status": { "$ref": "#/components/schemas/PetStatus" },
              "tag": { "type": "string" }
            }
          }
        }
      }
    }
    "##
}

#[test]
fn test_ingest_openapi_petstore() {
    let openapi_sample = sample_openapi();
    let contract = from_openapi_json(openapi_sample).expect("Should parse OpenAPI");

    assert_eq!(contract.name, "PetstoreApi");
    assert_eq!(contract.version, "1.0.0");
    assert_eq!(
        contract.description,
        Some("Sample OpenAPI contract".to_string())
    );

    // Verify Enum extraction
    let pet_status = contract
        .types
        .iter()
        .find(|t| t.name() == "PetStatus")
        .expect("PetStatus enum exists");
    match pet_status {
        contract::ir::TypeDefinition::Enum(e) => {
            assert_eq!(e.variants.len(), 3);
            assert_eq!(e.variants[0].name, "available");
        }
        _ => panic!("Expected enum"),
    }

    // Verify Record extraction
    let pet = contract
        .types
        .iter()
        .find(|t| t.name() == "Pet")
        .expect("Pet model exists");
    match pet {
        contract::ir::TypeDefinition::Record(r) => {
            assert_eq!(r.fields.len(), 4);
            let id_field = r.fields.iter().find(|f| f.name == "id").unwrap();
            assert_eq!(id_field.field_type, Type::Int64);

            let tag_field = r.fields.iter().find(|f| f.name == "tag").unwrap();
            assert_eq!(tag_field.field_type, Type::Optional(Box::new(Type::String)));
        }
        _ => panic!("Expected record"),
    }

    // Verify Services extraction
    assert_eq!(contract.services.len(), 1);
    let service = &contract.services[0];
    assert_eq!(service.name, "PetstoreApiApi");
    assert_eq!(service.operations.len(), 1);
    assert_eq!(service.operations[0].name, "listPets");
}

#[test]
fn test_adapter_interface_and_registry() {
    let openapi_sample = sample_openapi();
    let registry = default_registry();

    // Registry auto-detection
    let adapter = registry
        .find_adapter(None, openapi_sample)
        .expect("Should detect OpenAPI adapter");
    assert_eq!(adapter.name(), "openapi");
    assert_eq!(adapter.supported_extensions(), &["json", "yaml", "yml"]);

    // Parse via SchemaAdapter trait
    let contract = adapter
        .parse(openapi_sample)
        .expect("Should parse via adapter trait");
    assert_eq!(contract.name, "PetstoreApi");
    assert_eq!(contract.types.len(), 2);
}
