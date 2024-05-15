use crate::planner::IntrospectionSelectionSet;
use crate::schema::ComposedSchema;
use parser::types::OperationType;
use value::ConstValue;

use super::r#type::IntrospectionType;
use super::resolver::{resolve_obj, Resolver};
use super::schema::IntrospectionSchema;

pub struct IntrospectionRoot {
    pub(crate) kind: RootKind
}

#[derive(Clone, Copy)]
pub enum RootKind {
    Query,
    Mutation,
    Subscription
}

impl From<OperationType> for RootKind {
     fn from(value: OperationType) -> Self {
         match value {
            OperationType::Mutation => Self::Mutation,
            OperationType::Query => Self::Query,
            OperationType::Subscription => Self::Subscription,
         }
     }
}

impl Resolver for IntrospectionRoot {
    fn resolve(
        &self,
        selection_set: &IntrospectionSelectionSet,
        schema: &ComposedSchema,
    ) -> ConstValue {
        resolve_obj(selection_set, |name, field| match name {
            "__schema" => IntrospectionSchema.resolve(&field.selection_set, schema),
            "__type" => {
                if let Some(ConstValue::String(name)) = field.arguments.get("name") {
                    if let Some(ty) = schema.types.get(name.as_str()) {
                        return IntrospectionType::Named(ty).resolve(&field.selection_set, schema);
                    }
                }
                ConstValue::Null
            },
            "__typename" => match self.kind {
                RootKind::Query => ConstValue::String(format!("Query")),
                RootKind::Mutation => ConstValue::String(format!("Mutation")),
                RootKind::Subscription => ConstValue::String(format!("Subscription")),
            },
            _ => ConstValue::Null,
        })
    }
}
