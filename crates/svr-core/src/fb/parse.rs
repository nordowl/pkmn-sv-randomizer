use std::sync::Arc;

use thiserror::Error;

use super::schema::{ElementType, FieldType, ObjectId, ScalarType, Schema};
use super::value::{StructValue, TableValue, Value};
use super::wire::{Buf, WireError};

/// Deeper than any real schema nests; stops a crafted chain of tables overflowing the stack.
const MAX_DEPTH: usize = 64;

/// Bounds decoded values relative to input size. Uoffsets may alias, so a small buffer could
/// otherwise expand into an exponentially large tree. Real tables decode to well under one value
/// per input byte.
const BUDGET_PER_INPUT_BYTE: usize = 4;
const BUDGET_SLACK: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error("buffer identifier {found:?} does not match the schema's {expected:?}")]
    IdentifierMismatch { expected: [u8; 4], found: [u8; 4] },
    #[error("{object} has data in vtable slot {slot}, which the schema does not define")]
    UnmappedField { object: String, slot: usize },
    #[error("tables are nested deeper than {MAX_DEPTH} levels")]
    TooDeep,
    #[error("buffer decodes to more values than its size allows")]
    TooManyValues,
}

pub fn parse(schema: &Arc<Schema>, bytes: &[u8]) -> Result<TableValue, ParseError> {
    let buf = Buf(bytes);
    if let Some(expected) = schema.file_ident() {
        let found = buf.array::<4>(4)?;
        if found != expected {
            return Err(ParseError::IdentifierMismatch { expected, found });
        }
    }
    let budget = bytes.len().saturating_mul(BUDGET_PER_INPUT_BYTE).saturating_add(BUDGET_SLACK);
    let mut parser = Parser { schema, buf, budget };
    parser.table(schema.root_id(), buf.offset_target(0)?, 0)
}

struct Parser<'a> {
    schema: &'a Arc<Schema>,
    buf: Buf<'a>,
    budget: usize,
}

impl Parser<'_> {
    fn charge(&mut self, amount: usize) -> Result<(), ParseError> {
        self.budget = self.budget.checked_sub(amount).ok_or(ParseError::TooManyValues)?;
        Ok(())
    }

    fn table(&mut self, object: ObjectId, pos: usize, depth: usize) -> Result<TableValue, ParseError> {
        if depth > MAX_DEPTH {
            return Err(ParseError::TooDeep);
        }
        self.charge(1)?;
        let schema = self.schema;
        let def = schema.object_at(object);
        let table = self.buf.table(pos)?;
        // Keeping only known slots would silently drop this data on the next serialize.
        for slot in def.fields.len()..table.slot_count() {
            if table.field(slot)?.is_some() {
                return Err(ParseError::UnmappedField { object: def.name.clone(), slot });
            }
        }
        let mut fields = Vec::with_capacity(def.fields.len());
        for (slot, field) in def.fields.iter().enumerate() {
            let value = match table.field(slot)? {
                Some(field_pos) => Some(self.field(field.ty, field_pos, depth)?),
                None => None,
            };
            fields.push(value);
        }
        Ok(TableValue { schema: Arc::clone(schema), object, fields })
    }

    fn field(&mut self, ty: FieldType, pos: usize, depth: usize) -> Result<Value, ParseError> {
        match ty {
            FieldType::Single(element) => self.element(element, pos, depth),
            FieldType::Vector(element) => {
                let (size, _) = self.schema.element_layout(element);
                let (data, count) = self.buf.vector(self.buf.offset_target(pos)?, size)?;
                self.charge(count)?;
                Ok(Value::Vector(self.elements(element, data, size, count, depth)?))
            }
            FieldType::Array(element, len) => {
                let (size, _) = self.schema.element_layout(element);
                self.charge(usize::from(len))?;
                Ok(Value::Array(self.elements(element, pos, size, usize::from(len), depth)?))
            }
        }
    }

    fn elements(
        &mut self,
        element: ElementType,
        start: usize,
        size: usize,
        count: usize,
        depth: usize,
    ) -> Result<Vec<Value>, ParseError> {
        let mut items = Vec::with_capacity(count);
        for index in 0..count {
            items.push(self.element(element, start + index * size, depth)?);
        }
        Ok(items)
    }

    fn element(&mut self, element: ElementType, pos: usize, depth: usize) -> Result<Value, ParseError> {
        match element {
            ElementType::Scalar(ty, _) => Ok(self.scalar(ty, pos)?),
            ElementType::Str => {
                let text = self.buf.string(self.buf.offset_target(pos)?)?;
                self.charge(text.len())?;
                Ok(Value::Str(text.to_owned()))
            }
            ElementType::Table(object) => {
                let target = self.buf.offset_target(pos)?;
                Ok(Value::Table(self.table(object, target, depth + 1)?))
            }
            ElementType::Struct(object) => Ok(Value::Struct(self.structure(object, pos)?)),
        }
    }

    fn structure(&mut self, object: ObjectId, pos: usize) -> Result<StructValue, ParseError> {
        let schema = self.schema;
        let def = schema.object_at(object);
        self.charge(def.fields.len())?;
        let mut fields = Vec::with_capacity(def.fields.len());
        for field in &def.fields {
            fields.push(self.field(field.ty, pos + field.offset as usize, 0)?);
        }
        Ok(StructValue { schema: Arc::clone(schema), object, fields })
    }

    fn scalar(&self, ty: ScalarType, pos: usize) -> Result<Value, WireError> {
        let buf = self.buf;
        Ok(match ty {
            ScalarType::Bool => Value::Bool(buf.array::<1>(pos)?[0] != 0),
            ScalarType::I8 => Value::Int(i8::from_le_bytes(buf.array(pos)?).into()),
            ScalarType::U8 => Value::UInt(u8::from_le_bytes(buf.array(pos)?).into()),
            ScalarType::I16 => Value::Int(i16::from_le_bytes(buf.array(pos)?).into()),
            ScalarType::U16 => Value::UInt(u16::from_le_bytes(buf.array(pos)?).into()),
            ScalarType::I32 => Value::Int(i32::from_le_bytes(buf.array(pos)?).into()),
            ScalarType::U32 => Value::UInt(u32::from_le_bytes(buf.array(pos)?).into()),
            ScalarType::I64 => Value::Int(i64::from_le_bytes(buf.array(pos)?)),
            ScalarType::U64 => Value::UInt(u64::from_le_bytes(buf.array(pos)?)),
            ScalarType::F32 => Value::Float(f32::from_le_bytes(buf.array(pos)?).into()),
            ScalarType::F64 => Value::Float(f64::from_le_bytes(buf.array(pos)?)),
        })
    }
}
