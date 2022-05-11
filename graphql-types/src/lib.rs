use std::collections::HashMap;

#[derive(Clone, PartialEq, prost::Message)]
pub struct GraphqlRequest {
    #[prost(string, tag="1")]
    pub query: String,
    #[prost(string, optional, tag="2")]
    pub operation: Option<String>,
    #[prost(map="string, message", tag="3")]
    pub variables: HashMap<String, ConstValue>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GraphqlResponse {
    #[prost(message, optional, tag="1")]
    pub data: Option<ConstValue>,
    #[prost(message, repeated, tag="2")]
    pub errors: Vec<ServerError>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct ServerError {
    #[prost(string, tag="1")]
    pub message: String,
    #[prost(message, repeated, tag="2")]
    pub path: Vec<ConstValue>,
    #[prost(message, repeated, tag="3")]
    pub locations: Vec<Pos>,
    #[prost(map="string, message", tag="4")]
    pub extensions: HashMap<String, ConstValue>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct ConstValue {
    #[prost(message, optional, tag="1")]
    pub value: Option<Value>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct Value {
    #[prost(oneof="value::Value", tags="1, 2, 3, 4, 5, 6, 7")]
    pub value: Option<value::Value>,
}
/// Nested message and enum types in `Value`.
pub mod value {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Value {
        #[prost(message, tag="1")]
        Number(super::Number),
        #[prost(string, tag="2")]
        String(String),
        #[prost(bool, tag="3")]
        Boolean(bool),
        #[prost(bytes, tag="4")]
        Binary(Vec<u8>),
        #[prost(string, tag="5")]
        Enum(String),
        #[prost(message, tag="6")]
        List(super::ListConstValue),
        #[prost(message, tag="7")]
        Object(super::IndexMap),
    }
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct IndexMap {
    #[prost(map="string, message", tag="1")]
    pub map: HashMap<String, ConstValue>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct ListConstValue {
    #[prost(message, repeated, tag="1")]
    pub list: Vec<ConstValue>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct Number {
    #[prost(oneof="number::N", tags="1, 2, 3")]
    pub n: Option<number::N>,
}
/// Nested message and enum types in `Number`.
pub mod number {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum N {
        #[prost(uint64, tag="1")]
        PosInt(u64),
        #[prost(int64, tag="2")]
        NegInt(i64),
        #[prost(float, tag="3")]
        Float(f32),
    }
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct Pos {
    #[prost(uint64, tag="1")]
    pub line: u64,
    #[prost(uint64, tag="2")]
    pub column: u64,
}