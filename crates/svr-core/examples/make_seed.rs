//! Writes an in-game test seed for Scarlet. With no preset: every toggle on, choices and numbers as
//! in round 0 of the `every_option_at_once` test, and three alternate-form custom starters revealed
//! at Nemona's house, so the lineup shows whether scene forms work. With a preset id from the app's
//! presets.json: that preset, then any `option.id=value` overrides (values are JSON, else text).
//!
//! cargo run -p svr-core --release --example make_seed -- <mod folder> [seed] [preset] [id=value]...
//! with SVR_TEST_KEYS and SVR_TEST_GAMES set as for the real-game tests.

use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::output::{write_mod, SeedMeta};
use svr_core::rando::options::{Kind, OptValue, SpeciesPick};
use svr_core::rando::{self, Seed, Settings};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("usage: make_seed <mod folder> [seed]"));
    let seed = Seed(args.next().unwrap_or_else(|| "ingame-1".into()));
    let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES").expect("set SVR_TEST_GAMES"));
    let keys = Keys::load(Path::new(&std::env::var_os("SVR_TEST_KEYS").expect("set SVR_TEST_KEYS"))).unwrap();
    let paths = GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES");
    let game = GameFiles::open(&paths, &keys).unwrap();

    let registry = rando::registry();
    let mut settings = Settings::defaults(&registry);
    if let Some(preset) = args.next() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/src/presets.json");
        let presets: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let list = presets.get("presets").unwrap_or(&presets).as_array().expect("a preset list");
        let found = list.iter().find(|p| p["id"] == preset.as_str()).unwrap_or_else(|| panic!("no preset {preset}"));
        let values = found.get("values").or_else(|| found.get("settings")).and_then(|v| v.as_object()).unwrap();
        for (id, value) in values {
            settings.set(id, serde_json::from_value(value.clone()).unwrap());
        }
        for pair in args {
            let (id, raw) = pair.split_once('=').expect("overrides look like option.id=value");
            let value = serde_json::from_str(raw).unwrap_or(serde_json::Value::String(raw.to_owned()));
            settings.set(id, serde_json::from_value(value).unwrap());
        }
        return write(&game, &settings, seed, &dir);
    }
    for (i, def) in registry.iter().enumerate() {
        let value = match &def.kind {
            Kind::Toggle { .. } => OptValue::Bool(true),
            Kind::Choice { choices, .. } => OptValue::Text(choices[i % choices.len()].value.to_owned()),
            Kind::Number { max, .. } => OptValue::Int(*max),
            _ => continue,
        };
        settings.set(def.id, value);
    }
    let pick = |national, form| OptValue::Species(SpeciesPick { national: Some(national), form });
    settings.set("starters.mode", OptValue::Text("custom".into()));
    settings.set("starters.reveal", OptValue::Text("nemona".into()));
    settings.set("starters.grass", pick(37, 1));
    settings.set("starters.fire", pick(194, 1));
    settings.set("starters.water", pick(128, 1));
    settings.set("shops.stock", OptValue::Text("random".into()));
    settings.set("shops.pokemart", OptValue::Text("extras".into()));
    settings.set("shops.pokemart_extras", OptValue::Int(8));
    write(&game, &settings, seed, &dir);
}

fn write(game: &GameFiles, settings: &Settings, seed: Seed, dir: &Path) {
    let out = rando::randomize(game, settings, seed.clone()).unwrap();
    let meta = SeedMeta {
        app_version: "make_seed".into(),
        edition: "scarlet".into(),
        title_id: "0100A3D008C5C000".into(),
        seed: seed.0.clone(),
        created_unix: 0,
        file_count: out.files.len(),
    };
    let spoiler = out.log.to_text();
    let settings_json = serde_json::to_vec_pretty(&out.settings).unwrap();
    let extras: [(&str, &[u8]); 2] = [("spoiler.txt", spoiler.as_bytes()), ("settings.json", &settings_json)];
    write_mod(dir, game, &out.files, &meta, &extras).unwrap();
    println!("wrote {} files to {}, warnings {:?}", out.files.len(), dir.display(), out.warnings);
}
