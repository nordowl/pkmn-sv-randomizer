//! Share codes: the settings that differ from the defaults plus an optional seed, as JSON, zlib
//! compressed and base64url encoded behind a version prefix. Object values such as pools keep
//! only the fields that differ from that option's default.

use std::io::{Read, Write};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use svr_core::rando::options::{OptionDef, Settings};

use crate::settings::{changed_values, from_json_values, Normalized};

const PREFIX: &str = "SVR1-";
const MAX_JSON_BYTES: u64 = 1 << 20;
const DAMAGED: &str = "This share code is damaged or incomplete. Copy the whole code again and paste it here.";

#[derive(Serialize, Deserialize)]
struct Payload {
    v: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    s: Option<String>,
}

fn default_json(registry: &[OptionDef], id: &str) -> Option<Value> {
    let def = registry.iter().find(|def| def.id == id)?;
    Some(serde_json::to_value(def.default_value()).expect("option values serialize"))
}

fn shrink(value: Value, default: Option<Value>) -> Value {
    match (value, default) {
        (Value::Object(fields), Some(Value::Object(defaults))) => Value::Object(
            fields
                .into_iter()
                .filter(|(field, value)| defaults.get(field) != Some(value))
                .collect(),
        ),
        (value, _) => value,
    }
}

fn expand(value: Value, default: Option<Value>) -> Value {
    match (value, default) {
        (Value::Object(fields), Some(Value::Object(mut full))) => {
            full.extend(fields);
            Value::Object(full)
        }
        (value, _) => value,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DecodedShare {
    pub settings: Settings,
    pub seed: Option<String>,
    pub warnings: Vec<String>,
}

pub fn encode(registry: &[OptionDef], settings: &Settings, seed: Option<&str>) -> String {
    let values = changed_values(registry, settings)
        .into_iter()
        .map(|(id, value)| {
            let value = serde_json::to_value(value).expect("option values serialize");
            let shrunk = shrink(value, default_json(registry, &id));
            (id, shrunk)
        })
        .collect();
    let payload = Payload {
        v: values,
        s: seed.map(str::trim).filter(|seed| !seed.is_empty()).map(str::to_owned),
    };
    let json = serde_json::to_vec(&payload).expect("share payload serializes");
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::best());
    zlib.write_all(&json).expect("compressing into memory cannot fail");
    let compressed = zlib.finish().expect("compressing into memory cannot fail");
    format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(compressed))
}

pub fn decode(registry: &[OptionDef], code: &str) -> Result<DecodedShare, String> {
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let Some(body) = strip_prefix_ignore_case(&code, PREFIX) else {
        return Err(if strip_prefix_ignore_case(&code, "SVR").is_some() {
            "This share code comes from a different version of the randomizer. Update the app, then try again.".to_owned()
        } else {
            format!("This is not a randomizer share code. Share codes start with {PREFIX}")
        });
    };
    let compressed = URL_SAFE_NO_PAD.decode(body).map_err(|_| DAMAGED.to_owned())?;
    let mut json = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .take(MAX_JSON_BYTES)
        .read_to_end(&mut json)
        .map_err(|_| DAMAGED.to_owned())?;
    let payload: Payload = serde_json::from_slice(&json).map_err(|_| DAMAGED.to_owned())?;
    let values = payload
        .v
        .into_iter()
        .map(|(id, value)| {
            let full = expand(value, default_json(registry, &id));
            (id, full)
        })
        .collect();
    let Normalized { settings, warnings } = from_json_values(registry, values);
    Ok(DecodedShare {
        settings,
        seed: payload.s.filter(|seed| !seed.trim().is_empty()),
        warnings,
    })
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &text[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use svr_core::rando;

    #[test]
    fn defaults_make_a_short_code() {
        let registry = rando::registry();
        let code = encode(&registry, &Settings::defaults(&registry), None);
        assert!(code.starts_with(PREFIX) && code.len() < 40, "{code}");
    }

    #[test]
    fn foreign_text_is_named() {
        let registry = rando::registry();
        assert!(decode(&registry, "hello").unwrap_err().contains("not a randomizer share code"));
        assert!(decode(&registry, "SVR9-abc").unwrap_err().contains("different version"));
    }
}
