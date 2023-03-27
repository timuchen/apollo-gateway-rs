use thiserror::Error;

#[derive(Debug, Error)]
pub enum CombineError {
    #[error("Type '{type_name}' definition conflicted.")]
    DefinitionConflicted { type_name: String },

    #[error("Field '{type_name}.{field_name}' definition conflicted.")]
    FieldConflicted {
        type_name: String,
        field_name: String,
    },
}
