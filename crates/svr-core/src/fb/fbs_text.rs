use std::collections::HashMap;
use std::sync::Arc;

use super::schema::{
    ElementType, EnumDef, EnumId, FieldDef, FieldType, ObjectDef, ObjectId, ObjectKind, ScalarType, Schema, SchemaError,
    MAX_STRUCT_NESTING,
};
use super::value::Value;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Text(String),
    Punct(char),
}

struct Lexed {
    token: Token,
    line: usize,
}

fn tokenize(text: &str) -> Result<Vec<Lexed>, SchemaError> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    let mut line = 1;
    while let Some(&c) = chars.peek() {
        match c {
            '\n' => {
                line += 1;
                chars.next();
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            '/' => {
                chars.next();
                match chars.next() {
                    Some('/') => {
                        for c in chars.by_ref() {
                            if c == '\n' {
                                line += 1;
                                break;
                            }
                        }
                    }
                    Some('*') => {
                        let mut previous = '\0';
                        let mut closed = false;
                        for c in chars.by_ref() {
                            if c == '\n' {
                                line += 1;
                            }
                            if previous == '*' && c == '/' {
                                closed = true;
                                break;
                            }
                            previous = c;
                        }
                        if !closed {
                            return Err(SchemaError::Text { line, message: "unterminated comment".into() });
                        }
                    }
                    _ => return Err(SchemaError::Text { line, message: "stray '/'".into() }),
                }
            }
            '"' => {
                chars.next();
                let mut value = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\n') | None => {
                            return Err(SchemaError::Text { line, message: "unterminated string".into() })
                        }
                        Some(c) => value.push(c),
                    }
                }
                tokens.push(Lexed { token: Token::Text(value), line });
            }
            '{' | '}' | '(' | ')' | '[' | ']' | ':' | ';' | ',' | '=' => {
                chars.next();
                tokens.push(Lexed { token: Token::Punct(c), line });
            }
            c if c.is_alphanumeric() || matches!(c, '_' | '-' | '+') => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '+') {
                        word.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Lexed { token: Token::Word(word), line });
            }
            other => return Err(SchemaError::Text { line, message: format!("unexpected character {other:?}") }),
        }
    }
    Ok(tokens)
}

enum RawType {
    Single(String),
    Vector(String),
    Array(String, u16),
}

struct RawField {
    name: String,
    ty: RawType,
    default: Option<String>,
    deprecated: bool,
    required: bool,
    line: usize,
}

struct RawObject {
    name: String,
    namespace: String,
    is_struct: bool,
    fields: Vec<RawField>,
}

struct RawEnum {
    name: String,
    underlying: ScalarType,
    values: Vec<(String, i64)>,
}

#[derive(Default)]
struct RawSchema {
    objects: Vec<RawObject>,
    enums: Vec<RawEnum>,
    root: Option<(String, String, usize)>,
    file_ident: Option<[u8; 4]>,
    file_ext: String,
    namespace: String,
}

struct Cursor {
    tokens: Vec<Lexed>,
    next: usize,
}

impl Cursor {
    fn line(&self) -> usize {
        self.tokens
            .get(self.next)
            .or(self.tokens.last())
            .map_or(1, |lexed| lexed.line)
    }

    fn error(&self, message: impl Into<String>) -> SchemaError {
        SchemaError::Text { line: self.line(), message: message.into() }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next).map(|lexed| &lexed.token)
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.peek().cloned();
        self.next += 1;
        token
    }

    fn eat(&mut self, punct: char) -> bool {
        let found = self.peek() == Some(&Token::Punct(punct));
        if found {
            self.next += 1;
        }
        found
    }

    fn expect(&mut self, punct: char) -> Result<(), SchemaError> {
        if self.eat(punct) {
            Ok(())
        } else {
            Err(self.error(format!("expected '{punct}'")))
        }
    }

    fn word(&mut self) -> Result<String, SchemaError> {
        match self.bump() {
            Some(Token::Word(word)) => Ok(word),
            _ => {
                self.next -= 1;
                Err(self.error("expected a name"))
            }
        }
    }

    fn text(&mut self) -> Result<String, SchemaError> {
        match self.bump() {
            Some(Token::Text(text)) => Ok(text),
            _ => {
                self.next -= 1;
                Err(self.error("expected a quoted string"))
            }
        }
    }

    /// Parses `(a, b: 1)`, reporting which of the attributes that change meaning were present.
    fn attributes(&mut self) -> Result<Attributes, SchemaError> {
        let mut attributes = Attributes::default();
        if !self.eat('(') {
            return Ok(attributes);
        }
        loop {
            let name = self.word()?;
            match name.as_str() {
                "required" => attributes.required = true,
                "deprecated" => attributes.deprecated = true,
                // These change the wire layout; ignoring them would misread every buffer.
                "id" | "force_align" | "nested_flatbuffer" | "flexbuffer" => {
                    return Err(SchemaError::Unsupported(format!("attribute {name}")))
                }
                _ => {}
            }
            if self.eat(':') {
                self.bump();
            }
            if !self.eat(',') {
                break;
            }
        }
        self.expect(')')?;
        Ok(attributes)
    }

    fn raw_type(&mut self) -> Result<RawType, SchemaError> {
        if !self.eat('[') {
            return Ok(RawType::Single(self.word()?));
        }
        let element = self.word()?;
        let ty = if self.eat(':') {
            let len = self.word()?;
            let len = len.parse::<u16>().map_err(|_| self.error(format!("bad array length {len}")))?;
            if len == 0 {
                return Err(self.error("array length must be positive"));
            }
            RawType::Array(element, len)
        } else {
            RawType::Vector(element)
        };
        self.expect(']')?;
        Ok(ty)
    }

    fn object(&mut self, raw: &mut RawSchema, is_struct: bool) -> Result<(), SchemaError> {
        let name = self.word()?;
        self.attributes()?;
        self.expect('{')?;
        let mut fields = Vec::new();
        while !self.eat('}') {
            let line = self.line();
            let field_name = self.word()?;
            self.expect(':')?;
            let ty = self.raw_type()?;
            let default = if self.eat('=') { Some(self.word()?) } else { None };
            let attributes = self.attributes()?;
            self.expect(';')?;
            fields.push(RawField {
                name: field_name,
                ty,
                default,
                deprecated: attributes.deprecated,
                required: attributes.required,
                line,
            });
        }
        raw.objects.push(RawObject {
            name: qualified(&raw.namespace, &name),
            namespace: raw.namespace.clone(),
            is_struct,
            fields,
        });
        Ok(())
    }

    fn enumeration(&mut self, raw: &mut RawSchema) -> Result<(), SchemaError> {
        let name = self.word()?;
        self.expect(':')?;
        let underlying_name = self.word()?;
        let underlying = builtin_scalar(&underlying_name)
            .ok_or_else(|| self.error(format!("{underlying_name} is not a scalar type")))?;
        self.attributes()?;
        self.expect('{')?;
        let mut values = Vec::new();
        let mut next_value = 0i64;
        while !self.eat('}') {
            let value_name = self.word()?;
            if self.eat('=') {
                let literal = self.word()?;
                next_value = parse_int(&literal)
                    .and_then(|v| i64::try_from(v).ok())
                    .ok_or_else(|| self.error(format!("bad enum value {literal}")))?;
            }
            if underlying.int_value(i128::from(next_value)).is_none() {
                return Err(self.error(format!("enum value {value_name} does not fit {underlying_name}")));
            }
            values.push((value_name, next_value));
            next_value = next_value.saturating_add(1);
            if !self.eat(',') {
                self.expect('}')?;
                break;
            }
        }
        raw.enums.push(RawEnum { name: qualified(&raw.namespace, &name), underlying, values });
        Ok(())
    }
}

#[derive(Default)]
struct Attributes {
    required: bool,
    deprecated: bool,
}

fn qualified(namespace: &str, name: &str) -> String {
    if namespace.is_empty() {
        name.to_owned()
    } else {
        format!("{namespace}.{name}")
    }
}

fn builtin_scalar(name: &str) -> Option<ScalarType> {
    Some(match name {
        "bool" => ScalarType::Bool,
        "byte" | "int8" => ScalarType::I8,
        "ubyte" | "uint8" => ScalarType::U8,
        "short" | "int16" => ScalarType::I16,
        "ushort" | "uint16" => ScalarType::U16,
        "int" | "int32" => ScalarType::I32,
        "uint" | "uint32" => ScalarType::U32,
        "long" | "int64" => ScalarType::I64,
        "ulong" | "uint64" => ScalarType::U64,
        "float" | "float32" => ScalarType::F32,
        "double" | "float64" => ScalarType::F64,
        _ => return None,
    })
}

fn parse_int(literal: &str) -> Option<i128> {
    let (negative, digits) = match literal.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, literal.strip_prefix('+').unwrap_or(literal)),
    };
    let magnitude = match digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        Some(hex) => i128::from_str_radix(hex, 16).ok()?,
        None => digits.parse::<i128>().ok()?,
    };
    Some(if negative { -magnitude } else { magnitude })
}

fn parse_declarations(text: &str) -> Result<RawSchema, SchemaError> {
    let mut cursor = Cursor { tokens: tokenize(text)?, next: 0 };
    let mut raw = RawSchema::default();
    while cursor.peek().is_some() {
        let keyword = cursor.word()?;
        match keyword.as_str() {
            "namespace" => {
                raw.namespace = cursor.word()?;
                cursor.expect(';')?;
            }
            "attribute" => {
                cursor.text()?;
                cursor.expect(';')?;
            }
            "root_type" => {
                let line = cursor.line();
                raw.root = Some((cursor.word()?, raw.namespace.clone(), line));
                cursor.expect(';')?;
            }
            "file_extension" => {
                raw.file_ext = cursor.text()?;
                cursor.expect(';')?;
            }
            "file_identifier" => {
                let ident = cursor.text()?;
                let bytes = <[u8; 4]>::try_from(ident.as_bytes())
                    .map_err(|_| cursor.error("a file identifier is exactly 4 bytes"))?;
                raw.file_ident = Some(bytes);
                cursor.expect(';')?;
            }
            "table" => cursor.object(&mut raw, false)?,
            "struct" => cursor.object(&mut raw, true)?,
            "enum" => cursor.enumeration(&mut raw)?,
            other => return Err(SchemaError::Unsupported(format!("declaration {other}"))),
        }
    }
    Ok(raw)
}

struct Names {
    objects: HashMap<String, u32>,
    enums: HashMap<String, u32>,
}

impl Names {
    /// Searches the namespace chain innermost first, as flatc does.
    fn candidates<'a>(namespace: &'a str, name: &'a str) -> impl Iterator<Item = String> + 'a {
        let mut scope = Some(namespace);
        std::iter::from_fn(move || {
            let current = scope?;
            scope = current.rfind('.').map(|dot| &current[..dot]).or((!current.is_empty()).then_some(""));
            Some(qualified(current, name))
        })
    }

    fn resolve(&self, raw: &[RawObject], enums: &[RawEnum], namespace: &str, name: &str) -> Option<ElementType> {
        if name == "string" {
            return Some(ElementType::Str);
        }
        if let Some(scalar) = builtin_scalar(name) {
            return Some(ElementType::Scalar(scalar, None));
        }
        for candidate in Self::candidates(namespace, name) {
            if let Some(&id) = self.enums.get(&candidate) {
                return Some(ElementType::Scalar(enums[id as usize].underlying, Some(EnumId(id))));
            }
            if let Some(&id) = self.objects.get(&candidate) {
                return Some(if raw[id as usize].is_struct {
                    ElementType::Struct(ObjectId(id))
                } else {
                    ElementType::Table(ObjectId(id))
                });
            }
        }
        None
    }
}

fn default_value(
    scalar: ScalarType,
    enum_def: Option<&RawEnum>,
    literal: Option<&str>,
) -> Option<Value> {
    let Some(literal) = literal else {
        return Some(scalar.zero());
    };
    match scalar {
        ScalarType::Bool => match literal {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            other => parse_int(other).map(|v| Value::Bool(v != 0)),
        },
        ScalarType::F32 | ScalarType::F64 => literal.parse().ok().map(Value::Float),
        _ => {
            let named = enum_def
                .and_then(|def| def.values.iter().find(|(name, _)| name == literal))
                .map(|&(_, value)| i128::from(value));
            scalar.int_value(named.or_else(|| parse_int(literal))?)
        }
    }
}

/// A struct inside a table must fit a vtable's 16-bit field offset.
const MAX_STRUCT_SIZE: usize = u16::MAX as usize;

/// Lays out one struct by the standard FlatBuffers rules, nested structs first.
fn layout_struct(
    index: usize,
    raw: &[RawObject],
    resolved: &[Vec<FieldType>],
    layouts: &mut [Option<StructLayout>],
    visiting: &mut Vec<usize>,
) -> Result<StructLayout, SchemaError> {
    if let Some(layout) = &layouts[index] {
        return Ok(layout.clone());
    }
    if visiting.contains(&index) {
        return Err(SchemaError::Invalid(format!("struct {} contains itself", raw[index].name)));
    }
    if visiting.len() >= MAX_STRUCT_NESTING {
        return Err(SchemaError::Invalid(format!("struct {} nests too deeply", raw[index].name)));
    }
    visiting.push(index);
    let mut size = 0usize;
    let mut align = 1usize;
    let mut offsets = Vec::with_capacity(resolved[index].len());
    for (field, ty) in raw[index].fields.iter().zip(&resolved[index]) {
        let (element, count) = match *ty {
            FieldType::Single(element) => (element, 1usize),
            FieldType::Array(element, len) => (element, usize::from(len)),
            FieldType::Vector(_) => {
                return Err(SchemaError::Invalid(format!("{}.{} cannot be stored in a struct", raw[index].name, field.name)))
            }
        };
        let (element_size, element_align) = match element {
            ElementType::Scalar(scalar, _) => (scalar.size(), scalar.size()),
            ElementType::Struct(id) => {
                let nested = layout_struct(id.0 as usize, raw, resolved, layouts, visiting)?;
                (nested.size, nested.align)
            }
            ElementType::Str | ElementType::Table(_) => {
                return Err(SchemaError::Invalid(format!("{}.{} cannot be stored in a struct", raw[index].name, field.name)))
            }
        };
        size = size.next_multiple_of(element_align);
        offsets.push(size as u32);
        size = element_size
            .checked_mul(count)
            .and_then(|bytes| size.checked_add(bytes))
            .filter(|&total| total <= MAX_STRUCT_SIZE)
            .ok_or_else(|| SchemaError::Invalid(format!("struct {} is too large", raw[index].name)))?;
        align = align.max(element_align);
    }
    visiting.pop();
    let layout = StructLayout { size: size.next_multiple_of(align), align, offsets };
    layouts[index] = Some(layout.clone());
    Ok(layout)
}

#[derive(Clone)]
struct StructLayout {
    size: usize,
    align: usize,
    offsets: Vec<u32>,
}

pub(super) fn load(text: &str) -> Result<Arc<Schema>, SchemaError> {
    let raw = parse_declarations(text)?;

    let names = Names {
        objects: raw.objects.iter().enumerate().map(|(i, o)| (o.name.clone(), i as u32)).collect(),
        enums: raw.enums.iter().enumerate().map(|(i, e)| (e.name.clone(), i as u32)).collect(),
    };
    let text_error = |line, message| SchemaError::Text { line, message };

    let mut resolved: Vec<Vec<FieldType>> = Vec::with_capacity(raw.objects.len());
    for object in &raw.objects {
        let mut types = Vec::with_capacity(object.fields.len());
        for field in &object.fields {
            let lookup = |name: &str| {
                names.resolve(&raw.objects, &raw.enums, &object.namespace, name).ok_or_else(|| {
                    text_error(field.line, format!("{}.{} has unknown type {name}", object.name, field.name))
                })
            };
            types.push(match &field.ty {
                RawType::Single(name) => FieldType::Single(lookup(name)?),
                RawType::Vector(name) => FieldType::Vector(lookup(name)?),
                RawType::Array(name, len) => FieldType::Array(lookup(name)?, *len),
            });
        }
        resolved.push(types);
    }

    let mut layouts: Vec<Option<StructLayout>> = vec![None; raw.objects.len()];
    for (index, object) in raw.objects.iter().enumerate() {
        if object.is_struct {
            layout_struct(index, &raw.objects, &resolved, &mut layouts, &mut Vec::new())?;
        }
    }

    let mut objects = Vec::with_capacity(raw.objects.len());
    for (index, object) in raw.objects.iter().enumerate() {
        let layout = layouts[index].as_ref();
        let mut fields = Vec::with_capacity(object.fields.len());
        for (slot, (field, &ty)) in object.fields.iter().zip(&resolved[index]).enumerate() {
            let default = match ty {
                FieldType::Single(ElementType::Scalar(scalar, enum_id)) => {
                    let enum_def = enum_id.map(|id| &raw.enums[id.0 as usize]);
                    let value = default_value(scalar, enum_def, field.default.as_deref()).ok_or_else(|| {
                        text_error(field.line, format!("{}.{} has a bad default", object.name, field.name))
                    })?;
                    Some(value)
                }
                _ if field.default.is_some() => {
                    return Err(text_error(field.line, format!("{}.{} cannot have a default", object.name, field.name)))
                }
                _ => None,
            };
            fields.push(FieldDef {
                name: field.name.clone(),
                ty,
                offset: layout.map_or(4 + 2 * slot as u32, |l| l.offsets[slot]),
                default,
                deprecated: field.deprecated,
                required: field.required,
            });
        }
        let kind = match layout {
            Some(l) => ObjectKind::Struct { size: l.size as u32, align: l.align as u32 },
            None => ObjectKind::Table,
        };
        objects.push(ObjectDef { name: object.name.clone(), kind, fields });
    }

    let enums = raw
        .enums
        .iter()
        .map(|e| EnumDef { name: e.name.clone(), underlying: e.underlying, values: e.values.clone() })
        .collect();

    let (root_name, root_namespace, root_line) = raw
        .root
        .as_ref()
        .ok_or(SchemaError::Text { line: 1, message: "no root_type declaration".into() })?;
    let root = Names::candidates(root_namespace, root_name)
        .find_map(|candidate| names.objects.get(&candidate).copied())
        .ok_or_else(|| text_error(*root_line, format!("root_type {root_name} is not declared")))?;

    Schema::new(objects, enums, ObjectId(root), raw.file_ident, raw.file_ext.clone())
}
