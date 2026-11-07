use adapter_openapi::from_openapi_json;
use ir::Type;

#[test]
fn test_ingest_openapi_petstore() {
    let openapi_sample = r##"
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
    "##;

    let contract = from_openapi_json(openapi_sample).expect("Failed to parse OpenAPI");
    assert_eq!(contract.name, "PetstoreApi");
    assert_eq!(contract.version, "1.0.0");
    assert_eq!(contract.types.len(), 2);
    assert_eq!(contract.services.len(), 1);

    // Validate the contract
    assert!(contract.validate().is_ok());

    // Verify Pet record
    let pet_type = contract
        .types
        .iter()
        .find(|t| t.name() == "Pet")
        .expect("Pet type not found");

    if let ir::TypeDefinition::Record(rec) = pet_type {
        let tag_field = rec.fields.iter().find(|f| f.name == "tag").unwrap();
        // Since "tag" is not in required, it should be Optional
        assert_eq!(tag_field.field_type, Type::Optional(Box::new(Type::String)));

        let id_field = rec.fields.iter().find(|f| f.name == "id").unwrap();
        // "id" is in required, so non-optional Int64
        assert_eq!(id_field.field_type, Type::Int64);
    } else {
        panic!("Pet should be a Record");
    }
}
