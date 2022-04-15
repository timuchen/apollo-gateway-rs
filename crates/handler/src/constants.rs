use opentelemetry::Key;

pub const KEY_SERVICE: Key = Key::from_static_str("graphql-gateway.service");
pub const KEY_QUERY: Key = Key::from_static_str("graphql-gateway.query");
pub const KEY_PATH: Key = Key::from_static_str("graphql-gateway.path");
pub const KEY_PARENT_TYPE: Key = Key::from_static_str("graphql-gateway.parentType");
pub const KEY_RETURN_TYPE: Key = Key::from_static_str("graphql-gateway.returnType");
pub const KEY_FIELD_NAME: Key = Key::from_static_str("graphql-gateway.fieldName");
pub const KEY_VARIABLES: Key = Key::from_static_str("graphql-gateway.variables");
pub const KEY_ERROR: Key = Key::from_static_str("graphql-gateway.error");
