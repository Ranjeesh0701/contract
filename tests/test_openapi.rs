use contract::adapters::{default_registry, from_openapi_json, to_openapi_json};
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

#[test]
fn test_export_openapi_json() {
    let dsl = r#"
    /// Online store catalog API
    contract CatalogApi {
        version: "2.0.0"
    }

    /// Status of catalog item
    enum ItemStatus {
        InStock,
        OutOfStock
    }

    /// Item in catalog
    model CatalogItem {
        /// Unique SKU identifier
        id: String
        /// Display price
        price: Float64
        /// Availability status
        status: ItemStatus
        /// Description of the product
        description: String?
    }

    service CatalogService {
        /// Create a new catalog item
        createItem(CatalogItem): CatalogItem
    }
    "#;

    let contract = contract::parser::parse(dsl).expect("Should parse DSL");
    let openapi_json = to_openapi_json(&contract).expect("Should export to OpenAPI");

    let val: serde_json::Value =
        serde_json::from_str(&openapi_json).expect("Should produce valid JSON");
    assert_eq!(val["openapi"], "3.1.0");
    assert_eq!(val["info"]["title"], "CatalogApi");
    assert_eq!(val["info"]["version"], "2.0.0");
    assert_eq!(val["info"]["description"], "Online store catalog API");

    // Check components
    assert_eq!(val["components"]["schemas"]["ItemStatus"]["type"], "string");
    assert_eq!(
        val["components"]["schemas"]["ItemStatus"]["enum"],
        serde_json::json!(["InStock", "OutOfStock"])
    );

    let item_schema = &val["components"]["schemas"]["CatalogItem"];
    assert_eq!(item_schema["type"], "object");
    assert_eq!(
        item_schema["required"],
        serde_json::json!(["id", "price", "status"])
    );
    assert_eq!(item_schema["properties"]["id"]["type"], "string");
    assert_eq!(item_schema["properties"]["price"]["type"], "number");
    assert_eq!(
        item_schema["properties"]["status"]["$ref"],
        "#/components/schemas/ItemStatus"
    );

    // Check paths
    let paths = val["paths"].as_object().expect("Paths should exist");
    assert_eq!(paths.len(), 1);
    let op = &paths["/catalog-service/create-item"]["post"];
    assert_eq!(op["operationId"], "createItem");
    assert_eq!(op["summary"], "Create a new catalog item");
    assert_eq!(
        op["requestBody"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/CatalogItem"
    );
    assert_eq!(
        op["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/CatalogItem"
    );
}

#[test]
fn test_openapi_roundtrip() {
    let original_sample = sample_openapi();
    let contract_1 = from_openapi_json(original_sample).expect("Parse original OpenAPI");

    let exported_openapi = to_openapi_json(&contract_1).expect("Export to OpenAPI");
    let contract_2 = from_openapi_json(&exported_openapi).expect("Re-parse exported OpenAPI");

    assert_eq!(contract_1.name, contract_2.name);
    assert_eq!(contract_1.types.len(), contract_2.types.len());
    assert_eq!(contract_1.services.len(), contract_2.services.len());

    let diff_report = contract::diff::diff(&contract_1, &contract_2);
    assert!(
        diff_report.is_compatible(),
        "Exported OpenAPI should be backwards-compatible with original: {:?}",
        diff_report.breaking_changes()
    );
}
