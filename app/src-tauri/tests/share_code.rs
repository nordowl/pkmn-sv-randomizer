use svr_app::share;
use svr_core::rando::{self, OptValue, PoolFilter, Settings, ALLOW_TERA_LOCKED, GLOBAL_POOL};

fn changed_settings() -> Settings {
    let registry = rando::registry();
    let mut settings = Settings::defaults(&registry);
    let mut pool = PoolFilter::everything();
    pool.generations[0] = false;
    pool.legendaries = [false, true, false, true, false, true, false, true, false];
    pool.paradox = false;
    pool.stages = [true, false, true, true];
    pool.regional_dex = [true, false, true];
    pool.banned = vec![25, 133, 1000];
    pool.allowed = vec![150];
    settings.set(GLOBAL_POOL, OptValue::Pool(pool));
    settings.set(ALLOW_TERA_LOCKED, OptValue::Bool(true));
    settings
}

#[test]
fn share_codes_round_trip_exactly() {
    let registry = rando::registry();
    let settings = changed_settings();
    let code = share::encode(&registry, &settings, Some("my seed 42"));
    println!("OK share code ({} chars): {code}", code.len());
    assert!(code.starts_with("SVR1-"));
    assert!(code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));

    let decoded = share::decode(&registry, &format!("  \n{code}\t ")).unwrap();
    assert_eq!(decoded.settings, settings.normalized(&registry).0);
    assert_eq!(decoded.seed.as_deref(), Some("my seed 42"));
    assert!(decoded.warnings.is_empty(), "{:?}", decoded.warnings);

    let without_seed = share::decode(&registry, &share::encode(&registry, &settings, None)).unwrap();
    assert_eq!(without_seed.seed, None);
    assert_eq!(without_seed.settings, decoded.settings);

    let defaults = share::decode(&registry, &share::encode(&registry, &Settings::defaults(&registry), None)).unwrap();
    assert_eq!(defaults.settings, Settings::defaults(&registry));

    let mut typical = Settings::defaults(&registry);
    let mut pool = PoolFilter::everything();
    pool.inherit = true;
    pool.legendaries = [false; 9];
    typical.set(GLOBAL_POOL, OptValue::Pool(pool));
    let code = share::encode(&registry, &typical, Some("12345"));
    assert_eq!(share::decode(&registry, &code).unwrap().settings, typical);
    println!("OK no-legendaries code with a seed ({} chars): {code}", code.len());
}

#[test]
fn damaged_share_codes_are_refused_plainly() {
    let registry = rando::registry();
    let code = share::encode(&registry, &changed_settings(), Some("seed"));

    let truncated = &code[..code.len() - 6];
    let mut flipped: Vec<char> = code.chars().collect();
    let middle = flipped.len() / 2;
    flipped[middle] = if flipped[middle] == 'A' { 'B' } else { 'A' };
    let flipped: String = flipped.into_iter().collect();

    for damaged in [truncated, flipped.as_str(), "SVR1-", "SVR1-!!!!"] {
        let error = share::decode(&registry, damaged).unwrap_err();
        assert!(error.contains("damaged"), "{damaged}: {error}");
    }
    println!("OK damaged code: {}", share::decode(&registry, truncated).unwrap_err());
}
