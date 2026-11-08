use super::TextError;

const KEY_BASE: u16 = 0x7C89;
const KEY_ADVANCE: u16 = 0x2983;
const VARIABLE: u16 = 0x0010;
const TERMINATOR: u16 = 0x0000;
const TEXT_RETURN: u16 = 0xBE00;
const TEXT_CLEAR: u16 = 0xBE01;
const SECTION_OFFSET: usize = 0x10;

/// A Game Freak `.dat` line table. Each line is kept as its decrypted UTF-16 code units,
/// terminator included, so rewriting reproduces the original bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextFile {
    lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Line {
    units: Vec<u16>,
    flags: u16,
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn line_key(index: usize) -> u16 {
    KEY_BASE.wrapping_add(KEY_ADVANCE.wrapping_mul(index as u16))
}

fn crypt(units: &mut [u16], mut key: u16) {
    for unit in units {
        *unit ^= key;
        key = key.rotate_left(3);
    }
}

impl TextFile {
    pub fn parse(dat: &[u8]) -> Result<TextFile, TextError> {
        let bad = |detail: &str| TextError::BadDat(detail.to_owned());
        let header = |offset| read_u32(dat, offset).ok_or_else(|| bad("header is truncated"));
        let sections = read_u16(dat, 0).ok_or_else(|| bad("header is truncated"))?;
        let count = usize::from(read_u16(dat, 2).ok_or_else(|| bad("header is truncated"))?);
        let total = header(4)?;
        let initial_key = header(8)?;
        let section_offset = header(12)?;
        if sections != 1 || initial_key != 0 || section_offset as usize != SECTION_OFFSET {
            return Err(bad("unexpected header values"));
        }
        if SECTION_OFFSET + total as usize != dat.len() {
            return Err(bad("total length does not match the file size"));
        }
        let section_len = read_u32(dat, SECTION_OFFSET).ok_or_else(|| bad("section is truncated"))?;
        if section_len != total {
            return Err(bad("section length does not match the total length"));
        }

        let table = SECTION_OFFSET + 4;
        let truncated = || bad("line table is truncated");
        let mut lines = Vec::with_capacity(count);
        for index in 0..count {
            let entry = table + index * 8;
            let offset = read_u32(dat, entry).ok_or_else(truncated)?;
            let length = usize::from(read_u16(dat, entry + 4).ok_or_else(truncated)?);
            let flags = read_u16(dat, entry + 6).ok_or_else(truncated)?;
            let start = SECTION_OFFSET + offset as usize;
            let bytes = start
                .checked_add(length * 2)
                .and_then(|end| dat.get(start..end))
                .ok_or_else(|| bad("a line lies outside the file"))?;
            let mut units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            crypt(&mut units, line_key(index));
            lines.push(Line { units, flags });
        }
        Ok(TextFile { lines })
    }

    /// Lays lines out back to back, padded to 4 bytes, which is how the game files are written.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut section = vec![0u8; 4 + self.lines.len() * 8];
        for (index, line) in self.lines.iter().enumerate() {
            let entry = 4 + index * 8;
            let offset = section.len() as u32;
            section[entry..entry + 4].copy_from_slice(&offset.to_le_bytes());
            section[entry + 4..entry + 6].copy_from_slice(&(line.units.len() as u16).to_le_bytes());
            section[entry + 6..entry + 8].copy_from_slice(&line.flags.to_le_bytes());

            let mut units = line.units.clone();
            crypt(&mut units, line_key(index));
            for unit in units {
                section.extend_from_slice(&unit.to_le_bytes());
            }
            if section.len() % 4 == 2 {
                section.extend_from_slice(&[0, 0]);
            }
        }
        let total = section.len() as u32;
        section[..4].copy_from_slice(&total.to_le_bytes());

        let mut out = Vec::with_capacity(SECTION_OFFSET + section.len());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&(self.lines.len() as u16).to_le_bytes());
        out.extend_from_slice(&total.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(SECTION_OFFSET as u32).to_le_bytes());
        out.extend_from_slice(&section);
        out
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn flags(&self, index: usize) -> Option<u16> {
        self.lines.get(index).map(|line| line.flags)
    }

    /// Readable form of a line. Variables become `[VAR 0100(0000)]`, with the hex code first and
    /// its hex arguments in parentheses. `\n`, `\r`, `\c`, `\\` and `\[` are the other escapes.
    /// `None` if the index is out of range or the line is malformed.
    pub fn line(&self, index: usize) -> Option<String> {
        decode(&self.lines.get(index)?.units)
    }

    pub fn set_line(&mut self, index: usize, text: &str) -> Result<(), TextError> {
        let mut units = encode(text)?;
        let line = self
            .lines
            .get_mut(index)
            .ok_or(TextError::LineOutOfRange { index })?;
        // Game lines are padded with extra terminators; keeping the width means an unchanged
        // line rewrites to the same bytes.
        if units.len() < line.units.len() {
            units.resize(line.units.len(), TERMINATOR);
        }
        line.units = units;
        Ok(())
    }
}

fn flush_run(out: &mut String, run: &mut Vec<u16>) -> Option<()> {
    for decoded in char::decode_utf16(run.drain(..)) {
        match decoded.ok()? {
            '\n' => out.push_str("\\n"),
            '\\' => out.push_str("\\\\"),
            '[' => out.push_str("\\["),
            c => out.push(c),
        }
    }
    Some(())
}

fn decode(units: &[u16]) -> Option<String> {
    let mut out = String::new();
    let mut run = Vec::new();
    let mut i = 0;
    while let Some(&unit) = units.get(i) {
        match unit {
            TERMINATOR => break,
            VARIABLE => {
                flush_run(&mut out, &mut run)?;
                let count = usize::from(*units.get(i + 1)?);
                if count == 0 {
                    return None;
                }
                let code = *units.get(i + 2)?;
                let args = units.get(i + 3..i + 2 + count)?;
                match (count, code) {
                    (1, TEXT_RETURN) => out.push_str("\\r"),
                    (1, TEXT_CLEAR) => out.push_str("\\c"),
                    _ => {
                        out.push_str(&format!("[VAR {code:04X}"));
                        if !args.is_empty() {
                            let args: Vec<String> = args.iter().map(|a| format!("{a:04X}")).collect();
                            out.push_str(&format!("({})", args.join(",")));
                        }
                        out.push(']');
                    }
                }
                i += 2 + count;
            }
            _ => {
                run.push(unit);
                i += 1;
            }
        }
    }
    flush_run(&mut out, &mut run)?;
    Some(out)
}

fn parse_hex(text: &str) -> Result<u16, TextError> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(TextError::BadText(format!("\"{text}\" is not a hex value")));
    }
    u16::from_str_radix(text, 16)
        .map_err(|_| TextError::BadText(format!("\"{text}\" does not fit in 16 bits")))
}

fn encode_variable(inner: &str, units: &mut Vec<u16>) -> Result<(), TextError> {
    let bad = || TextError::BadText(format!("[{inner}] is not a valid variable"));
    let spec = inner.strip_prefix("VAR ").ok_or_else(bad)?;
    let (code, args) = match spec.split_once('(') {
        None => (spec, None),
        Some((code, rest)) => (code, Some(rest.strip_suffix(')').ok_or_else(bad)?)),
    };
    let args: Vec<u16> = match args {
        None | Some("") => Vec::new(),
        Some(list) => list.split(',').map(parse_hex).collect::<Result<_, _>>()?,
    };
    let count = u16::try_from(args.len() + 1).map_err(|_| bad())?;
    units.extend([VARIABLE, count, parse_hex(code)?]);
    units.extend(args);
    Ok(())
}

fn encode(text: &str) -> Result<Vec<u16>, TextError> {
    let mut units = Vec::with_capacity(text.len() + 1);
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let consumed = match c {
            '\\' => {
                let escape = rest[1..].chars().next();
                match escape {
                    Some('n') => units.push(u16::from(b'\n')),
                    Some('\\') => units.push(u16::from(b'\\')),
                    Some('[') => units.push(u16::from(b'[')),
                    Some('r') => units.extend([VARIABLE, 1, TEXT_RETURN]),
                    Some('c') => units.extend([VARIABLE, 1, TEXT_CLEAR]),
                    _ => return Err(TextError::BadText("invalid escape".to_owned())),
                }
                2
            }
            '[' => {
                let end = rest
                    .find(']')
                    .ok_or_else(|| TextError::BadText("unclosed [".to_owned()))?;
                encode_variable(&rest[1..end], &mut units)?;
                end + 1
            }
            '\0' | '\u{10}' => {
                return Err(TextError::BadText("control character cannot be stored".to_owned()))
            }
            c => {
                let mut buf = [0u16; 2];
                units.extend_from_slice(c.encode_utf16(&mut buf));
                c.len_utf8()
            }
        };
        rest = &rest[consumed..];
    }
    if units.len() >= usize::from(u16::MAX) {
        return Err(TextError::BadText("line is too long".to_owned()));
    }
    units.push(TERMINATOR);
    Ok(units)
}
