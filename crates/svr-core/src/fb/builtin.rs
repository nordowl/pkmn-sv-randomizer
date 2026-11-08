use std::sync::{Arc, LazyLock};

use super::schema::Schema;

macro_rules! embedded_schema {
    ($name:ident, $path:literal) => {
        pub fn $name() -> Arc<Schema> {
            static SCHEMA: LazyLock<Arc<Schema>> = LazyLock::new(|| {
                Schema::from_fbs_text(include_str!($path)).expect(concat!("embedded schema ", $path))
            });
            Arc::clone(&SCHEMA)
        }
    };
}

embedded_schema!(personal, "../../schemas/personal_array.fbs");
embedded_schema!(waza, "../../schemas/waza_array.fbs");
embedded_schema!(tokusei, "../../schemas/tokusei_array.fbs");
embedded_schema!(pokemon_catalog, "../../schemas/pokemon_catalog.fbs");
