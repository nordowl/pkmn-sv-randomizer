//! Missing-species engine test.

use crate::rando::options::OptionDef;
use crate::rando::{Ctx, RandoError, Section};

pub const SECTION: Section = Section {
    id: "experimental",
    options,
    run,
};

fn options() -> Vec<OptionDef> {
    Vec::new()
}

fn run(_ctx: &mut Ctx) -> Result<(), RandoError> {
    Ok(())
}
