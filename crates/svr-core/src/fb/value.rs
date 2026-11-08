use std::fmt;
use std::sync::Arc;

use thiserror::Error;

use super::schema::{ElementType, FieldDef, FieldType, ObjectDef, ObjectId, ScalarType, Schema};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FieldError {
    #[error("{object} has no field named {field}")]
    Unknown { object: String, field: String },
    #[error("{object}.{field} is not {requested}")]
    TypeMismatch { object: String, field: String, requested: &'static str },
    #[error("{value} is out of range for {object}.{field}")]
    OutOfRange { object: String, field: String, value: i128 },
}

#[derive(Debug, Clone)]
pub enum Value {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Str(String),
    Vector(Vec<Value>),
    Table(TableValue),
    Struct(StructValue),
    Array(Vec<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::UInt(a), Value::UInt(b)) => a == b,
            // By bits, so a NaN compares equal to itself and -0.0 differs from 0.0 across a round trip.
            (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Vector(a), Value::Vector(b)) | (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Table(a), Value::Table(b)) => a == b,
            (Value::Struct(a), Value::Struct(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    pub fn as_int(&self) -> Option<i64> {
        match *self {
            Value::Int(v) => Some(v),
            Value::UInt(v) => i64::try_from(v).ok(),
            _ => None,
        }
    }

    pub fn as_uint(&self) -> Option<u64> {
        match *self {
            Value::Int(v) => u64::try_from(v).ok(),
            Value::UInt(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match *self {
            Value::Float(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&TableValue> {
        match self {
            Value::Table(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_table_mut(&mut self) -> Option<&mut TableValue> {
        match self {
            Value::Table(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&StructValue> {
        match self {
            Value::Struct(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_struct_mut(&mut self) -> Option<&mut StructValue> {
        match self {
            Value::Struct(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_slice(&self) -> Option<&[Value]> {
        match self {
            Value::Vector(v) | Value::Array(v) => Some(v),
            _ => None,
        }
    }

    fn scalar_int(&self, def: &ObjectDef, field: &FieldDef) -> Result<i64, FieldError> {
        match *self {
            Value::Int(v) => Ok(v),
            Value::UInt(v) => {
                i64::try_from(v).map_err(|_| out_of_range(def, field, i128::from(v)))
            }
            _ => Err(mismatch(def, field, "an integer")),
        }
    }

    fn scalar_uint(&self, def: &ObjectDef, field: &FieldDef) -> Result<u64, FieldError> {
        match *self {
            Value::UInt(v) => Ok(v),
            Value::Int(v) => {
                u64::try_from(v).map_err(|_| out_of_range(def, field, i128::from(v)))
            }
            _ => Err(mismatch(def, field, "an integer")),
        }
    }
}

fn mismatch(def: &ObjectDef, field: &FieldDef, requested: &'static str) -> FieldError {
    FieldError::TypeMismatch { object: def.name.clone(), field: field.name.clone(), requested }
}

fn out_of_range(def: &ObjectDef, field: &FieldDef, value: i128) -> FieldError {
    FieldError::OutOfRange { object: def.name.clone(), field: field.name.clone(), value }
}

/// For a slot whose stored `Value` disagrees with the schema, which takes a hand-built tree.
fn stored_mismatch(schema: &Schema, object: ObjectId, field: &str, requested: &'static str) -> FieldError {
    FieldError::TypeMismatch { object: schema.object_at(object).name.clone(), field: field.to_owned(), requested }
}

fn unknown(def: &ObjectDef, field: &str) -> FieldError {
    FieldError::Unknown { object: def.name.clone(), field: field.to_owned() }
}

fn int_value(def: &ObjectDef, field: &FieldDef, v: i128) -> Result<Value, FieldError> {
    let FieldType::Single(ElementType::Scalar(ty, _)) = field.ty else {
        return Err(mismatch(def, field, "an integer"));
    };
    if ty.int_value(0).is_none() {
        return Err(mismatch(def, field, "an integer"));
    }
    ty.int_value(v).ok_or_else(|| out_of_range(def, field, v))
}

fn scalar_type_is(field: &FieldDef, test: impl Fn(ScalarType) -> bool) -> bool {
    matches!(field.ty, FieldType::Single(ElementType::Scalar(ty, _)) if test(ty))
}

/// One table row: one `Option<Value>` per schema field, indexed by vtable slot. `None` means the
/// field is absent from the buffer, which is distinct from present-and-equal-to-default.
#[derive(Clone)]
pub struct TableValue {
    pub(super) schema: Arc<Schema>,
    pub(super) object: ObjectId,
    pub(super) fields: Vec<Option<Value>>,
}

/// One inline struct. Every field is always present.
#[derive(Clone)]
pub struct StructValue {
    pub(super) schema: Arc<Schema>,
    pub(super) object: ObjectId,
    pub(super) fields: Vec<Value>,
}

macro_rules! scalar_accessors {
    ($ty:ty) => {
        impl $ty {
            pub fn schema(&self) -> &Arc<Schema> {
                &self.schema
            }

            pub fn def(&self) -> &ObjectDef {
                self.schema.object_at(self.object)
            }

            fn field_def(&self, field: &str) -> Result<(usize, &FieldDef), FieldError> {
                let def = self.def();
                let index = def.field_index(field).ok_or_else(|| unknown(def, field))?;
                Ok((index, &def.fields[index]))
            }

            /// Absent table fields read as their schema default.
            pub fn int(&self, field: &str) -> Result<i64, FieldError> {
                let (def, value) = self.scalar(field)?;
                value.ok_or_else(|| mismatch(self.def(), def, "an integer"))?.scalar_int(self.def(), def)
            }

            /// Absent table fields read as their schema default. Use this for `u64` hash fields,
            /// which do not always fit `int`.
            pub fn uint(&self, field: &str) -> Result<u64, FieldError> {
                let (def, value) = self.scalar(field)?;
                value.ok_or_else(|| mismatch(self.def(), def, "an integer"))?.scalar_uint(self.def(), def)
            }

            pub fn float(&self, field: &str) -> Result<f64, FieldError> {
                let (def, value) = self.scalar(field)?;
                value.and_then(Value::as_float).ok_or_else(|| mismatch(self.def(), def, "a float"))
            }

            pub fn bool(&self, field: &str) -> Result<bool, FieldError> {
                let (def, value) = self.scalar(field)?;
                value.and_then(Value::as_bool).ok_or_else(|| mismatch(self.def(), def, "a bool"))
            }

            pub fn set_int(&mut self, field: &str, v: i64) -> Result<(), FieldError> {
                self.set_integer(field, i128::from(v))
            }

            pub fn set_uint(&mut self, field: &str, v: u64) -> Result<(), FieldError> {
                self.set_integer(field, i128::from(v))
            }

            fn set_integer(&mut self, field: &str, v: i128) -> Result<(), FieldError> {
                let (index, def) = self.field_def(field)?;
                let value = int_value(self.def(), def, v)?;
                self.store(index, value);
                Ok(())
            }

            pub fn set_float(&mut self, field: &str, v: f64) -> Result<(), FieldError> {
                let (index, def) = self.field_def(field)?;
                if !scalar_type_is(def, |ty| matches!(ty, ScalarType::F32 | ScalarType::F64)) {
                    return Err(mismatch(self.def(), def, "a float"));
                }
                self.store(index, Value::Float(v));
                Ok(())
            }

            pub fn set_bool(&mut self, field: &str, v: bool) -> Result<(), FieldError> {
                let (index, def) = self.field_def(field)?;
                if !scalar_type_is(def, |ty| ty == ScalarType::Bool) {
                    return Err(mismatch(self.def(), def, "a bool"));
                }
                self.store(index, Value::Bool(v));
                Ok(())
            }

            /// Returns a zeroed struct of the type stored in `field`, ready to place in it or to
            /// push onto its vector.
            pub fn new_struct(&self, field: &str) -> Result<StructValue, FieldError> {
                let (_, def) = self.field_def(field)?;
                let (FieldType::Single(ElementType::Struct(object))
                | FieldType::Vector(ElementType::Struct(object))
                | FieldType::Array(ElementType::Struct(object), _)) = def.ty
                else {
                    return Err(mismatch(self.def(), def, "a struct"));
                };
                Ok(StructValue::zeroed(&self.schema, object))
            }
        }

        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut out = f.debug_struct(&self.def().name);
                for (def, value) in self.fields() {
                    out.field(&def.name, &value);
                }
                out.finish()
            }
        }
    };
}

scalar_accessors!(TableValue);
scalar_accessors!(StructValue);

impl PartialEq for TableValue {
    fn eq(&self, other: &Self) -> bool {
        self.def().name == other.def().name && self.fields == other.fields
    }
}

impl PartialEq for StructValue {
    fn eq(&self, other: &Self) -> bool {
        self.def().name == other.def().name && self.fields == other.fields
    }
}

impl TableValue {
    pub(super) fn empty(schema: &Arc<Schema>, object: ObjectId) -> Self {
        let count = schema.object_at(object).fields.len();
        Self { schema: Arc::clone(schema), object, fields: vec![None; count] }
    }

    fn scalar(&self, field: &str) -> Result<(&FieldDef, Option<&Value>), FieldError> {
        let (index, def) = self.field_def(field)?;
        Ok((def, self.fields[index].as_ref().or(def.default.as_ref())))
    }

    fn store(&mut self, index: usize, value: Value) {
        self.fields[index] = Some(value);
    }

    /// Iterates all schema fields in slot order, yielding `None` for absent ones.
    pub fn fields(&self) -> impl Iterator<Item = (&FieldDef, Option<&Value>)> {
        self.def().fields.iter().zip(self.fields.iter().map(Option::as_ref))
    }

    /// Returns `None` for both an absent field and an unknown name; `int` and `set_*` tell them apart.
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields[self.def().field_index(field)?].as_ref()
    }

    pub fn string(&self, field: &str) -> Option<&str> {
        self.get(field)?.as_str()
    }

    pub fn set_string(&mut self, field: &str, v: impl Into<String>) -> Result<(), FieldError> {
        let (index, def) = self.field_def(field)?;
        if def.ty != FieldType::Single(ElementType::Str) {
            return Err(mismatch(self.def(), def, "a string"));
        }
        self.store(index, Value::Str(v.into()));
        Ok(())
    }

    pub fn table(&self, field: &str) -> Option<&TableValue> {
        self.get(field)?.as_table()
    }

    /// Creates an empty sub-table when the field is absent.
    pub fn table_mut(&mut self, field: &str) -> Result<&mut TableValue, FieldError> {
        let (index, def) = self.field_def(field)?;
        let FieldType::Single(ElementType::Table(object)) = def.ty else {
            return Err(mismatch(self.def(), def, "a table"));
        };
        if self.fields[index].is_none() {
            let created = TableValue::empty(&self.schema, object);
            self.fields[index] = Some(Value::Table(created));
        }
        match &mut self.fields[index] {
            Some(Value::Table(table)) => Ok(table),
            _ => Err(stored_mismatch(&self.schema, self.object, field, "a table")),
        }
    }

    pub fn vector(&self, field: &str) -> Option<&[Value]> {
        match self.get(field)? {
            Value::Vector(items) => Some(items),
            _ => None,
        }
    }

    /// Creates an empty vector when the field is absent.
    pub fn vector_mut(&mut self, field: &str) -> Result<&mut Vec<Value>, FieldError> {
        let (index, def) = self.field_def(field)?;
        if !matches!(def.ty, FieldType::Vector(_)) {
            return Err(mismatch(self.def(), def, "a vector"));
        }
        let slot = self.fields[index].get_or_insert_with(|| Value::Vector(Vec::new()));
        match slot {
            Value::Vector(items) => Ok(items),
            _ => Err(stored_mismatch(&self.schema, self.object, field, "a vector")),
        }
    }

    /// Iterates a vector of tables. An absent or non-table vector yields nothing.
    pub fn tables(&self, field: &str) -> impl Iterator<Item = &TableValue> {
        self.vector(field).unwrap_or(&[]).iter().filter_map(Value::as_table)
    }

    /// Iterates a vector of tables mutably without creating it. An absent or non-table vector
    /// yields nothing.
    pub fn tables_mut(&mut self, field: &str) -> impl Iterator<Item = &mut TableValue> {
        let items: &mut [Value] = match self.def().field_index(field) {
            Some(index) => match &mut self.fields[index] {
                Some(Value::Vector(items)) => items,
                _ => &mut [],
            },
            None => &mut [],
        };
        items.iter_mut().filter_map(Value::as_table_mut)
    }

    /// Returns an empty table of the type stored in `field`, ready to push onto its vector.
    pub fn new_table(&self, field: &str) -> Result<TableValue, FieldError> {
        let (_, def) = self.field_def(field)?;
        let (FieldType::Single(ElementType::Table(object)) | FieldType::Vector(ElementType::Table(object))) = def.ty
        else {
            return Err(mismatch(self.def(), def, "a table"));
        };
        Ok(TableValue::empty(&self.schema, object))
    }

    pub fn struct_value(&self, field: &str) -> Option<&StructValue> {
        self.get(field)?.as_struct()
    }

    /// Creates a zeroed struct when the field is absent.
    pub fn struct_mut(&mut self, field: &str) -> Result<&mut StructValue, FieldError> {
        let (index, def) = self.field_def(field)?;
        let FieldType::Single(ElementType::Struct(object)) = def.ty else {
            return Err(mismatch(self.def(), def, "a struct"));
        };
        if self.fields[index].is_none() {
            let created = StructValue::zeroed(&self.schema, object);
            self.fields[index] = Some(Value::Struct(created));
        }
        match &mut self.fields[index] {
            Some(Value::Struct(value)) => Ok(value),
            _ => Err(stored_mismatch(&self.schema, self.object, field, "a struct")),
        }
    }
}

impl StructValue {
    pub(super) fn zeroed(schema: &Arc<Schema>, object: ObjectId) -> Self {
        let zero_element = |element| match element {
            ElementType::Scalar(ty, _) => ty.zero(),
            ElementType::Struct(id) => Value::Struct(StructValue::zeroed(schema, id)),
            ElementType::Str => Value::Str(String::new()),
            ElementType::Table(id) => Value::Table(TableValue::empty(schema, id)),
        };
        let fields = schema
            .object_at(object)
            .fields
            .iter()
            .map(|field| match field.ty {
                FieldType::Single(element) => zero_element(element),
                FieldType::Array(element, len) => {
                    Value::Array((0..len).map(|_| zero_element(element)).collect())
                }
                FieldType::Vector(_) => Value::Vector(Vec::new()),
            })
            .collect();
        Self { schema: Arc::clone(schema), object, fields }
    }

    fn scalar(&self, field: &str) -> Result<(&FieldDef, Option<&Value>), FieldError> {
        let (index, def) = self.field_def(field)?;
        Ok((def, Some(&self.fields[index])))
    }

    fn store(&mut self, index: usize, value: Value) {
        self.fields[index] = value;
    }

    /// Iterates all fields in declaration order.
    pub fn fields(&self) -> impl Iterator<Item = (&FieldDef, &Value)> {
        self.def().fields.iter().zip(self.fields.iter())
    }

    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields.get(self.def().field_index(field)?)
    }

    pub fn struct_value(&self, field: &str) -> Option<&StructValue> {
        self.get(field)?.as_struct()
    }

    pub fn struct_mut(&mut self, field: &str) -> Result<&mut StructValue, FieldError> {
        let (index, def) = self.field_def(field)?;
        if !matches!(def.ty, FieldType::Single(ElementType::Struct(_))) {
            return Err(mismatch(self.def(), def, "a struct"));
        }
        match &mut self.fields[index] {
            Value::Struct(value) => Ok(value),
            _ => Err(stored_mismatch(&self.schema, self.object, field, "a struct")),
        }
    }

    pub fn array(&self, field: &str) -> Option<&[Value]> {
        match self.get(field)? {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Returns the elements mutably. The array's length is fixed by the schema, hence a slice.
    pub fn array_mut(&mut self, field: &str) -> Result<&mut [Value], FieldError> {
        let (index, def) = self.field_def(field)?;
        if !matches!(def.ty, FieldType::Array(..)) {
            return Err(mismatch(self.def(), def, "an array"));
        }
        match &mut self.fields[index] {
            Value::Array(items) => Ok(items),
            _ => Err(stored_mismatch(&self.schema, self.object, field, "an array")),
        }
    }
}
