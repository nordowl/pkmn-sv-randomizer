use std::sync::Arc;

use thiserror::Error;

use super::value::Value;
use super::wire::WireError;

/// Far deeper than any real schema; stops a crafted chain of structs overflowing the stack.
pub(super) const MAX_STRUCT_NESTING: usize = 64;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum SchemaError {
    #[error("not a binary schema: bytes 4..8 are {found:?}, expected \"BFBS\"")]
    NotBfbs { found: Vec<u8> },
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error("invalid schema: {0}")]
    Invalid(String),
    #[error("schema construct not supported: {0}")]
    Unsupported(String),
    #[error("schema text, line {line}: {message}")]
    Text { line: usize, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectId(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumId(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarType {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl ScalarType {
    pub fn size(self) -> usize {
        match self {
            Self::Bool | Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::U8 => "u8",
            Self::I16 => "i16",
            Self::U16 => "u16",
            Self::I32 => "i32",
            Self::U32 => "u32",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    /// Builds the `Value` for an integer, or returns `None` when this type is not an integer or
    /// cannot hold `v`.
    pub(super) fn int_value(self, v: i128) -> Option<Value> {
        let (signed, bits) = match self {
            Self::Bool | Self::F32 | Self::F64 => return None,
            Self::I8 => (true, 8),
            Self::U8 => (false, 8),
            Self::I16 => (true, 16),
            Self::U16 => (false, 16),
            Self::I32 => (true, 32),
            Self::U32 => (false, 32),
            Self::I64 => (true, 64),
            Self::U64 => (false, 64),
        };
        let (lo, hi) = if signed {
            (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
        } else {
            (0, (1i128 << bits) - 1)
        };
        if v < lo || v > hi {
            None
        } else if signed {
            Some(Value::Int(v as i64))
        } else {
            Some(Value::UInt(v as u64))
        }
    }

    pub(super) fn zero(self) -> Value {
        match self {
            Self::Bool => Value::Bool(false),
            Self::I8 | Self::I16 | Self::I32 | Self::I64 => Value::Int(0),
            Self::U8 | Self::U16 | Self::U32 | Self::U64 => Value::UInt(0),
            Self::F32 | Self::F64 => Value::Float(0.0),
        }
    }
}

/// How one value of a single field, a vector element or an array element is stored. All three
/// share the same inline representation, which is why nested vectors have no place in this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    Scalar(ScalarType, Option<EnumId>),
    Str,
    Table(ObjectId),
    Struct(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Single(ElementType),
    Vector(ElementType),
    Array(ElementType, u16),
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: FieldType,
    /// Matches reflection's `Field.offset`: the byte offset inside a struct, or the vtable entry
    /// position inside a table.
    pub offset: u32,
    /// The schema default for scalar fields.
    pub default: Option<Value>,
    pub deprecated: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Table,
    Struct { size: u32, align: u32 },
}

#[derive(Debug, Clone)]
pub struct ObjectDef {
    pub name: String,
    pub kind: ObjectKind,
    /// Ordered by field id, so a field's index is its vtable slot.
    pub fields: Vec<FieldDef>,
}

impl ObjectDef {
    pub(super) fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub underlying: ScalarType,
    pub values: Vec<(String, i64)>,
}

#[derive(Debug)]
pub struct Schema {
    objects: Vec<ObjectDef>,
    enums: Vec<EnumDef>,
    root: ObjectId,
    file_ident: Option<[u8; 4]>,
    file_ext: String,
}

impl Schema {
    pub fn from_bfbs(bytes: &[u8]) -> Result<Arc<Schema>, SchemaError> {
        super::reflection::load(bytes)
    }

    pub fn from_fbs_text(text: &str) -> Result<Arc<Schema>, SchemaError> {
        super::fbs_text::load(text)
    }

    pub(super) fn new(
        objects: Vec<ObjectDef>,
        enums: Vec<EnumDef>,
        root: ObjectId,
        file_ident: Option<[u8; 4]>,
        file_ext: String,
    ) -> Result<Arc<Schema>, SchemaError> {
        let schema = Schema { objects, enums, root, file_ident, file_ext };
        schema.validate()?;
        Ok(Arc::new(schema))
    }

    /// Accepts a fully qualified enum name, or an unqualified one when exactly one enum matches.
    pub fn enum_def(&self, name: &str) -> Option<&EnumDef> {
        if let Some(exact) = self.enums.iter().find(|e| e.name == name) {
            return Some(exact);
        }
        let mut matches = self.enums.iter().filter(|e| e.name.rsplit('.').next() == Some(name));
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    pub fn enum_value(&self, enum_name: &str, value_name: &str) -> Option<i64> {
        let def = self.enum_def(enum_name)?;
        def.values.iter().find(|(name, _)| name == value_name).map(|&(_, value)| value)
    }

    pub fn enum_name(&self, enum_name: &str, value: i64) -> Option<&str> {
        let def = self.enum_def(enum_name)?;
        def.values.iter().find(|&&(_, v)| v == value).map(|(name, _)| name.as_str())
    }

    pub fn enum_entries(&self, enum_name: &str) -> Option<Vec<(String, i64)>> {
        self.enum_def(enum_name).map(|def| def.values.clone())
    }

    pub fn objects(&self) -> &[ObjectDef] {
        &self.objects
    }

    pub fn enums(&self) -> &[EnumDef] {
        &self.enums
    }

    pub fn object_at(&self, id: ObjectId) -> &ObjectDef {
        &self.objects[id.0 as usize]
    }

    pub fn enum_at(&self, id: EnumId) -> &EnumDef {
        &self.enums[id.0 as usize]
    }

    pub fn root_id(&self) -> ObjectId {
        self.root
    }

    pub fn file_ident(&self) -> Option<[u8; 4]> {
        self.file_ident
    }

    pub fn file_ext(&self) -> &str {
        &self.file_ext
    }

    /// Returns the inline `(size, align)` of one element.
    pub fn element_layout(&self, element: ElementType) -> (usize, usize) {
        match element {
            ElementType::Scalar(ty, _) => (ty.size(), ty.size()),
            ElementType::Str | ElementType::Table(_) => (4, 4),
            ElementType::Struct(id) => match self.object_at(id).kind {
                ObjectKind::Struct { size, align } => (size as usize, align as usize),
                ObjectKind::Table => (4, 4),
            },
        }
    }

    /// Returns the inline `(size, align)` of a field inside its table or struct.
    pub fn field_layout(&self, ty: FieldType) -> (usize, usize) {
        match ty {
            FieldType::Single(element) => self.element_layout(element),
            FieldType::Vector(_) => (4, 4),
            FieldType::Array(element, len) => {
                let (size, align) = self.element_layout(element);
                (size * usize::from(len), align)
            }
        }
    }

    fn validate(&self) -> Result<(), SchemaError> {
        let invalid = |msg: String| Err(SchemaError::Invalid(msg));
        if !self.has_kind(self.root, |kind| kind == ObjectKind::Table) {
            return invalid("the root is not a table".into());
        }
        for e in &self.enums {
            if e.underlying.int_value(0).is_none() {
                return invalid(format!("enum {} has a non-integer underlying type", e.name));
            }
        }
        for object in &self.objects {
            if let ObjectKind::Struct { size, align } = object.kind {
                if size == 0 || !align.is_power_of_two() || size % align != 0 {
                    return invalid(format!("struct {} has size {size} and alignment {align}", object.name));
                }
            }
        }
        for object in &self.objects {
            let is_struct = matches!(object.kind, ObjectKind::Struct { .. });
            for field in &object.fields {
                self.validate_ids(object, field)?;
                if is_struct && !self.is_inline(field.ty) {
                    return invalid(format!("{}.{} cannot be stored in a struct", object.name, field.name));
                }
                if !is_struct && matches!(field.ty, FieldType::Array(..)) {
                    return invalid(format!("{}.{} is an array outside a struct", object.name, field.name));
                }
            }
        }
        let mut visits = vec![Visit::New; self.objects.len()];
        for (index, object) in self.objects.iter().enumerate() {
            if let ObjectKind::Struct { size, align } = object.kind {
                self.validate_struct_layout(object, size as usize, align as usize)?;
                self.struct_height(index, 0, &mut visits)?;
            }
        }
        Ok(())
    }

    fn has_kind(&self, id: ObjectId, test: impl Fn(ObjectKind) -> bool) -> bool {
        self.objects.get(id.0 as usize).is_some_and(|o| test(o.kind))
    }

    fn validate_ids(&self, object: &ObjectDef, field: &FieldDef) -> Result<(), SchemaError> {
        let (FieldType::Single(element) | FieldType::Vector(element) | FieldType::Array(element, _)) = field.ty;
        let ok = match element {
            ElementType::Scalar(_, Some(id)) => (id.0 as usize) < self.enums.len(),
            ElementType::Scalar(_, None) | ElementType::Str => true,
            ElementType::Table(id) => self.has_kind(id, |k| k == ObjectKind::Table),
            ElementType::Struct(id) => self.has_kind(id, |k| matches!(k, ObjectKind::Struct { .. })),
        };
        if ok {
            Ok(())
        } else {
            Err(SchemaError::Invalid(format!("{}.{} refers to a missing or mismatched type", object.name, field.name)))
        }
    }

    fn is_inline(&self, ty: FieldType) -> bool {
        match ty {
            FieldType::Single(ElementType::Scalar(..) | ElementType::Struct(_)) => true,
            FieldType::Array(ElementType::Scalar(..) | ElementType::Struct(_), len) => len > 0,
            FieldType::Single(_) | FieldType::Vector(_) | FieldType::Array(..) => false,
        }
    }

    fn validate_struct_layout(&self, object: &ObjectDef, size: usize, align: usize) -> Result<(), SchemaError> {
        for field in &object.fields {
            let (field_size, field_align) = self.field_layout(field.ty);
            let offset = field.offset as usize;
            if offset % field_align != 0 || offset + field_size > size || field_align > align {
                return Err(SchemaError::Invalid(format!(
                    "{}.{} does not fit its struct: offset {offset}, size {field_size}, alignment {field_align}",
                    object.name, field.name
                )));
            }
        }
        Ok(())
    }

    /// Returns how many struct levels `index` nests, rejecting cycles and excessive depth.
    fn struct_height(&self, index: usize, depth: usize, visits: &mut [Visit]) -> Result<usize, SchemaError> {
        let name = || &self.objects[index].name;
        match visits[index] {
            Visit::Done(height) => return Ok(height),
            Visit::Active => return Err(SchemaError::Invalid(format!("struct {} contains itself", name()))),
            Visit::New => visits[index] = Visit::Active,
        }
        let too_deep = || SchemaError::Invalid(format!("struct {} nests too deeply", name()));
        if depth >= MAX_STRUCT_NESTING {
            return Err(too_deep());
        }
        let mut height = 1;
        for field in &self.objects[index].fields {
            if let FieldType::Single(ElementType::Struct(id)) | FieldType::Array(ElementType::Struct(id), _) = field.ty {
                height = height.max(1 + self.struct_height(id.0 as usize, depth + 1, visits)?);
            }
        }
        if height > MAX_STRUCT_NESTING {
            return Err(too_deep());
        }
        visits[index] = Visit::Done(height);
        Ok(height)
    }
}

#[derive(Clone, Copy)]
enum Visit {
    New,
    Active,
    Done(usize),
}
