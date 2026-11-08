use contract::adapters::{default_registry, from_graphql};
use contract::ir::Type;
use std::path::Path;

#[test]
fn test_parse_graphql_sdl() {
    let gql_src = r#"
# System user role
enum UserRole {
  ADMIN
  MEMBER
  GUEST
}

# User account model
type User {
  id: ID!
  name: String!
  email: String
  role: UserRole!
  tags: [String!]!
}

type Query {
  getUser(id: ID!): User
}

type Mutation {
  createUser(name: String!): User
}
"#;

    let contract = from_graphql(gql_src).expect("Should parse GraphQL SDL");

    assert_eq!(contract.types.len(), 2);
    assert_eq!(contract.services.len(), 1);

    // Verify Enum
    let role_type = contract
        .types
        .iter()
        .find(|t| t.name() == "UserRole")
        .expect("UserRole enum exists");
    if let contract::ir::TypeDefinition::Enum(e) = role_type {
        assert_eq!(e.variants.len(), 3);
        assert_eq!(e.variants[0].name, "ADMIN");
    } else {
        panic!("Expected EnumType");
    }

    // Verify Record
    let user_type = contract
        .types
        .iter()
        .find(|t| t.name() == "User")
        .expect("User record exists");
    if let contract::ir::TypeDefinition::Record(r) = user_type {
        let id_field = r.fields.iter().find(|f| f.name == "id").unwrap();
        assert_eq!(id_field.field_type, Type::String); // ID! is required String

        let email_field = r.fields.iter().find(|f| f.name == "email").unwrap();
        assert_eq!(
            email_field.field_type,
            Type::Optional(Box::new(Type::String)) // String without ! is Optional
        );

        let role_field = r.fields.iter().find(|f| f.name == "role").unwrap();
        assert_eq!(
            role_field.field_type,
            Type::Reference("UserRole".to_string())
        );

        let tags_field = r.fields.iter().find(|f| f.name == "tags").unwrap();
        assert_eq!(tags_field.field_type, Type::List(Box::new(Type::String)));
    } else {
        panic!("Expected Record");
    }

    // Verify Service & Operations
    let svc = &contract.services[0];
    assert_eq!(svc.name, "GraphQLApi");
    assert_eq!(svc.operations.len(), 2);
    assert!(svc.operations.iter().any(|op| op.name == "getUser"));
    assert!(svc.operations.iter().any(|op| op.name == "createUser"));

    // Verify registry auto-detection
    let registry = default_registry();
    let adapter = registry
        .find_adapter(Some(Path::new("schema.graphql")), gql_src)
        .expect("Registry should find graphql adapter");
    assert_eq!(adapter.name(), "graphql");
}
