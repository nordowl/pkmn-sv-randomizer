use std::sync::Arc;

use super::schema::{
    ElementType, EnumDef, EnumId, FieldDef, FieldType, ObjectDef, ObjectId, ObjectKind, ScalarType, Schema, SchemaError,
};
use super::value::Value;
use super::wire::{Buf, TableRef};

const SCHEMA_OBJECTS: usize = 0;
const SCHEMA_ENUMS: usize = 1;
const SCHEMA_FILE_IDENT: usize = 2;
const SCHEMA_FILE_EXT: usize = 3;
const SCHEMA_ROOT_TABLE: usize = 4;

const OBJECT_NAME: usize = 0;
const OBJECT_FIELDS: usize = 1;
const OBJECT_IS_STRUCT: usize = 2;
const OBJECT_MINALIGN: usize = 3;
const OBJECT_BYTESIZE: usize = 4;

const FIELD_NAME: usize = 0;
const FIELD_TYPE: usize = 1;
const FIELD_ID: usize = 2;
const FIELD_OFFSET: usize = 3;
const FIELD_DEFAULT_INTEGER: usize = 4;
const FIELD_DEFAULT_REAL: usize = 5;
const FIELD_DEPRECATED: usize = 6;
const FIELD_REQUIRED: usize = 7;

const TYPE_BASE_TYPE: usize = 0;
const TYPE_ELEMENT: usize = 1;
const TYPE_INDEX: usize = 2;
const TYPE_FIXED_LENGTH: usize = 3;

const ENUM_NAME: usize = 0;
const ENUM_VALUES: usize = 1;
const ENUM_IS_UNION: usize = 2;
const ENUM_UNDERLYING_TYPE: usize = 3;

const ENUM_VAL_NAME: usize = 0;
const ENUM_VAL_VALUE: usize = 1;

const BASE_NONE: u8 = 0;
const BASE_UTYPE: u8 = 1;
const BASE_STRING: u8 = 13;
const BASE_VECTOR: u8 = 14;
const BASE_OBJ: u8 = 15;
const BASE_UNION: u8 = 16;
const BASE_ARRAY: u8 = 17;

fn scalar_type(base: u8) -> Option<ScalarType> {
    Some(match base {
        2 => ScalarType::Bool,
        3 => ScalarType::I8,
        4 => ScalarType::U8,
        5 => ScalarType::I16,
        6 => ScalarType::U16,
        7 => ScalarType::I32,
        8 => ScalarType::U32,
        9 => ScalarType::I64,
        10 => ScalarType::U64,
        11 => ScalarType::F32,
        12 => ScalarType::F64,
        _ => return None,
    })
}

fn bool_field(table: TableRef, slot: usize) -> Result<bool, SchemaError> {
    Ok(table.scalar::<1>(slot)?.is_some_and(|[b]| b != 0))
}

fn required_string<'a>(table: TableRef<'a>, slot: usize, what: &str) -> Result<&'a str, SchemaError> {
    table.string_field(slot)?.ok_or_else(|| SchemaError::Invalid(format!("{what} has no name")))
}

pub(super) fn load(bytes: &[u8]) -> Result<Arc<Schema>, SchemaError> {
    let found = bytes.get(4..8).unwrap_or(&[]);
    if found != b"BFBS" {
        return Err(SchemaError::NotBfbs { found: found.to_vec() });
    }
    let buf = Buf(bytes);
    let root = buf.table(buf.offset_target(0)?)?;

    let enums = root.tables_field(SCHEMA_ENUMS)?.into_iter().map(read_enum).collect::<Result<Vec<_>, _>>()?;

    let object_tables = root.tables_field(SCHEMA_OBJECTS)?;
    let struct_flags = object_tables
        .iter()
        .map(|&o| bool_field(o, OBJECT_IS_STRUCT))
        .collect::<Result<Vec<_>, _>>()?;
    let objects = object_tables
        .iter()
        .map(|&o| read_object(o, &struct_flags))
        .collect::<Result<Vec<_>, _>>()?;

    let root_table = root
        .table_field(SCHEMA_ROOT_TABLE)?
        .ok_or_else(|| SchemaError::Invalid("no root table".into()))?;
    let root_name = required_string(root_table, OBJECT_NAME, "root table")?;
    let root_index = objects
        .iter()
        .position(|o| o.name == root_name)
        .ok_or_else(|| SchemaError::Invalid(format!("root table {root_name} is not in the object list")))?;

    let file_ident = match root.string_field(SCHEMA_FILE_IDENT)?.unwrap_or("") {
        "" => None,
        ident => Some(<[u8; 4]>::try_from(ident.as_bytes()).map_err(|_| {
            SchemaError::Invalid(format!("file identifier {ident:?} is not 4 bytes"))
        })?),
    };
    let file_ext = root.string_field(SCHEMA_FILE_EXT)?.unwrap_or("").to_owned();

    Schema::new(objects, enums, ObjectId(root_index as u32), file_ident, file_ext)
}

fn read_enum(table: TableRef) -> Result<EnumDef, SchemaError> {
    let name = required_string(table, ENUM_NAME, "enum")?.to_owned();
    if bool_field(table, ENUM_IS_UNION)? {
        return Err(SchemaError::Unsupported(format!("union {name}")));
    }
    let underlying = table
        .table_field(ENUM_UNDERLYING_TYPE)?
        .and_then(|t| t.scalar::<1>(TYPE_BASE_TYPE).transpose())
        .transpose()?
        .and_then(|[base]| scalar_type(base))
        .ok_or_else(|| SchemaError::Invalid(format!("enum {name} has no scalar underlying type")))?;
    let values = table
        .tables_field(ENUM_VALUES)?
        .into_iter()
        .map(|v| {
            let value = v.scalar::<8>(ENUM_VAL_VALUE)?.map_or(0, i64::from_le_bytes);
            Ok((required_string(v, ENUM_VAL_NAME, "enum value")?.to_owned(), value))
        })
        .collect::<Result<Vec<_>, SchemaError>>()?;
    Ok(EnumDef { name, underlying, values })
}

fn read_object(table: TableRef, struct_flags: &[bool]) -> Result<ObjectDef, SchemaError> {
    let name = required_string(table, OBJECT_NAME, "object")?.to_owned();
    let kind = if bool_field(table, OBJECT_IS_STRUCT)? {
        let read_i32 = |slot| -> Result<i32, SchemaError> {
            Ok(table.scalar::<4>(slot)?.map_or(0, i32::from_le_bytes))
        };
        let size = u32::try_from(read_i32(OBJECT_BYTESIZE)?);
        let align = u32::try_from(read_i32(OBJECT_MINALIGN)?);
        match (size, align) {
            (Ok(size), Ok(align)) => ObjectKind::Struct { size, align },
            _ => return Err(SchemaError::Invalid(format!("struct {name} has a negative layout"))),
        }
    } else {
        ObjectKind::Table
    };

    let mut fields = table
        .tables_field(OBJECT_FIELDS)?
        .into_iter()
        .map(|f| {
            let id = f.scalar::<2>(FIELD_ID)?.map_or(0, u16::from_le_bytes);
            Ok((id, read_field(f, &name, struct_flags)?))
        })
        .collect::<Result<Vec<_>, SchemaError>>()?;
    fields.sort_by_key(|&(id, _)| id);
    if fields.iter().enumerate().any(|(index, &(id, _))| usize::from(id) != index) {
        return Err(SchemaError::Invalid(format!("{name} has non-contiguous field ids")));
    }
    Ok(ObjectDef { name, kind, fields: fields.into_iter().map(|(_, field)| field).collect() })
}

fn read_field(table: TableRef, object: &str, struct_flags: &[bool]) -> Result<FieldDef, SchemaError> {
    let name = required_string(table, FIELD_NAME, "field")?.to_owned();
    let label = || format!("{object}.{name}");
    let ty_table = table
        .table_field(FIELD_TYPE)?
        .ok_or_else(|| SchemaError::Invalid(format!("{} has no type", label())))?;
    let ty = read_type(ty_table, struct_flags).map_err(|e| match e {
        TypeProblem::Schema(e) => e,
        TypeProblem::Unsupported(what) => SchemaError::Unsupported(format!("{} is {what}", label())),
        TypeProblem::BadIndex => SchemaError::Invalid(format!("{} has an out-of-range type index", label())),
    })?;

    let default_int = table.scalar::<8>(FIELD_DEFAULT_INTEGER)?.map_or(0, i64::from_le_bytes);
    let default_real = table.scalar::<8>(FIELD_DEFAULT_REAL)?.map_or(0.0, f64::from_le_bytes);
    let default = match ty {
        FieldType::Single(ElementType::Scalar(scalar, _)) => Some(
            scalar_default(scalar, default_int, default_real)
                .ok_or_else(|| SchemaError::Invalid(format!("{} default {default_int} does not fit", label())))?,
        ),
        _ => None,
    };

    Ok(FieldDef {
        offset: u32::from(table.scalar::<2>(FIELD_OFFSET)?.map_or(0, u16::from_le_bytes)),
        ty,
        default,
        deprecated: bool_field(table, FIELD_DEPRECATED)?,
        required: bool_field(table, FIELD_REQUIRED)?,
        name,
    })
}

fn scalar_default(scalar: ScalarType, int: i64, real: f64) -> Option<Value> {
    match scalar {
        ScalarType::Bool => Some(Value::Bool(int != 0)),
        ScalarType::F32 | ScalarType::F64 => Some(Value::Float(real)),
        // Reflection stores every integer default as i64, so a u64 default arrives bit-cast.
        ScalarType::U64 => Some(Value::UInt(int as u64)),
        _ => scalar.int_value(i128::from(int)),
    }
}

enum TypeProblem {
    Schema(SchemaError),
    Unsupported(&'static str),
    BadIndex,
}

impl<E: Into<SchemaError>> From<E> for TypeProblem {
    fn from(e: E) -> Self {
        TypeProblem::Schema(e.into())
    }
}

fn read_type(table: TableRef, struct_flags: &[bool]) -> Result<FieldType, TypeProblem> {
    let base = table.scalar::<1>(TYPE_BASE_TYPE)?.map_or(BASE_NONE, |[b]| b);
    let element = table.scalar::<1>(TYPE_ELEMENT)?.map_or(BASE_NONE, |[b]| b);
    let index = table.scalar::<4>(TYPE_INDEX)?.map_or(-1, i32::from_le_bytes);
    let fixed_length = table.scalar::<2>(TYPE_FIXED_LENGTH)?.map_or(0, u16::from_le_bytes);
    Ok(match base {
        BASE_VECTOR => FieldType::Vector(read_element(element, index, struct_flags)?),
        BASE_ARRAY => FieldType::Array(read_element(element, index, struct_flags)?, fixed_length),
        other => FieldType::Single(read_element(other, index, struct_flags)?),
    })
}

fn read_element(base: u8, index: i32, struct_flags: &[bool]) -> Result<ElementType, TypeProblem> {
    match base {
        BASE_UTYPE | BASE_UNION => Err(TypeProblem::Unsupported("a union")),
        BASE_STRING => Ok(ElementType::Str),
        BASE_OBJ => {
            let id = usize::try_from(index).map_err(|_| TypeProblem::BadIndex)?;
            match struct_flags.get(id) {
                Some(true) => Ok(ElementType::Struct(ObjectId(id as u32))),
                Some(false) => Ok(ElementType::Table(ObjectId(id as u32))),
                None => Err(TypeProblem::BadIndex),
            }
        }
        other => {
            let scalar = scalar_type(other).ok_or(TypeProblem::Unsupported("an unknown base type"))?;
            let enum_id = u32::try_from(index).ok().map(EnumId);
            Ok(ElementType::Scalar(scalar, enum_id))
        }
    }
}
