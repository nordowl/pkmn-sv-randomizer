use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;

use super::schema::{ElementType, FieldType, ObjectId, ScalarType, Schema};
use super::value::{StructValue, TableValue, Value};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SerializeError {
    #[error("{object}.{field} holds a value that does not match its schema type {expected}")]
    TypeMismatch { object: String, field: String, expected: String },
    #[error("{object}.{field} holds {value}, which does not fit {expected}")]
    OutOfRange { object: String, field: String, expected: String, value: String },
    #[error("{object}.{field} is required but absent")]
    MissingRequired { object: String, field: String },
    #[error("{object} has an inline size of {size} bytes, more than a vtable can address")]
    TableTooLarge { object: String, size: usize },
    #[error("the buffer outgrew what a 32-bit offset can address")]
    BufferTooLarge,
}

/// Panics when the tree breaks its schema: a wrong `Value` kind pushed through `vector_mut` or
/// `array_mut`, or a required field left absent. Use `try_serialize` to handle those as errors.
pub fn serialize(root: &TableValue) -> Vec<u8> {
    match try_serialize(root) {
        Ok(bytes) => bytes,
        Err(error) => panic!("cannot serialize table: {error}"),
    }
}

pub fn try_serialize(root: &TableValue) -> Result<Vec<u8>, SerializeError> {
    let schema = root.schema();
    let mut writer = Writer { schema, out: vec![0; 4], vtables: HashMap::new(), max_align: 4 };
    if let Some(ident) = schema.file_ident() {
        writer.out.extend_from_slice(&ident);
    }
    let table_pos = writer.table(root)?;
    writer.patch_u32(0, table_pos)?;
    writer.pad_for(writer.max_align, 0);
    Ok(writer.out)
}

/// A present table field and where it landed inside the table.
struct Placed<'t> {
    slot: usize,
    value: &'t Value,
    size: usize,
    align: usize,
    offset: usize,
}

/// A uoffset already written as a placeholder, waiting for its target to be appended.
enum Pending<'t> {
    Str(&'t str),
    Table(&'t TableValue),
    Vector(Site<'t>, ElementType, &'t [Value]),
}

/// Writes front to back so that every child lands after the uoffset pointing at it. That makes
/// uoffsets forward and positive by construction, and alignment relative to the buffer start.
struct Writer<'t> {
    schema: &'t Arc<Schema>,
    out: Vec<u8>,
    vtables: HashMap<Vec<u8>, usize>,
    max_align: usize,
}

/// Names the field being written, for error messages.
#[derive(Clone, Copy)]
struct Site<'t> {
    object: &'t str,
    field: &'t str,
}

impl Site<'_> {
    fn mismatch(&self, expected: impl Into<String>) -> SerializeError {
        SerializeError::TypeMismatch {
            object: self.object.to_owned(),
            field: self.field.to_owned(),
            expected: expected.into(),
        }
    }

    fn out_of_range(&self, ty: ScalarType, value: impl ToString) -> SerializeError {
        SerializeError::OutOfRange {
            object: self.object.to_owned(),
            field: self.field.to_owned(),
            expected: ty.name().to_owned(),
            value: value.to_string(),
        }
    }
}

impl<'t> Writer<'t> {
    fn pad_for(&mut self, align: usize, prefix: usize) {
        self.max_align = self.max_align.max(align);
        while (self.out.len() + prefix) % align != 0 {
            self.out.push(0);
        }
    }

    fn reserve(&mut self, len: usize) -> usize {
        let pos = self.out.len();
        self.out.resize(pos + len, 0);
        pos
    }

    fn put(&mut self, pos: usize, bytes: &[u8]) {
        self.out[pos..pos + bytes.len()].copy_from_slice(bytes);
    }

    fn patch_u32(&mut self, pos: usize, value: usize) -> Result<(), SerializeError> {
        let value = u32::try_from(value).map_err(|_| SerializeError::BufferTooLarge)?;
        self.put(pos, &value.to_le_bytes());
        Ok(())
    }

    fn vtable(&mut self, entries: &[u16], object_size: u16) -> usize {
        let mut bytes = Vec::with_capacity(4 + 2 * entries.len());
        bytes.extend_from_slice(&((4 + 2 * entries.len()) as u16).to_le_bytes());
        bytes.extend_from_slice(&object_size.to_le_bytes());
        for entry in entries {
            bytes.extend_from_slice(&entry.to_le_bytes());
        }
        if let Some(&pos) = self.vtables.get(&bytes) {
            return pos;
        }
        self.pad_for(2, 0);
        let pos = self.out.len();
        self.out.extend_from_slice(&bytes);
        self.vtables.insert(bytes, pos);
        pos
    }

    fn table(&mut self, table: &'t TableValue) -> Result<usize, SerializeError> {
        let schema = self.schema;
        let def = table.def();
        if let Some((field, _)) = table.fields().find(|(field, value)| field.required && value.is_none()) {
            return Err(SerializeError::MissingRequired { object: def.name.clone(), field: field.name.clone() });
        }

        // Ascending alignment keeps padding to the few bytes needed between size classes.
        let mut placed: Vec<Placed<'t>> = table
            .fields
            .iter()
            .enumerate()
            .filter_map(|(slot, value)| {
                let (size, align) = schema.field_layout(def.fields[slot].ty);
                value.as_ref().map(|value| Placed { slot, value, size, align, offset: 0 })
            })
            .collect();
        placed.sort_by_key(|p| (p.align, p.slot));

        let mut object_size = 4usize;
        let mut table_align = 4;
        let mut entries = vec![0u16; placed.iter().map(|p| p.slot + 1).max().unwrap_or(0)];
        for p in &mut placed {
            object_size = object_size.next_multiple_of(p.align);
            p.offset = object_size;
            object_size += p.size;
            table_align = table_align.max(p.align);
            entries[p.slot] = p.offset as u16;
        }
        // Offsets only grow, so a total that fits proves the truncating casts above were exact.
        let object_size_u16 = u16::try_from(object_size)
            .map_err(|_| SerializeError::TableTooLarge { object: def.name.clone(), size: object_size })?;

        let vtable_pos = self.vtable(&entries, object_size_u16);
        self.pad_for(table_align, 0);
        let table_pos = self.reserve(object_size);
        let soffset = i32::try_from(table_pos - vtable_pos).map_err(|_| SerializeError::BufferTooLarge)?;
        self.put(table_pos, &soffset.to_le_bytes());

        let mut pending = Vec::new();
        for p in &placed {
            let field = &def.fields[p.slot];
            let site = Site { object: &def.name, field: &field.name };
            self.field(site, field.ty, p.value, table_pos + p.offset, &mut pending)?;
        }
        self.flush(pending)?;
        Ok(table_pos)
    }

    fn field(
        &mut self,
        site: Site<'t>,
        ty: FieldType,
        value: &'t Value,
        pos: usize,
        pending: &mut Vec<(usize, Pending<'t>)>,
    ) -> Result<(), SerializeError> {
        match (ty, value) {
            (FieldType::Single(element), _) => self.element(site, element, value, pos, pending),
            (FieldType::Vector(element), Value::Vector(items)) => {
                pending.push((pos, Pending::Vector(site, element, items)));
                Ok(())
            }
            (FieldType::Array(element, len), Value::Array(items)) if items.len() == usize::from(len) => {
                let (size, _) = self.schema.element_layout(element);
                for (index, item) in items.iter().enumerate() {
                    self.element(site, element, item, pos + index * size, pending)?;
                }
                Ok(())
            }
            (FieldType::Vector(_), _) => Err(site.mismatch("a vector")),
            (FieldType::Array(_, len), _) => Err(site.mismatch(format!("an array of {len}"))),
        }
    }

    fn element(
        &mut self,
        site: Site<'t>,
        element: ElementType,
        value: &'t Value,
        pos: usize,
        pending: &mut Vec<(usize, Pending<'t>)>,
    ) -> Result<(), SerializeError> {
        match (element, value) {
            (ElementType::Scalar(ty, _), _) => self.scalar(site, ty, value, pos),
            (ElementType::Str, Value::Str(text)) => {
                pending.push((pos, Pending::Str(text)));
                Ok(())
            }
            (ElementType::Table(object), Value::Table(table)) => {
                self.check_object(site, object, table.schema(), table.object)?;
                pending.push((pos, Pending::Table(table)));
                Ok(())
            }
            (ElementType::Struct(object), Value::Struct(value)) => {
                self.check_object(site, object, value.schema(), value.object)?;
                self.structure(value, pos, pending)
            }
            (ElementType::Str, _) => Err(site.mismatch("a string")),
            (ElementType::Table(_), _) => Err(site.mismatch("a table")),
            (ElementType::Struct(_), _) => Err(site.mismatch("a struct")),
        }
    }

    fn check_object(
        &self,
        site: Site,
        expected: ObjectId,
        schema: &Arc<Schema>,
        actual: ObjectId,
    ) -> Result<(), SerializeError> {
        if Arc::ptr_eq(self.schema, schema) && expected == actual {
            Ok(())
        } else {
            Err(site.mismatch(self.schema.object_at(expected).name.clone()))
        }
    }

    fn structure(
        &mut self,
        value: &'t StructValue,
        pos: usize,
        pending: &mut Vec<(usize, Pending<'t>)>,
    ) -> Result<(), SerializeError> {
        let def = value.def();
        for (field, item) in def.fields.iter().zip(&value.fields) {
            let site = Site { object: &def.name, field: &field.name };
            self.field(site, field.ty, item, pos + field.offset as usize, pending)?;
        }
        Ok(())
    }

    fn scalar(&mut self, site: Site, ty: ScalarType, value: &Value, pos: usize) -> Result<(), SerializeError> {
        let bytes: Vec<u8> = match (ty, value) {
            (ScalarType::Bool, Value::Bool(v)) => vec![u8::from(*v)],
            (ScalarType::F32, Value::Float(v)) => (*v as f32).to_le_bytes().to_vec(),
            (ScalarType::F64, Value::Float(v)) => v.to_le_bytes().to_vec(),
            (_, Value::Int(v)) if ty.int_value(0).is_some() => {
                ty.int_value(i128::from(*v)).ok_or_else(|| site.out_of_range(ty, v))?;
                v.to_le_bytes()[..ty.size()].to_vec()
            }
            (_, Value::UInt(v)) if ty.int_value(0).is_some() => {
                ty.int_value(i128::from(*v)).ok_or_else(|| site.out_of_range(ty, v))?;
                v.to_le_bytes()[..ty.size()].to_vec()
            }
            _ => return Err(site.mismatch(ty.name())),
        };
        self.put(pos, &bytes);
        Ok(())
    }

    /// Appends every waiting target and points its uoffset at it. `pending` is in field order, which
    /// together with `table`'s sort is what makes the output deterministic.
    fn flush(&mut self, pending: Vec<(usize, Pending<'t>)>) -> Result<(), SerializeError> {
        for (offset_pos, target) in pending {
            let target_pos = match target {
                Pending::Str(text) => self.string(text)?,
                Pending::Table(table) => self.table(table)?,
                Pending::Vector(site, element, items) => self.vector(site, element, items)?,
            };
            self.patch_u32(offset_pos, target_pos - offset_pos)?;
        }
        Ok(())
    }

    fn string(&mut self, text: &str) -> Result<usize, SerializeError> {
        self.pad_for(4, 0);
        let pos = self.reserve(4);
        self.patch_u32(pos, text.len())?;
        self.out.extend_from_slice(text.as_bytes());
        self.out.push(0);
        Ok(pos)
    }

    fn vector(&mut self, site: Site<'t>, element: ElementType, items: &'t [Value]) -> Result<usize, SerializeError> {
        let (size, align) = self.schema.element_layout(element);
        // The length prefix sits directly before the data, so the data's alignment places it.
        self.pad_for(align.max(4), 4);
        let pos = self.reserve(4 + size * items.len());
        self.patch_u32(pos, items.len())?;

        let mut pending = Vec::new();
        for (index, item) in items.iter().enumerate() {
            self.element(site, element, item, pos + 4 + index * size, &mut pending)?;
        }
        self.flush(pending)?;
        Ok(pos)
    }
}
