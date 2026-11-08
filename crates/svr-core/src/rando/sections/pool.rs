use crate::rando::options::{Kind, Opt, OptionDef, Page, Source, Status};
use crate::rando::{Ctx, RandoError, Section, ALLOW_TERA_LOCKED, GLOBAL_POOL};

pub const SECTION: Section = Section {
    id: "pool",
    options,
    run,
};

fn options() -> Vec<OptionDef> {
    vec![
        Opt::new(GLOBAL_POOL, Page::Pool, "Global pool", "Species every section may use", Kind::Pool)
            .help("Generations, legendaries, Paradox Pokémon, evolution stages, forms and regional dexes. Each section can override this.")
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        Opt::toggle(
            ALLOW_TERA_LOCKED,
            Page::Pool,
            "Global pool",
            "Allow Ogerpon and Terapagos",
            false,
        )
        .help("The SV reference warns not to Terastallize these two after catching them. Off keeps them out of every random pool.")
        .sources(&[Source::Sv])
        .status(Status::Test)
        .build(),
    ]
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let size = ctx.global_pool()?.len();
    ctx.log
        .section("Species pool")
        .note(format!("{size} species forms are in the global pool."));
    Ok(())
}
