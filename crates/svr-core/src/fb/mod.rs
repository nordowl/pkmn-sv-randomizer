//! Schema-driven FlatBuffers reader and writer.
//!
//! A [`Schema`] comes from a game `.bfbs` or a hand-written `.fbs`. [`parse`] turns a table into a
//! [`TableValue`] tree, code mutates it by field name, and [`serialize`] writes a valid buffer back.

pub mod builtin;
mod fbs_text;
mod parse;
mod reflection;
mod schema;
mod value;
mod wire;
mod write;

pub use parse::{parse, ParseError};
pub use schema::{
    ElementType, EnumDef, EnumId, FieldDef, FieldType, ObjectDef, ObjectId, ObjectKind, ScalarType, Schema, SchemaError,
};
pub use value::{FieldError, StructValue, TableValue, Value};
pub use wire::WireError;
pub use write::{serialize, try_serialize, SerializeError};
