use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use value::{ConstValue, Variables};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub data: RequestData,
    pub headers: HashMap<String, String>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestData {
    pub query: String,
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "variables_is_empty", default)]
    pub variables: Variables,
}

impl RequestData {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            operation: None,
            variables: Default::default(),
        }
    }

    pub fn operation(self, operation: impl Into<String>) -> Self {
        Self {
            operation: Some(operation.into()),
            ..self
        }
    }

    pub fn variables(self, variables: Variables) -> Self {
        Self { variables, ..self }
    }

    pub fn extend_variables(mut self, variables: Variables) -> Self {
        if let ConstValue::Object(obj) = variables.into_value() {
            self.variables.extend(obj);
        }
        self
    }
}

#[inline]
fn variables_is_empty(variables: &Variables) -> bool {
    variables.is_empty()
}
