use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Map, Value as Json};
use svr_core::fb::{
    self, builtin, ElementType, FieldDef, FieldType, ObjectDef, ObjectKind, ParseError, ScalarType, Schema, SchemaError,
    SerializeError, StructValue, TableValue, Value,
};

fn fixtures() -> Option<PathBuf> {
    match std::env::var_os("SVR_TEST_TABLES") {
        Some(path) => Some(PathBuf::from(path)),
        None => {
            println!("skipped: SVR_TEST_TABLES is not set");
            None
        }
    }
}

fn references() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/gonzalooo/SV_FLATBUFFERS")
}

fn walk_files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk_files(&path, extension, out);
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
}

fn files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_files(dir, extension, &mut out);
    out.sort();
    out
}

/// The schema for a table: its sibling `.bfbs`, or the embedded one for the three that have none.
fn schema_for(table: &Path) -> Arc<Schema> {
    let sibling = table.with_extension("bfbs");
    if sibling.exists() {
        return Schema::from_bfbs(&std::fs::read(sibling).unwrap()).unwrap();
    }
    match table.file_stem().unwrap().to_str().unwrap() {
        "personal_array" => builtin::personal(),
        "waza_array" => builtin::waza(),
        "tokusei_array" => builtin::tokusei(),
        other => panic!("no schema for {other}"),
    }
}

fn field_kind(ty: FieldType, in_struct: bool) -> String {
    let element_kind = |element: ElementType| match element {
        ElementType::Scalar(ty, enum_id) => {
            format!("{ty:?}{}", if enum_id.is_some() { "(enum)" } else { "" })
        }
        ElementType::Str => "string".to_owned(),
        ElementType::Table(_) => "table".to_owned(),
        ElementType::Struct(_) => "struct".to_owned(),
    };
    let owner = if in_struct { "struct" } else { "table" };
    match ty {
        FieldType::Single(element) => format!("{owner} field: {}", element_kind(element)),
        FieldType::Vector(element) => format!("{owner} field: vector of {}", element_kind(element)),
        FieldType::Array(element, _) => format!("{owner} field: fixed array of {}", element_kind(element)),
    }
}

#[test]
fn criterion_1_all_schemas_parse() {
    let Some(dir) = fixtures() else { return };
    let paths = files(&dir, "bfbs");
    let mut objects = 0;
    let mut tables = 0;
    let mut structs = 0;
    let mut enums = 0;
    let mut deprecated = 0;
    let mut required = 0;
    let mut nonzero_defaults = 0;
    let mut idents = 0;
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut underlying: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures = Vec::new();
    for path in &paths {
        let schema = match Schema::from_bfbs(&std::fs::read(path).unwrap()) {
            Ok(schema) => schema,
            Err(error) => {
                failures.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        objects += schema.objects().len();
        enums += schema.enums().len();
        idents += usize::from(schema.file_ident().is_some());
        for def in schema.enums() {
            *underlying.entry(format!("{:?}", def.underlying)).or_default() += 1;
        }
        for object in schema.objects() {
            let in_struct = matches!(object.kind, ObjectKind::Struct { .. });
            if in_struct {
                structs += 1;
            } else {
                tables += 1;
            }
            for field in &object.fields {
                *kinds.entry(field_kind(field.ty, in_struct)).or_default() += 1;
                deprecated += usize::from(field.deprecated);
                required += usize::from(field.required);
                nonzero_defaults += usize::from(match &field.default {
                    Some(Value::Bool(v)) => *v,
                    Some(Value::Int(v)) => *v != 0,
                    Some(Value::UInt(v)) => *v != 0,
                    Some(Value::Float(v)) => *v != 0.0,
                    _ => false,
                });
            }
        }
    }
    println!("schemas parsed: {} of {}", paths.len() - failures.len(), paths.len());
    println!("objects: {objects} ({tables} tables, {structs} structs); enums: {enums}");
    println!("schemas declaring a file identifier: {idents}");
    println!("fields: {deprecated} deprecated, {required} required, {nonzero_defaults} with a non-zero default");
    println!("enum underlying types: {underlying:?}");
    println!("field constructs:");
    for (kind, count) in &kinds {
        println!("  {count:>6}  {kind}");
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(paths.len(), 287);
}

/// What a walk of one buffer found. Nothing here is computed by the crate's parser or writer: it
/// reads raw bytes itself and takes only the field list from the schema.
#[derive(Default)]
struct Walk {
    problems: Vec<String>,
    unmapped_slots: usize,
    max_slots: BTreeMap<String, usize>,
    field_counts: BTreeMap<String, usize>,
    odd_bools: usize,
    non_finite_floats: usize,
    tables: usize,
    non_utf8_strings: usize,
}

struct Walker<'a> {
    buf: &'a [u8],
    schema: &'a Schema,
    touched: Vec<bool>,
    report: Walk,
}

impl Walker<'_> {
    fn problem(&mut self, message: String) {
        if self.report.problems.len() < 20 {
            self.report.problems.push(message);
        } else if self.report.problems.len() == 20 {
            self.report.problems.push("further problems omitted".to_owned());
        }
    }

    fn in_bounds(&mut self, pos: usize, len: usize, what: &str) -> bool {
        let fits = pos.checked_add(len).is_some_and(|end| end <= self.buf.len());
        if !fits {
            self.problem(format!("{what}: {len} bytes at {pos} exceed the buffer"));
        }
        fits
    }

    fn mark(&mut self, pos: usize, len: usize, what: &str) -> bool {
        let fits = self.in_bounds(pos, len, what);
        if fits {
            self.touched[pos..pos + len].fill(true);
        }
        fits
    }

    fn aligned(&mut self, pos: usize, align: usize, what: &str) {
        if pos % align != 0 {
            self.problem(format!("{what} at {pos} is not aligned to {align}"));
        }
    }

    fn u16_at(&self, pos: usize) -> u16 {
        u16::from_le_bytes([self.buf[pos], self.buf[pos + 1]])
    }

    fn u32_at(&self, pos: usize) -> u32 {
        u32::from_le_bytes(self.buf[pos..pos + 4].try_into().unwrap())
    }

    fn uoffset(&mut self, pos: usize, what: &str) -> Option<usize> {
        if !self.mark(pos, 4, what) {
            return None;
        }
        self.aligned(pos, 4, what);
        let offset = self.u32_at(pos) as usize;
        if offset == 0 {
            self.problem(format!("{what} at {pos} is a zero uoffset"));
            return None;
        }
        Some(pos + offset)
    }

    fn table(&mut self, object: &ObjectDef, pos: usize) {
        self.report.tables += 1;
        if !self.mark(pos, 4, "soffset") {
            return;
        }
        self.aligned(pos, 4, "table");
        let soffset = i32::from_le_bytes(self.buf[pos..pos + 4].try_into().unwrap());
        let Some(vtable) = usize::try_from(pos as i64 - i64::from(soffset)).ok() else {
            self.problem(format!("table at {pos} has a vtable before the buffer"));
            return;
        };
        if !self.mark(vtable, 4, "vtable header") {
            return;
        }
        self.aligned(vtable, 2, "vtable");
        let vtable_len = usize::from(self.u16_at(vtable));
        let object_size = usize::from(self.u16_at(vtable + 2));
        if vtable_len < 4 || vtable_len % 2 != 0 {
            self.problem(format!("vtable at {vtable} has size {vtable_len}"));
            return;
        }
        if !self.mark(vtable, vtable_len, "vtable") || !self.in_bounds(pos, object_size, "table body") {
            return;
        }

        let slots = (vtable_len - 4) / 2;
        let longest = self.report.max_slots.entry(object.name.clone()).or_default();
        *longest = (*longest).max(slots);
        self.report.field_counts.insert(object.name.clone(), object.fields.len());
        if slots > object.fields.len() {
            self.report.unmapped_slots += 1;
            self.problem(format!("{} at {pos} has {slots} vtable slots, schema has {}", object.name, object.fields.len()));
        }

        let mut spans: Vec<(usize, usize)> = Vec::new();
        for slot in 0..slots {
            let offset = usize::from(self.u16_at(vtable + 4 + 2 * slot));
            if offset == 0 {
                continue;
            }
            let Some(field) = object.fields.get(slot) else {
                continue;
            };
            let (size, align) = self.schema.field_layout(field.ty);
            spans.push((offset, offset + size));
            if offset < 4 || offset + size > object_size {
                self.problem(format!("{}.{} at offset {offset}+{size} leaves the {object_size}-byte table", object.name, field.name));
                continue;
            }
            self.aligned(pos + offset, align, &field.name);
            self.field(field, pos + offset);
        }
        spans.sort_unstable();
        for pair in spans.windows(2) {
            if pair[0].1 > pair[1].0 {
                self.problem(format!("{} at {pos} has overlapping fields {pair:?}", object.name));
            }
        }
    }

    fn field(&mut self, field: &FieldDef, pos: usize) {
        match field.ty {
            FieldType::Single(element) => self.element(element, pos, &field.name),
            FieldType::Vector(element) => {
                let Some(start) = self.uoffset(pos, &field.name) else { return };
                if !self.mark(start, 4, "vector length") {
                    return;
                }
                self.aligned(start, 4, "vector length");
                let count = self.u32_at(start) as usize;
                let (size, align) = self.schema.element_layout(element);
                let data = start + 4;
                self.aligned(data, align, "vector data");
                if count.checked_mul(size).is_none_or(|len| data + len > self.buf.len()) {
                    self.problem(format!("vector at {start} with {count} elements exceeds the buffer"));
                    return;
                }
                for index in 0..count {
                    self.element(element, data + index * size, &field.name);
                }
            }
            FieldType::Array(element, len) => {
                let (size, _) = self.schema.element_layout(element);
                for index in 0..usize::from(len) {
                    self.element(element, pos + index * size, &field.name);
                }
            }
        }
    }

    fn element(&mut self, element: ElementType, pos: usize, name: &str) {
        match element {
            ElementType::Scalar(ty, _) => self.scalar(ty, pos, name),
            ElementType::Str => {
                let Some(start) = self.uoffset(pos, name) else { return };
                if !self.mark(start, 4, "string length") {
                    return;
                }
                self.aligned(start, 4, "string length");
                let len = self.u32_at(start) as usize;
                let data = start + 4;
                if !self.mark(data, len + 1, "string") {
                    return;
                }
                if self.buf[data + len] != 0 {
                    self.problem(format!("string at {start} has no NUL terminator"));
                }
                if std::str::from_utf8(&self.buf[data..data + len]).is_err() {
                    self.report.non_utf8_strings += 1;
                }
            }
            ElementType::Table(id) => {
                let schema = self.schema;
                let Some(target) = self.uoffset(pos, name) else { return };
                self.table(schema.object_at(id), target);
            }
            ElementType::Struct(id) => {
                let schema = self.schema;
                let object = schema.object_at(id);
                if let ObjectKind::Struct { size, align } = object.kind {
                    self.aligned(pos, align as usize, "struct");
                    if pos + size as usize > self.buf.len() {
                        self.problem(format!("struct {} at {pos} exceeds the buffer", object.name));
                        return;
                    }
                }
                for field in &object.fields {
                    self.field(field, pos + field.offset as usize);
                }
            }
        }
    }

    fn scalar(&mut self, ty: ScalarType, pos: usize, name: &str) {
        if !self.mark(pos, ty.size(), name) {
            return;
        }
        self.aligned(pos, ty.size(), name);
        match ty {
            ScalarType::Bool if self.buf[pos] > 1 => self.report.odd_bools += 1,
            ScalarType::F32 if !f32::from_le_bytes(self.buf[pos..pos + 4].try_into().unwrap()).is_finite() => {
                self.report.non_finite_floats += 1;
            }
            ScalarType::F64 if !f64::from_le_bytes(self.buf[pos..pos + 8].try_into().unwrap()).is_finite() => {
                self.report.non_finite_floats += 1;
            }
            _ => {}
        }
    }
}

struct Coverage {
    walk: Walk,
    unexplained: Vec<usize>,
    unexplained_nonzero: Vec<usize>,
}

fn structural_check(schema: &Schema, buf: &[u8]) -> Coverage {
    let mut walker = Walker { buf, schema, touched: vec![false; buf.len()], report: Walk::default() };
    let identifier_len = if schema.file_ident().is_some() { 4 } else { 0 };
    walker.mark(4, identifier_len, "file identifier");
    if let Some(root) = walker.uoffset(0, "root") {
        walker.table(schema.object_at(schema.root_id()), root);
    }
    let unexplained: Vec<usize> = (0..buf.len()).filter(|&pos| !walker.touched[pos]).collect();
    let unexplained_nonzero = unexplained.iter().copied().filter(|&pos| buf[pos] != 0).collect();
    Coverage { walk: walker.report, unexplained, unexplained_nonzero }
}

#[test]
fn criteria_2_and_3_round_trip_structure_and_determinism() {
    let Some(dir) = fixtures() else { return };
    let paths = files(&dir, "bin");
    let mut original_bytes = 0usize;
    let mut output_bytes = 0usize;
    let mut failures = Vec::new();
    let mut ratios: Vec<(f64, usize, usize, String)> = Vec::new();
    let mut original_problem_tables = Vec::new();
    let mut original_nonzero = Vec::new();
    let mut odd_bools = 0;
    let mut non_finite = 0;
    let mut non_utf8 = 0;
    let mut tables = 0;
    let mut padding_original = 0;
    let mut padding_output = 0;
    let mut round_tripped = 0;
    let mut densest = (0.0f64, String::new());

    for path in &paths {
        let failures_before = failures.len();
        let name = path.strip_prefix(&dir).unwrap().display().to_string();
        let schema = schema_for(path);
        let original = std::fs::read(path).unwrap();
        let first = match fb::parse(&schema, &original) {
            Ok(tree) => tree,
            Err(error) => {
                failures.push(format!("{name}: original does not parse: {error}"));
                continue;
            }
        };
        let bytes = fb::serialize(&first);
        if bytes != fb::serialize(&first) {
            failures.push(format!("{name}: serialize is not deterministic"));
        }
        match fb::parse(&schema, &bytes) {
            Ok(second) => {
                if second != first {
                    failures.push(format!("{name}: re-parsed tree differs: {:?}", diff_tables(&first, &second).first()));
                }
                if fb::serialize(&second) != bytes {
                    failures.push(format!("{name}: serialize(parse(serialize(x))) changed the bytes"));
                }
            }
            Err(error) => failures.push(format!("{name}: output does not parse: {error}")),
        }

        let output = structural_check(&schema, &bytes);
        if !output.walk.problems.is_empty() {
            failures.push(format!("{name}: output fails the structural check: {:?}", output.walk.problems));
        }
        if !output.unexplained_nonzero.is_empty() {
            failures.push(format!("{name}: output has {} unexplained non-zero bytes", output.unexplained_nonzero.len()));
        }

        let source = structural_check(&schema, &original);
        if !source.walk.problems.is_empty() {
            original_problem_tables.push(format!("{name}: {:?}", &source.walk.problems[..source.walk.problems.len().min(3)]));
        }
        if !source.unexplained_nonzero.is_empty() {
            original_nonzero.push(format!("{name}: {} bytes, first at {}", source.unexplained_nonzero.len(), source.unexplained_nonzero[0]));
        }
        odd_bools += source.walk.odd_bools;
        non_finite += source.walk.non_finite_floats;
        non_utf8 += source.walk.non_utf8_strings;
        tables += source.walk.tables;
        padding_original += source.unexplained.len();
        padding_output += output.unexplained.len();

        let density = count_table_values(&first) as f64 / original.len() as f64;
        if density > densest.0 {
            densest = (density, name.clone());
        }
        round_tripped += usize::from(failures.len() == failures_before);
        original_bytes += original.len();
        output_bytes += bytes.len();
        ratios.push((bytes.len() as f64 / original.len() as f64, original.len(), bytes.len(), name));
    }

    ratios.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!("tables round-tripped: {round_tripped} of {}", paths.len());
    println!("densest table: {:.3} decoded values per input byte, {}", densest.0, densest.1);
    println!("table objects visited in originals: {tables}");
    println!(
        "size: {original_bytes} -> {output_bytes} bytes, overall ratio {:.4}",
        output_bytes as f64 / original_bytes as f64
    );
    for (ratio, from, to, name) in ratios.iter().take(3).chain(ratios.iter().rev().take(3)) {
        println!("  ratio {ratio:.4}  {from} -> {to}  {name}");
    }
    println!("unexplained bytes, all zero padding unless listed below: originals {padding_original}, output {padding_output}");
    println!("originals: non-canonical bool bytes {odd_bools}, non-finite floats {non_finite}, non-UTF-8 strings {non_utf8}");
    println!("originals failing the structural check: {}", original_problem_tables.len());
    for line in original_problem_tables.iter().take(10) {
        println!("  {line}");
    }
    println!("originals with unexplained non-zero bytes: {}", original_nonzero.len());
    for line in original_nonzero.iter().take(10) {
        println!("  {line}");
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(paths.len(), 290);
}

fn count_table_values(table: &TableValue) -> usize {
    1 + table.fields().filter_map(|(_, value)| value).map(count_values).sum::<usize>()
}

fn count_values(value: &Value) -> usize {
    match value {
        Value::Table(table) => count_table_values(table),
        Value::Struct(value) => 1 + value.fields().map(|(_, item)| count_values(item)).sum::<usize>(),
        Value::Vector(items) | Value::Array(items) => 1 + items.iter().map(count_values).sum::<usize>(),
        _ => 1,
    }
}

fn diff_values(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Table(a), Value::Table(b)) => diff_tables_into(path, a, b, out),
        (Value::Struct(a), Value::Struct(b)) => diff_structs(path, a, b, out),
        (Value::Vector(a), Value::Vector(b)) | (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!("{path}: length {} vs {}", a.len(), b.len()));
            }
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                diff_values(&format!("{path}[{index}]"), a, b, out);
            }
        }
        _ => {
            if a != b {
                out.push(format!("{path}: {a:?} vs {b:?}"));
            }
        }
    }
}

fn diff_structs(path: &str, a: &StructValue, b: &StructValue, out: &mut Vec<String>) {
    for ((field, a), (_, b)) in a.fields().zip(b.fields()) {
        diff_values(&format!("{path}.{}", field.name), a, b, out);
    }
}

fn diff_tables_into(path: &str, a: &TableValue, b: &TableValue, out: &mut Vec<String>) {
    for ((field, a), (_, b)) in a.fields().zip(b.fields()) {
        let here = format!("{path}.{}", field.name);
        match (a, b) {
            (Some(a), Some(b)) => diff_values(&here, a, b, out),
            (None, None) => {}
            (Some(_), None) => out.push(format!("{here}: present vs absent")),
            (None, Some(_)) => out.push(format!("{here}: absent vs present")),
        }
    }
}

fn diff_tables(a: &TableValue, b: &TableValue) -> Vec<String> {
    let mut out = Vec::new();
    diff_tables_into("", a, b, &mut out);
    out
}

/// Compares every schema field of `table` with the JSON row. A key the JSON omits means the field
/// holds its default, and a JSON key the schema lacks is a mismatch, so nothing is skipped silently.
fn compare_table(table: &TableValue, row: &Map<String, Json>, path: &str, errors: &mut Vec<String>) {
    for (field, value) in table.fields() {
        let here = format!("{path}.{}", field.name);
        compare_field(table.schema(), field, value, row.get(&field.name), &here, errors);
    }
    for key in row.keys() {
        if !table.def().fields.iter().any(|f| &f.name == key) {
            errors.push(format!("{path}: JSON key {key} is not in the schema"));
        }
    }
}

fn compare_struct(value: &StructValue, row: &Map<String, Json>, path: &str, errors: &mut Vec<String>) {
    for (field, item) in value.fields() {
        let here = format!("{path}.{}", field.name);
        compare_field(value.schema(), field, Some(item), row.get(&field.name), &here, errors);
    }
    for key in row.keys() {
        if !value.def().fields.iter().any(|f| &f.name == key) {
            errors.push(format!("{path}: JSON key {key} is not in the schema"));
        }
    }
}

fn compare_field(
    schema: &Schema,
    field: &FieldDef,
    value: Option<&Value>,
    json: Option<&Json>,
    path: &str,
    errors: &mut Vec<String>,
) {
    match field.ty {
        FieldType::Single(element) => {
            let value = value.or(field.default.as_ref());
            compare_element(schema, element, value, json, path, errors);
        }
        FieldType::Vector(element) | FieldType::Array(element, _) => {
            let items = value.and_then(Value::as_slice).unwrap_or(&[]);
            let expected = json.and_then(Json::as_array).map_or(&[][..], Vec::as_slice);
            if items.len() != expected.len() {
                errors.push(format!("{path}: {} elements, JSON has {}", items.len(), expected.len()));
                return;
            }
            for (index, (item, expected)) in items.iter().zip(expected).enumerate() {
                compare_element(schema, element, Some(item), Some(expected), &format!("{path}[{index}]"), errors);
            }
        }
    }
}

fn compare_element(
    schema: &Schema,
    element: ElementType,
    value: Option<&Value>,
    json: Option<&Json>,
    path: &str,
    errors: &mut Vec<String>,
) {
    let empty = Map::new();
    match element {
        ElementType::Scalar(ty, enum_id) => {
            let value = value.cloned().unwrap_or(match ty {
                ScalarType::Bool => Value::Bool(false),
                ScalarType::F32 | ScalarType::F64 => Value::Float(0.0),
                _ => Value::UInt(0),
            });
            let matches = match (&value, json) {
                (_, None) => match &value {
                    Value::Bool(v) => !*v,
                    Value::Int(v) => *v == 0,
                    Value::UInt(v) => *v == 0,
                    Value::Float(v) => *v == 0.0,
                    _ => false,
                },
                (Value::Bool(v), Some(Json::Bool(j))) => v == j,
                (Value::Int(v), Some(Json::Number(j))) => j.as_i64() == Some(*v),
                (Value::UInt(v), Some(Json::Number(j))) => j.as_u64() == Some(*v),
                (Value::Float(v), Some(Json::Number(j))) => {
                    j.as_f64().is_some_and(|j| (j as f32).to_bits() == (*v as f32).to_bits())
                }
                (Value::Int(_) | Value::UInt(_), Some(Json::String(name))) => {
                    let number = value.as_int().unwrap_or(i64::MAX);
                    enum_id.is_some_and(|id| {
                        schema.enum_at(id).values.iter().any(|(n, v)| n == name && *v == number)
                    })
                }
                _ => false,
            };
            if !matches {
                errors.push(format!("{path}: tree has {value:?}, JSON has {json:?}"));
            }
        }
        ElementType::Str => {
            let text = value.and_then(Value::as_str).unwrap_or("");
            let expected = json.and_then(Json::as_str).unwrap_or("");
            if text != expected {
                errors.push(format!("{path}: tree has {text:?}, JSON has {expected:?}"));
            }
        }
        ElementType::Table(_) => match (value.and_then(Value::as_table), json.and_then(Json::as_object)) {
            (Some(table), Some(row)) => compare_table(table, row, path, errors),
            (Some(table), None) => compare_table(table, &empty, path, errors),
            (None, None) => {}
            (None, Some(_)) => errors.push(format!("{path}: table is absent, JSON has it")),
        },
        ElementType::Struct(_) => match (value.and_then(Value::as_struct), json.and_then(Json::as_object)) {
            (Some(value), Some(row)) => compare_struct(value, row, path, errors),
            (Some(value), None) => compare_struct(value, &empty, path, errors),
            (None, None) => {}
            (None, Some(_)) => errors.push(format!("{path}: struct is absent, JSON has it")),
        },
    }
}

fn load_json(relative: &str) -> Json {
    serde_json::from_str(&std::fs::read_to_string(references().join(relative)).unwrap()).unwrap()
}

fn report_oracle(name: &str, rows: usize, errors: &[String]) {
    println!("oracle {name}: {rows} rows, {} mismatches", errors.len());
    for line in errors.iter().take(15) {
        println!("  {line}");
    }
    assert!(errors.is_empty(), "{name} disagrees with its oracle");
}

/// Maps the oracle's schema onto the measured layout, keeping its numbers. The oracle's reader
/// declares a dex field as {index, group}, so its `group` is whatever byte follows the ushort.
fn personal_row_to_measured_layout(row: &Map<String, Json>) -> Map<String, Json> {
    let mut row = row.clone();
    for key in ["paldea_dex", "kitakami_dex", "blueberry_dex"] {
        if let Some(index) = row.get(key).and_then(|dex| dex.get("index")).cloned() {
            row.insert(key.to_owned(), index);
        }
    }
    if let Some(species) = row.get_mut("species").and_then(Json::as_object_mut) {
        for (from, to) in [("model", "national_dex"), ("reserved", "reserved0")] {
            if let Some(value) = species.remove(from) {
                species.insert(to.to_owned(), value);
            }
        }
    }
    if let Some(moves) = row.get_mut("levelup_moves").and_then(Json::as_array_mut) {
        for entry in moves {
            let level = entry["level"].as_i64().unwrap();
            entry["level"] = json!(i64::from(level as u8 as i8));
        }
    }
    row
}

/// The oracle names StatAmps column-wise. Its numbers are right; its names and one sign are not.
fn waza_row_to_measured_layout(row: &Map<String, Json>) -> Map<String, Json> {
    let mut row = row.clone();
    let Some(amps) = row.get("StatAmps").and_then(Json::as_object).cloned() else {
        return row;
    };
    let order = [
        ("Stat1", "Stat1", false),
        ("Stat2", "Stat1Stage", false),
        ("Stat3", "Stat1Percent", true),
        ("Stat1Stage", "Stat2", false),
        ("Stat2Stage", "Stat2Stage", false),
        ("Stat3Stage", "Stat2Percent", true),
        ("Stat1Percent", "Stat3", false),
        ("Stat2Percent", "Stat3Stage", false),
        ("Stat3Percent", "Stat3Percent", true),
    ];
    let mut measured = Map::new();
    for (from, to, unsigned) in order {
        let raw = amps[from].as_i64().unwrap();
        let value = if unsigned { i64::from(raw as u8) } else { i64::from(raw as u8 as i8) };
        measured.insert(to.to_owned(), json!(value));
    }
    row.insert("StatAmps".to_owned(), Json::Object(measured));
    row
}

fn parse_fixture(dir: &Path, relative: &str) -> TableValue {
    let path = dir.join(relative);
    let schema = schema_for(&path);
    fb::parse(&schema, &std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn criterion_4_personal_matches_oracle() {
    let Some(dir) = fixtures() else { return };
    let tree = parse_fixture(&dir, "avalon/data/personal_array.bin");
    let oracle = load_json("SV_PERSONAL/personal_array_clean.json");
    let rows = oracle["entry"].as_array().unwrap();
    let parsed: Vec<&TableValue> = tree.tables("entry").collect();
    assert_eq!(parsed.len(), 1424);
    assert_eq!(rows.len(), 1424);
    let mut errors = Vec::new();
    for (index, (table, row)) in parsed.iter().zip(rows).enumerate() {
        let row = personal_row_to_measured_layout(row.as_object().unwrap());
        compare_table(table, &row, &format!("entry[{index}]"), &mut errors);
    }
    report_oracle("personal_array, every field", rows.len(), &errors);
}

#[test]
fn criterion_4_waza_matches_oracle() {
    let Some(dir) = fixtures() else { return };
    let tree = parse_fixture(&dir, "avalon/data/waza_array.bin");
    let oracle = load_json("SV_PERSONAL/waza_array_clean.json");
    let rows = oracle["Table"].as_array().unwrap();
    let parsed: Vec<&TableValue> = tree.tables("Table").collect();
    assert_eq!(parsed.len(), 920);
    assert_eq!(rows.len(), 920);
    let mut errors = Vec::new();
    for (index, (table, row)) in parsed.iter().zip(rows).enumerate() {
        let row = waza_row_to_measured_layout(row.as_object().unwrap());
        compare_table(table, &row, &format!("Table[{index}]"), &mut errors);
    }
    report_oracle("waza_array, every field", rows.len(), &errors);
}

#[test]
fn criterion_4_trainers_match_oracle() {
    let Some(dir) = fixtures() else { return };
    let tree = parse_fixture(&dir, "world/data/trainer/trdata/trdata_array.bin");
    let oracle = load_json("SV_TRAINERS/trdata_array_clean.json");
    let rows = oracle["values"].as_array().unwrap();
    let parsed: Vec<&TableValue> = tree.tables("values").collect();
    assert_eq!(parsed.len(), 756);
    assert_eq!(rows.len(), 756);
    let mut errors = Vec::new();
    for (index, (table, row)) in parsed.iter().zip(rows).enumerate() {
        compare_table(table, row.as_object().unwrap(), &format!("values[{index}]"), &mut errors);
    }
    report_oracle("trdata_array, every field including devId names", rows.len(), &errors);
}

#[test]
fn criterion_5_handwritten_schemas_explain_every_row() {
    let Some(dir) = fixtures() else { return };
    for (relative, row_object, expected_slots) in [
        ("avalon/data/personal_array.bin", "Personal", 28),
        ("avalon/data/waza_array.bin", "Waza", 73),
        ("avalon/data/tokusei_array.bin", "Tokusei", 9),
    ] {
        let path = dir.join(relative);
        let schema = schema_for(&path);
        let buffer = std::fs::read(&path).unwrap();
        let coverage = structural_check(&schema, &buffer);
        let longest = coverage.walk.max_slots[row_object];
        let fields = coverage.walk.field_counts[row_object];
        println!(
            "{relative}: {} tables, longest vtable {longest} slots, schema {fields} fields; \
             {} unexplained bytes, {} of them non-zero; {} structural problems",
            coverage.walk.tables,
            coverage.unexplained.len(),
            coverage.unexplained_nonzero.len(),
            coverage.walk.problems.len(),
        );
        assert_eq!(coverage.walk.unmapped_slots, 0, "{relative}: a row has a field the schema does not map");
        assert_eq!(longest, fields, "{relative}: the schema defines fields no row uses");
        assert_eq!(fields, expected_slots);
        assert!(coverage.unexplained_nonzero.is_empty(), "{relative}: {:?}", &coverage.unexplained_nonzero[..coverage.unexplained_nonzero.len().min(10)]);
        assert!(coverage.walk.problems.is_empty(), "{relative}: {:?}", coverage.walk.problems);
    }
}

fn trainers(dir: &Path) -> (Arc<Schema>, TableValue) {
    let path = dir.join("world/data/trainer/trdata/trdata_array.bin");
    let schema = schema_for(&path);
    let tree = fb::parse(&schema, &std::fs::read(path).unwrap()).unwrap();
    (schema, tree)
}

#[test]
fn criterion_6_mutating_a_present_field_changes_only_that_field() {
    let Some(dir) = fixtures() else { return };
    let (schema, original) = trainers(&dir);

    let magikarp = schema.enum_name("DevID", 129).expect("DevID 129").to_owned();
    assert!(magikarp.contains("KOIKI"), "DevID 129 is named {magikarp}");
    assert_eq!(schema.enum_value("DevID", &magikarp), Some(129));

    let mut edited = original.clone();
    let index = edited
        .tables("values")
        .position(|t| t.table("poke1").is_some_and(|p| p.int("devId").unwrap() != 129))
        .expect("a trainer whose first Pokemon is not Magikarp");
    let before = edited.tables("values").nth(index).unwrap().table("poke1").unwrap().int("devId").unwrap();
    edited.tables_mut("values").nth(index).unwrap().table_mut("poke1").unwrap().set_int("devId", 129).unwrap();

    let reparsed = fb::parse(&schema, &fb::serialize(&edited)).unwrap();
    let changes = diff_tables(&original, &reparsed);
    println!("trainer {index} poke1.devId {before} -> 129 ({magikarp}); diff against the original: {changes:?}");
    assert_eq!(reparsed, edited);
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert!(changes[0].starts_with(&format!(".values[{index}].poke1.devId:")), "{changes:?}");
    let row = reparsed.tables("values").nth(index).unwrap();
    assert_eq!(row.table("poke1").unwrap().int("devId").unwrap(), 129);
}

/// Every trainer keeps all six poke tables, with empty slots holding an absent devId, so a
/// one-Pokemon trainer gains a second one by setting absent scalars rather than adding a table.
#[test]
fn criterion_6_filling_an_empty_trainer_slot_makes_absent_fields_present() {
    let Some(dir) = fixtures() else { return };
    let (schema, original) = trainers(&dir);

    let missing_tables: Vec<usize> = ["poke1", "poke2", "poke3", "poke4", "poke5", "poke6"]
        .iter()
        .map(|slot| original.tables("values").filter(|t| t.table(slot).is_none()).count())
        .collect();
    println!("trainers without poke1..poke6 tables, out of 756: {missing_tables:?}");

    let index = original
        .tables("values")
        .position(|t| {
            t.table("poke1").is_some_and(|p| p.get("devId").is_some())
                && t.table("poke2").is_some_and(|p| p.get("devId").is_none() && p.get("level").is_none())
        })
        .expect("a trainer with one Pokemon");
    let trid = original.tables("values").nth(index).unwrap().string("trid").unwrap_or("").to_owned();

    let mut edited = original.clone();
    let slot = edited.tables_mut("values").nth(index).unwrap().table_mut("poke2").unwrap();
    assert_eq!(slot.int("devId").unwrap(), 0, "an absent devId reads its schema default");
    slot.set_int("devId", 129).unwrap();
    slot.set_int("level", 50).unwrap();

    let reparsed = fb::parse(&schema, &fb::serialize(&edited)).unwrap();
    let changes = diff_tables(&original, &reparsed);
    println!("trainer {index} ({trid}) gained poke2; diff against the original: {changes:?}");
    assert_eq!(reparsed, edited);
    assert_eq!(
        changes,
        [
            format!(".values[{index}].poke2.devId: absent vs present"),
            format!(".values[{index}].poke2.level: absent vs present"),
        ]
    );
    let row = reparsed.tables("values").nth(index).unwrap().table("poke2").unwrap();
    assert_eq!(row.int("devId").unwrap(), 129);
    assert_eq!(row.int("level").unwrap(), 50);
    assert_eq!(row.get("formId"), None, "untouched absent fields stay absent");
}

/// Creates the first absent sub-table found depth-first and returns its path.
fn force_first_absent_table(table: &mut TableValue, path: &str) -> Option<String> {
    let names: Vec<(String, FieldType)> = table.def().fields.iter().map(|f| (f.name.clone(), f.ty)).collect();
    for (name, ty) in &names {
        if matches!(ty, FieldType::Single(ElementType::Table(_))) && table.get(name).is_none() {
            table.table_mut(name).unwrap().def();
            return Some(format!("{path}.{name}"));
        }
    }
    for (name, ty) in &names {
        match ty {
            FieldType::Single(ElementType::Table(_)) => {
                let child = table.table_mut(name).unwrap();
                if let Some(found) = force_first_absent_table(child, &format!("{path}.{name}")) {
                    return Some(found);
                }
            }
            FieldType::Vector(ElementType::Table(_)) => {
                for (index, child) in table.tables_mut(name).enumerate() {
                    if let Some(found) = force_first_absent_table(child, &format!("{path}.{name}[{index}]")) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn criterion_6_forcing_an_absent_sub_table_present() {
    let Some(dir) = fixtures() else { return };
    let mut tried = 0;
    for path in files(&dir, "bin") {
        let schema = schema_for(&path);
        let original = fb::parse(&schema, &std::fs::read(&path).unwrap()).unwrap();
        let mut edited = original.clone();
        let Some(added) = force_first_absent_table(&mut edited, "") else {
            tried += 1;
            continue;
        };
        let reparsed = fb::parse(&schema, &fb::serialize(&edited)).unwrap();
        let changes = diff_tables(&original, &reparsed);
        println!(
            "{} ({tried} tables earlier had no absent sub-table): added {added}; diff: {changes:?}",
            path.strip_prefix(&dir).unwrap().display()
        );
        assert_eq!(reparsed, edited);
        assert_eq!(changes, [format!("{added}: absent vs present")]);
        return;
    }
    panic!("none of the {tried} tables has an absent sub-table to force present");
}

#[test]
fn malformed_real_tables_error_without_panicking() {
    let Some(dir) = fixtures() else { return };
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut outcomes = [0usize; 2];
    let mut schema_outcomes = [0usize; 2];
    for relative in [
        "avalon/data/tokusei_array.bin",
        "world/data/trainer/trdata/trdata_array.bin",
    ] {
        let path = dir.join(relative);
        let schema = schema_for(&path);
        let clean = std::fs::read(&path).unwrap();
        let cuts: Vec<usize> = (0..200).map(|_| rng.below(clean.len())).collect();
        for cut in cuts {
            outcomes[usize::from(fb::parse(&schema, &clean[..cut]).is_ok())] += 1;
        }
        for _ in 0..if clean.len() > 10_000 { 30 } else { 2000 } {
            let mut mutated = clean.clone();
            for _ in 0..1 + rng.below(8) {
                let at = rng.below(mutated.len());
                mutated[at] = rng.next() as u8;
            }
            outcomes[usize::from(fb::parse(&schema, &mutated).is_ok())] += 1;
        }
    }
    let bfbs = std::fs::read(dir.join("world/data/trainer/trdata/trdata_array.bfbs")).unwrap();
    let table = std::fs::read(dir.join("world/data/trainer/trdata/trdata_array.bin")).unwrap();
    for _ in 0..300 {
        let mut mutated = bfbs.clone();
        for _ in 0..1 + rng.below(8) {
            let at = rng.below(mutated.len());
            mutated[at] = rng.next() as u8;
        }
        match Schema::from_bfbs(&mutated) {
            Ok(schema) => {
                schema_outcomes[1] += 1;
                if let Ok(tree) = fb::parse(&schema, &table) {
                    let _ = fb::try_serialize(&tree);
                }
            }
            Err(_) => schema_outcomes[0] += 1,
        }
    }
    println!("mutated tables: {} errors, {} still parsed", outcomes[0], outcomes[1]);
    println!("mutated schemas: {} errors, {} still parsed", schema_outcomes[0], schema_outcomes[1]);
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

const ALL_CONSTRUCTS: &str = r#"
    // A line comment.
    /* A block
       comment. */
    namespace test.ns;
    attribute "custom";
    file_extension "bin";
    file_identifier "TEST";

    enum Color : ubyte { Red, Green = 5, Blue }
    enum Wide : long { Small = -1, Big = 4000000000 }

    struct Inner { a: ubyte; b: double; }
    struct Outer {
        flag: bool;
        inner: Inner;
        values: [short:3];
        inners: [Inner:2];
        tail: ubyte;
    }

    table Leaf (custom) {
        name: string (required);
        hash: ulong;
    }

    table Root {
        byte_first: ubyte = 7;
        big: long;
        real: float = 1.5;
        precise: double;
        color: Color = Green;
        wide: Wide = Big;
        enabled: bool = true;
        text: string;
        leaf: Leaf;
        outer: Outer;
        leaves: [Leaf];
        names: [string];
        numbers: [ulong];
        small_numbers: [ubyte];
        structs: [Inner];
        old_field: int (deprecated);
        self_ref: Root;
    }

    root_type Root;
"#;

fn sample_tree() -> (Arc<Schema>, TableValue) {
    let schema = Schema::from_fbs_text(ALL_CONSTRUCTS).unwrap();
    let mut root = seed_root(&schema);
    root.set_int("byte_first", 7).unwrap();
    root.set_int("big", -5).unwrap();
    root.set_float("real", 2.25).unwrap();
    root.set_float("precise", f64::MIN_POSITIVE).unwrap();
    root.set_bool("enabled", false).unwrap();
    root.set_string("text", "héllo").unwrap();
    root.table_mut("leaf").unwrap().set_string("name", "").unwrap();
    root.table_mut("leaf").unwrap().set_uint("hash", u64::MAX).unwrap();
    {
        let outer = root.struct_mut("outer").unwrap();
        outer.set_bool("flag", true).unwrap();
        outer.struct_mut("inner").unwrap().set_float("b", -0.5).unwrap();
        outer.array_mut("values").unwrap()[1] = Value::Int(-300);
        outer.array_mut("inners").unwrap()[1].as_struct_mut().unwrap().set_int("a", 200).unwrap();
        outer.set_int("tail", 9).unwrap();
    }
    let mut leaf = root.new_table("leaves").unwrap();
    leaf.set_string("name", "row 0").unwrap();
    root.vector_mut("leaves").unwrap().push(Value::Table(leaf));
    root.vector_mut("names").unwrap().extend([Value::Str("a".into()), Value::Str(String::new())]);
    root.vector_mut("numbers").unwrap().push(Value::UInt(1 << 40));
    root.vector_mut("small_numbers").unwrap();
    let mut item = root.new_struct("structs").unwrap();
    item.set_int("a", 1).unwrap();
    root.vector_mut("structs").unwrap().push(Value::Struct(item));
    root.table_mut("self_ref").unwrap().table_mut("self_ref").unwrap().set_int("big", 1).unwrap();
    (schema, root)
}

fn seed_root(schema: &Arc<Schema>) -> TableValue {
    let buffer = minimal_buffer(schema);
    fb::parse(schema, &buffer).unwrap()
}

/// An empty root table, written by hand so the writer is not trusted to create its own input.
fn minimal_buffer(schema: &Schema) -> Vec<u8> {
    let mut buffer = vec![16u8, 0, 0, 0];
    buffer.extend_from_slice(&schema.file_ident().unwrap_or([0; 4]));
    buffer.extend_from_slice(&[4, 0, 4, 0, 0, 0, 0, 0]);
    buffer.extend_from_slice(&[8, 0, 0, 0]);
    buffer
}

#[test]
fn every_construct_round_trips_on_a_synthetic_table() {
    let (schema, root) = sample_tree();
    let bytes = fb::serialize(&root);
    assert_eq!(&bytes[4..8], b"TEST", "the declared file identifier is written");
    assert_eq!(fb::serialize(&root), bytes);
    let reparsed = fb::parse(&schema, &bytes).unwrap();
    assert_eq!(reparsed, root, "{:?}", diff_tables(&root, &reparsed));

    let coverage = structural_check(&schema, &bytes);
    assert_eq!(coverage.walk.problems, Vec::<String>::new());
    assert_eq!(coverage.unexplained_nonzero, Vec::<usize>::new());
    assert_eq!(bytes.len() % 8, 0, "the buffer ends on its largest alignment");

    assert_eq!(reparsed.table("leaf").unwrap().uint("hash").unwrap(), u64::MAX);
    assert!(matches!(reparsed.table("leaf").unwrap().int("hash"), Err(fb::FieldError::OutOfRange { .. })));
    assert_eq!(reparsed.string("text"), Some("héllo"));
    assert_eq!(reparsed.vector("names").unwrap().len(), 2);
    assert_eq!(reparsed.vector("small_numbers").unwrap().len(), 0);
    let outer = reparsed.struct_value("outer").unwrap();
    assert_eq!(outer.array("values").unwrap()[1], Value::Int(-300));
    assert_eq!(outer.array("inners").unwrap()[1].as_struct().unwrap().int("a").unwrap(), 200);
    assert_eq!(outer.struct_value("inner").unwrap().float("b").unwrap(), -0.5);
}

#[test]
fn defaults_read_when_absent_and_present_defaults_stay_present() {
    let (schema, root) = sample_tree();
    let empty = seed_root(&schema);

    assert_eq!(empty.get("color"), None);
    assert_eq!(empty.int("color").unwrap(), 5);
    assert_eq!(empty.int("wide").unwrap(), 4_000_000_000);
    assert_eq!(empty.float("real").unwrap(), 1.5);
    assert!(empty.bool("enabled").unwrap());
    assert_eq!(empty.int("byte_first").unwrap(), 7);

    let reparsed = fb::parse(&schema, &fb::serialize(&root)).unwrap();
    assert_eq!(reparsed.get("byte_first"), Some(&Value::UInt(7)), "present and equal to its default");
    assert_eq!(reparsed.get("color"), None, "absent stays absent");
    assert_eq!(reparsed.int("color").unwrap(), 5);
}

#[test]
fn setters_enforce_field_names_kinds_and_widths() {
    let (_, mut root) = sample_tree();
    assert!(matches!(root.int("nope"), Err(fb::FieldError::Unknown { .. })));
    assert!(matches!(root.set_int("nope", 1), Err(fb::FieldError::Unknown { .. })));
    assert!(matches!(root.set_int("byte_first", 256), Err(fb::FieldError::OutOfRange { .. })));
    assert!(matches!(root.set_int("byte_first", -1), Err(fb::FieldError::OutOfRange { .. })));
    assert!(root.set_int("byte_first", 255).is_ok());
    assert!(matches!(root.set_int("text", 1), Err(fb::FieldError::TypeMismatch { .. })));
    assert!(matches!(root.set_int("real", 1), Err(fb::FieldError::TypeMismatch { .. })));
    assert!(matches!(root.set_bool("big", true), Err(fb::FieldError::TypeMismatch { .. })));
    assert!(matches!(root.set_string("big", "x"), Err(fb::FieldError::TypeMismatch { .. })));
    assert!(matches!(root.table_mut("big"), Err(fb::FieldError::TypeMismatch { .. })));
    assert!(matches!(root.vector_mut("big"), Err(fb::FieldError::TypeMismatch { .. })));
    assert!(matches!(root.struct_mut("leaf"), Err(fb::FieldError::TypeMismatch { .. })));
    assert!(matches!(root.int("text"), Err(fb::FieldError::TypeMismatch { .. })));
}

#[test]
fn try_serialize_rejects_trees_that_break_the_schema() {
    let (_, mut root) = sample_tree();
    root.vector_mut("numbers").unwrap().push(Value::Str("not a number".into()));
    assert!(matches!(fb::try_serialize(&root), Err(SerializeError::TypeMismatch { .. })));

    let (_, mut root) = sample_tree();
    root.vector_mut("small_numbers").unwrap().push(Value::UInt(300));
    assert!(matches!(fb::try_serialize(&root), Err(SerializeError::OutOfRange { .. })));

    let (schema, mut root) = sample_tree();
    let row = fb::parse(&schema, &minimal_buffer(&schema)).unwrap().new_table("leaves").unwrap();
    root.vector_mut("leaves").unwrap().push(Value::Table(row));
    assert!(matches!(fb::try_serialize(&root), Err(SerializeError::MissingRequired { .. })));

    let (_, mut root) = sample_tree();
    let foreign = builtin::tokusei();
    let stray = fb::parse(&foreign, &minimal_buffer(&foreign)).unwrap();
    root.vector_mut("leaves").unwrap().push(Value::Table(stray));
    assert!(matches!(fb::try_serialize(&root), Err(SerializeError::TypeMismatch { .. })));
}

#[test]
fn malformed_input_errors_without_panicking() {
    let schema = Schema::from_fbs_text(ALL_CONSTRUCTS).unwrap();
    let (_, root) = sample_tree();
    let clean = fb::serialize(&root);

    for cut in 0..clean.len() {
        let _ = fb::parse(&schema, &clean[..cut]);
    }
    let mut rng = XorShift(0xDEAD_BEEF_CAFE_F00D);
    for _ in 0..20_000 {
        let mut mutated = clean.clone();
        for _ in 0..1 + rng.below(6) {
            let at = rng.below(mutated.len());
            mutated[at] = rng.next() as u8;
        }
        let _ = fb::parse(&schema, &mutated);
    }
    for _ in 0..20_000 {
        let mut text = ALL_CONSTRUCTS.as_bytes().to_vec();
        for _ in 0..1 + rng.below(6) {
            if text.is_empty() {
                break;
            }
            let at = rng.below(text.len());
            match rng.below(3) {
                0 => text[at] = b"{}()[]:;,=\"/*-0123456789abz ".get(rng.below(29)).copied().unwrap_or(b' '),
                1 => drop(text.remove(at)),
                _ => text.truncate(at.max(1)),
            }
        }
        if let Ok(mutated) = Schema::from_fbs_text(&String::from_utf8_lossy(&text)) {
            if let Ok(tree) = fb::parse(&mutated, &clean) {
                let _ = fb::try_serialize(&tree);
            }
        }
    }
    assert!(matches!(fb::parse(&schema, &[]), Err(ParseError::Wire(_))));
    assert!(matches!(Schema::from_bfbs(&[]), Err(SchemaError::NotBfbs { .. })));
    assert!(matches!(Schema::from_bfbs(b"\x00\x00\x00\x00BFBS"), Err(SchemaError::Wire(_))));

    let mut other_ident = clean.clone();
    other_ident[4] = b'X';
    assert!(matches!(fb::parse(&schema, &other_ident), Err(ParseError::IdentifierMismatch { .. })));
}

#[test]
fn unmapped_vtable_slots_are_an_error_not_silent_data_loss() {
    let schema = Schema::from_fbs_text("table R { a: ubyte; } root_type R;").unwrap();
    let mut buffer = vec![12u8, 0, 0, 0];
    buffer.extend_from_slice(&[8, 0, 6, 0, 4, 0, 5, 0]);
    buffer.extend_from_slice(&[8, 0, 0, 0, 1, 2, 0, 0]);
    assert!(matches!(fb::parse(&schema, &buffer), Err(ParseError::UnmappedField { slot: 1, .. })));

    let mut buffer = vec![12u8, 0, 0, 0];
    buffer.extend_from_slice(&[8, 0, 6, 0, 4, 0, 0, 0]);
    buffer.extend_from_slice(&[8, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(fb::parse(&schema, &buffer).unwrap().int("a").unwrap(), 1, "a zero trailing slot is not data");
}

#[test]
fn crafted_expansion_and_depth_are_rejected() {
    let schema = Schema::from_fbs_text("table R { kids: [R]; } root_type R;").unwrap();

    // Every table's child vector holds several offsets to the same next table, so the tree is
    // 8^40 nodes while the buffer stays under 2 KiB.
    let mut buffer = vec![12u8, 0, 0, 0];
    buffer.extend_from_slice(&[6, 0, 8, 0, 4, 0, 0, 0]);
    let levels = 40usize;
    let fan_out = 8usize;
    let level_size = 12 + 4 * fan_out;
    let first_table = buffer.len();
    for level in 0..levels {
        let table = first_table + level * level_size;
        let soffset = (table - 4) as i32;
        buffer.extend_from_slice(&soffset.to_le_bytes());
        buffer.extend_from_slice(&4u32.to_le_bytes());
        buffer.extend_from_slice(&(fan_out as u32).to_le_bytes());
        for slot in 0..fan_out {
            let at = table + 12 + 4 * slot;
            let next = first_table + (level + 1) * level_size;
            buffer.extend_from_slice(&((next - at) as u32).to_le_bytes());
        }
    }
    buffer.extend_from_slice(&((first_table + levels * level_size - 4) as i32).to_le_bytes());
    buffer.extend_from_slice(&4u32.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(fb::parse(&schema, &buffer).unwrap_err(), ParseError::TooManyValues);

    let mut deep = vec![12u8, 0, 0, 0];
    deep.extend_from_slice(&[6, 0, 8, 0, 4, 0, 0, 0]);
    for _ in 0..200 {
        let table = deep.len();
        deep.extend_from_slice(&((table - 4) as i32).to_le_bytes());
        deep.extend_from_slice(&4u32.to_le_bytes());
        deep.extend_from_slice(&1u32.to_le_bytes());
        deep.extend_from_slice(&4u32.to_le_bytes());
    }
    assert_eq!(fb::parse(&schema, &deep).unwrap_err(), ParseError::TooDeep);
}

#[test]
fn structural_check_detects_broken_buffers() {
    let (schema, root) = sample_tree();
    let clean = fb::serialize(&root);
    let check = structural_check(&schema, &clean);
    assert!(check.walk.problems.is_empty() && check.unexplained_nonzero.is_empty());

    let mut wild_root = clean.clone();
    wild_root[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(!structural_check(&schema, &wild_root).walk.problems.is_empty(), "a root past the end");

    assert!(!structural_check(&schema, &clean[..clean.len() / 2]).walk.problems.is_empty(), "a truncated buffer");

    // Offsets are relative, so one inserted byte leaves the tree readable and every field misaligned.
    let mut shifted = clean.clone();
    shifted.insert(8, 0);
    let root_offset = u32::from_le_bytes(shifted[0..4].try_into().unwrap());
    shifted[0..4].copy_from_slice(&(root_offset + 1).to_le_bytes());
    let check = structural_check(&schema, &shifted);
    assert!(check.walk.problems.iter().any(|p| p.contains("not aligned")), "{:?}", check.walk.problems);

    let needle = "héllo".as_bytes();
    let at = clean.windows(needle.len()).position(|w| w == needle).unwrap();
    let mut unterminated = clean.clone();
    unterminated[at + needle.len()] = b'!';
    let check = structural_check(&schema, &unterminated);
    assert!(check.walk.problems.iter().any(|p| p.contains("NUL")), "{:?}", check.walk.problems);

    let padding = structural_check(&schema, &clean).unexplained[0];
    let mut hidden = clean.clone();
    hidden[padding] = 0x7F;
    assert_eq!(structural_check(&schema, &hidden).unexplained_nonzero, [padding], "data hidden in padding");

    let narrow = Schema::from_fbs_text("table R { a: ubyte; } root_type R;").unwrap();
    let mut extra_slot = vec![12u8, 0, 0, 0];
    extra_slot.extend_from_slice(&[8, 0, 6, 0, 4, 0, 5, 0]);
    extra_slot.extend_from_slice(&[8, 0, 0, 0, 1, 2, 0, 0]);
    assert_eq!(structural_check(&narrow, &extra_slot).walk.unmapped_slots, 1, "a slot the schema lacks");
}

#[test]
fn oracle_comparison_detects_differences() {
    let schema = Schema::from_fbs_text(
        "enum C : ubyte { A = 1, B = 2 }
         struct S { x: ubyte; }
         table Sub { y: ushort; }
         table R { n: ushort; c: C; s: S; t: string; v: [ubyte]; b: bool; sub: Sub; }
         root_type R;",
    )
    .unwrap();
    let mut table = fb::parse(&schema, &minimal_buffer(&schema)).unwrap();
    table.set_int("n", 7).unwrap();
    table.set_int("c", 2).unwrap();
    table.struct_mut("s").unwrap().set_int("x", 3).unwrap();
    table.set_string("t", "text").unwrap();
    table.vector_mut("v").unwrap().push(Value::UInt(9));
    table.set_bool("b", true).unwrap();
    table.table_mut("sub").unwrap().set_int("y", 4).unwrap();

    let good = json!({"n": 7, "c": "B", "s": {"x": 3}, "t": "text", "v": [9], "b": true, "sub": {"y": 4}});
    let errors_for = |row: Json| {
        let mut errors = Vec::new();
        compare_table(&table, row.as_object().unwrap(), "", &mut errors);
        errors
    };
    assert_eq!(errors_for(good.clone()), Vec::<String>::new());

    let mutations: [(&str, Json); 9] = [
        ("wrong number", json!(8)),
        ("wrong enum name", json!("A")),
        ("wrong struct field", json!({"x": 4})),
        ("wrong string", json!("other")),
        ("wrong vector", json!([9, 9])),
        ("wrong bool", json!(false)),
        ("wrong sub-table", json!({"y": 5})),
        ("extra key in sub-table", json!({"y": 4, "z": 1})),
        ("number for an enum name", json!(1)),
    ];
    let keys = ["n", "c", "s", "t", "v", "b", "sub", "sub", "c"];
    for ((why, replacement), key) in mutations.into_iter().zip(keys) {
        let mut row = good.clone();
        row[key] = replacement;
        assert!(!errors_for(row).is_empty(), "should notice: {why}");
    }
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("n");
    assert!(!errors_for(missing).is_empty(), "should notice a non-default field the JSON omits");
    let mut extra = good.clone();
    extra["unknown"] = json!(1);
    assert!(!errors_for(extra).is_empty(), "should notice a JSON key the schema lacks");
}

const DEEP_STRUCTS: &str = "struct A0 { n: A1; } struct A1 { n: A2; } struct A2 { n: A3; } struct A3 { n: A4; }
    struct A4 { n: A5; } struct A5 { n: A6; } struct A6 { n: A7; } struct A7 { n: A8; } struct A8 { n: A9; }
    struct A9 { n: B0; } struct B0 { n: B1; } struct B1 { n: B2; } struct B2 { n: B3; } struct B3 { n: B4; }
    struct B4 { n: B5; } struct B5 { n: B6; } struct B6 { n: B7; } struct B7 { n: B8; } struct B8 { n: B9; }
    struct B9 { n: C0; } struct C0 { n: C1; } struct C1 { n: C2; } struct C2 { n: C3; } struct C3 { n: C4; }
    struct C4 { n: C5; } struct C5 { n: C6; } struct C6 { n: C7; } struct C7 { n: C8; } struct C8 { n: C9; }
    struct C9 { n: D0; } struct D0 { n: D1; } struct D1 { n: D2; } struct D2 { n: D3; } struct D3 { n: D4; }
    struct D4 { n: D5; } struct D5 { n: D6; } struct D6 { n: D7; } struct D7 { n: D8; } struct D8 { n: D9; }
    struct D9 { n: E0; } struct E0 { n: E1; } struct E1 { n: E2; } struct E2 { n: E3; } struct E3 { n: E4; }
    struct E4 { n: E5; } struct E5 { n: E6; } struct E6 { n: E7; } struct E7 { n: E8; } struct E8 { n: E9; }
    struct E9 { n: F0; } struct F0 { n: F1; } struct F1 { n: F2; } struct F2 { n: F3; } struct F3 { n: F4; }
    struct F4 { n: F5; } struct F5 { n: F6; } struct F6 { n: F7; } struct F7 { n: F8; } struct F8 { n: F9; }
    struct F9 { n: G0; } struct G0 { n: G1; } struct G1 { n: G2; } struct G2 { n: G3; } struct G3 { n: G4; }
    struct G4 { n: G5; } struct G5 { n: G6; } struct G6 { n: G7; } struct G7 { x: ubyte; }
    table R { s: A0; } root_type R;";

#[test]
fn text_schemas_reject_what_they_cannot_honor() {
    let bad = [
        ("table R { a: Missing; } root_type R;", "unknown type"),
        ("table R { a: int; }", "no root_type"),
        ("struct S { a: S; } table R { s: S; } root_type R;", "struct contains itself"),
        ("struct S { a: string; } table R { s: S; } root_type R;", "offset in a struct"),
        ("struct S { a: [int]; } table R { s: S; } root_type R;", "vector in a struct"),
        ("table R { a: [int:3]; } root_type R;", "array in a table"),
        ("table R { a: int (id: 1); } root_type R;", "explicit ids would change slots"),
        ("struct S (force_align: 16) { a: int; } table R { s: S; } root_type R;", "force_align changes layout"),
        ("union U { R } table R { u: U; } root_type R;", "unions"),
        (DEEP_STRUCTS, "structs nested past the limit"),
        ("table R { a: ubyte = 300; } root_type R;", "default too large"),
        ("enum E : ubyte { A = 300 } table R { a: E; } root_type R;", "enum value too large"),
        ("struct S { a: [ulong:65535]; } table R { s: S; } root_type R;", "struct beyond a vtable offset"),
        ("table R { a: string = oops; } root_type R;", "default on a string"),
        ("table R { a: int; ", "unterminated table"),
        ("/* never closed", "unterminated comment"),
        ("file_identifier \"TOOLONG\"; table R { a: int; } root_type R;", "identifier length"),
    ];
    for (text, why) in bad {
        assert!(Schema::from_fbs_text(text).is_err(), "should reject: {why}");
    }
}

/// A schema whose first struct nests a second struct declaring alignment 0, the case where layout
/// checks must not divide by a value they have not validated yet.
#[test]
fn binary_schema_with_zero_alignment_is_rejected() {
    let schema = bfbs_with_nested_struct_alignment(0);
    assert!(matches!(Schema::from_bfbs(&schema), Err(SchemaError::Invalid(_))));
    let sane = bfbs_with_nested_struct_alignment(1);
    Schema::from_bfbs(&sane).unwrap();
}

fn bfbs_with_nested_struct_alignment(align: i32) -> Vec<u8> {
    let mut writer = MiniBfbs::default();
    writer.build(align)
}

/// Writes reflection tables by hand, front to back, so this input does not depend on the crate's
/// writer. Only the slots the loader reads are present.
#[derive(Default)]
struct MiniBfbs {
    out: Vec<u8>,
}

impl MiniBfbs {
    fn align(&mut self, align: usize) {
        while self.out.len() % align != 0 {
            self.out.push(0);
        }
    }

    fn put_u32(&mut self, pos: usize, value: u32) {
        self.out[pos..pos + 4].copy_from_slice(&value.to_le_bytes());
    }

    /// Lays out a table from (slot, size, align) triples and returns its position and field positions.
    fn table(&mut self, slots: usize, fields: &[(usize, usize, usize)]) -> (usize, Vec<usize>) {
        let mut offsets = vec![0u16; slots];
        let mut size = 4usize;
        let mut positions = Vec::new();
        for &(slot, field_size, align) in fields {
            size = size.next_multiple_of(align);
            offsets[slot] = size as u16;
            positions.push(size);
            size += field_size;
        }
        self.align(2);
        let vtable = self.out.len();
        self.out.extend_from_slice(&((4 + 2 * slots) as u16).to_le_bytes());
        self.out.extend_from_slice(&(size as u16).to_le_bytes());
        for offset in offsets {
            self.out.extend_from_slice(&offset.to_le_bytes());
        }
        self.align(8);
        let table = self.out.len();
        self.out.resize(table + size, 0);
        self.out[table..table + 4].copy_from_slice(&((table - vtable) as i32).to_le_bytes());
        (table, positions.into_iter().map(|p| table + p).collect())
    }

    fn string(&mut self, offset_pos: usize, text: &str) {
        self.align(4);
        let pos = self.out.len();
        self.out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        self.out.extend_from_slice(text.as_bytes());
        self.out.push(0);
        self.put_u32(offset_pos, (pos - offset_pos) as u32);
    }

    fn vector(&mut self, offset_pos: usize, count: usize) -> Vec<usize> {
        self.align(4);
        let pos = self.out.len();
        self.out.extend_from_slice(&(count as u32).to_le_bytes());
        self.out.resize(pos + 4 + 4 * count, 0);
        self.put_u32(offset_pos, (pos - offset_pos) as u32);
        (0..count).map(|i| pos + 4 + 4 * i).collect()
    }

    fn link(&mut self, offset_pos: usize, target: usize) {
        self.put_u32(offset_pos, (target - offset_pos) as u32);
    }

    /// Three objects: Holder (struct containing Bad), Bad (struct), Root (table).
    fn build(&mut self, bad_align: i32) -> Vec<u8> {
        self.out.extend_from_slice(&[0, 0, 0, 0]);
        self.out.extend_from_slice(b"BFBS");
        let (schema, schema_fields) = self.table(5, &[(0, 4, 4), (4, 4, 4)]);
        self.link(0, schema);
        let [objects_field, root_field] = schema_fields[..] else { unreachable!() };
        let object_slots = self.vector(objects_field, 3);

        let holder = self.object("Holder", true, 8, 1, Some((1, 1)));
        let bad = self.object("Bad", true, 8, bad_align, None);
        let root = self.object("Root", false, 0, 0, None);
        for (slot, target) in object_slots.iter().zip([holder, bad, root]) {
            self.link(*slot, target);
        }
        self.link(root_field, root);
        std::mem::take(&mut self.out)
    }

    fn object(&mut self, name: &str, is_struct: bool, bytesize: i32, minalign: i32, field: Option<(i32, u16)>) -> usize {
        let (object, slots) = self.table(5, &[(2, 1, 1), (3, 4, 4), (4, 4, 4), (0, 4, 4), (1, 4, 4)]);
        let [is_struct_pos, minalign_pos, bytesize_pos, name_pos, fields_pos] = slots[..] else { unreachable!() };
        self.out[is_struct_pos] = u8::from(is_struct);
        self.out[minalign_pos..minalign_pos + 4].copy_from_slice(&minalign.to_le_bytes());
        self.out[bytesize_pos..bytesize_pos + 4].copy_from_slice(&bytesize.to_le_bytes());
        self.string(name_pos, name);
        let count = usize::from(field.is_some());
        let entries = self.vector(fields_pos, count);
        if let Some((index, _)) = field {
            let (table, positions) = self.table(2, &[(0, 4, 4), (1, 4, 4)]);
            let [field_name, type_pos] = positions[..] else { unreachable!() };
            self.string(field_name, "nested");
            let (ty, ty_fields) = self.table(3, &[(0, 1, 1), (2, 4, 4)]);
            let [base_pos, index_pos] = ty_fields[..] else { unreachable!() };
            self.out[base_pos] = 15;
            self.out[index_pos..index_pos + 4].copy_from_slice(&index.to_le_bytes());
            self.link(type_pos, ty);
            self.link(entries[0], table);
        }
        object
    }
}

#[test]
fn struct_layout_follows_flatbuffers_rules() {
    let schema = Schema::from_fbs_text(
        "struct A { x: ubyte; y: uint; z: ubyte; }
         struct B { a: A; f: [ushort:3]; d: double; }
         table R { b: B; } root_type R;",
    )
    .unwrap();
    let object = |name: &str| schema.objects().iter().find(|o| o.name == name).unwrap();
    assert_eq!(object("A").kind, ObjectKind::Struct { size: 12, align: 4 });
    assert_eq!(object("A").fields.iter().map(|f| f.offset).collect::<Vec<_>>(), [0, 4, 8]);
    assert_eq!(object("B").kind, ObjectKind::Struct { size: 32, align: 8 });
    assert_eq!(object("B").fields.iter().map(|f| f.offset).collect::<Vec<_>>(), [0, 12, 24]);
}

#[test]
fn enum_lookup_handles_namespaces_and_ambiguity() {
    let schema = Schema::from_fbs_text(
        "namespace a; enum E : ubyte { X = 1, Y = 2 }
         namespace b; enum E : ubyte { X = 3 }
         namespace c; enum Only : byte { Neg = -4 }
         table R { e: a.E; } root_type R;",
    )
    .unwrap();
    assert_eq!(schema.enum_value("a.E", "Y"), Some(2));
    assert_eq!(schema.enum_value("E", "X"), None, "two enums match, so the short name is ambiguous");
    assert_eq!(schema.enum_value("Only", "Neg"), Some(-4));
    assert_eq!(schema.enum_name("c.Only", -4), Some("Neg"));
    assert_eq!(schema.enum_name("Only", 0), None);
    assert_eq!(schema.enum_entries("b.E"), Some(vec![("X".to_owned(), 3)]));
    assert_eq!(schema.enum_entries("Nope"), None);
}

#[test]
fn builtin_schemas_load_with_the_measured_shape() {
    let personal = builtin::personal();
    assert_eq!(personal.object_at(personal.root_id()).name, "PersonalTable");
    let row = personal.objects().iter().find(|o| o.name == "Personal").unwrap();
    assert_eq!(row.fields.len(), 28);
    let species = personal.objects().iter().find(|o| o.name == "PersonalSpecies").unwrap();
    assert_eq!(species.kind, ObjectKind::Struct { size: 24, align: 4 });
    let learn = personal.objects().iter().find(|o| o.name == "PersonalLevelUpMove").unwrap();
    assert_eq!(learn.kind, ObjectKind::Struct { size: 4, align: 2 });

    let waza = builtin::waza();
    assert_eq!(waza.objects().iter().find(|o| o.name == "Waza").unwrap().fields.len(), 73);
    assert_eq!(waza.enum_value("WazaAffinity", "Attack"), Some(5));

    let tokusei = builtin::tokusei();
    assert_eq!(tokusei.objects().iter().find(|o| o.name == "Tokusei").unwrap().fields.len(), 9);
}
