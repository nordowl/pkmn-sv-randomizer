//! Evolution condition codes, from pkNX's EvolutionType, with what each one's parameter means and
//! which move the Pokémon must know. The evolutions and moves sections both read this one table.

pub mod code {
    pub const FRIENDSHIP: u16 = 1;
    pub const FRIENDSHIP_DAY: u16 = 2;
    pub const FRIENDSHIP_NIGHT: u16 = 3;
    pub const LEVEL: u16 = 4;
    pub const TRADE: u16 = 5;
    pub const TRADE_ITEM: u16 = 6;
    pub const TRADE_SPECIES: u16 = 7;
    pub const USE_ITEM: u16 = 8;
    pub const HELD_ITEM_DAY: u16 = 19;
    pub const HELD_ITEM_NIGHT: u16 = 20;
    pub const UPSIDE_DOWN: u16 = 28;
    pub const WEATHER: u16 = 31;
    pub const LEVEL_DAY: u16 = 32;
    pub const LEVEL_NIGHT: u16 = 33;
    pub const LEVEL_DUSK: u16 = 40;
    pub const UNION_CIRCLE: u16 = 51;
    pub const HISUI: u16 = 61;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arg {
    None,
    /// An item ID, as for trades.
    Item,
    /// An evolution item ID, which the game maps from item IDs in its plib table.
    EvoItem,
    Move,
    Type,
    Species,
    /// A count that "make evolutions easier" lowers: steps, coins, wins, uses or HP lost.
    Count,
}

/// What the Pokémon must know to evolve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Needs {
    Nothing,
    /// The move in the parameter.
    ParamMove,
    /// Any move of the type in the parameter.
    MoveOfParamType,
    /// A fixed move, such as Rage Fist for Primeape.
    Move(&'static str),
}

pub struct Method {
    pub code: u16,
    pub text: &'static str,
    pub arg: Arg,
    pub needs: Needs,
}

const fn m(code: u16, text: &'static str, arg: Arg) -> Method {
    let needs = match arg {
        Arg::Move => Needs::ParamMove,
        Arg::Type => Needs::MoveOfParamType,
        _ => Needs::Nothing,
    };
    Method { code, text, arg, needs }
}

pub const METHODS: &[Method] = &[
    m(1, "High friendship", Arg::None),
    m(2, "High friendship (day)", Arg::None),
    m(3, "High friendship (night)", Arg::None),
    m(4, "Level up", Arg::None),
    m(5, "Trade", Arg::None),
    m(6, "Trade holding", Arg::Item),
    m(7, "Trade for", Arg::Species),
    m(8, "Use", Arg::EvoItem),
    m(9, "Level up, Attack > Defense", Arg::None),
    m(10, "Level up, Attack = Defense", Arg::None),
    m(11, "Level up, Attack < Defense", Arg::None),
    m(16, "High beauty", Arg::None),
    m(17, "Use (male)", Arg::EvoItem),
    m(18, "Use (female)", Arg::EvoItem),
    m(19, "Level up holding (day)", Arg::EvoItem),
    m(20, "Level up holding (night)", Arg::EvoItem),
    m(21, "Level up knowing", Arg::Move),
    m(22, "Level up with in party", Arg::Species),
    m(23, "Level up (male)", Arg::None),
    m(24, "Level up (female)", Arg::None),
    m(28, "Level up holding the console upside down", Arg::None),
    m(29, "Level up with affection knowing a move of type", Arg::Type),
    m(30, "Level up knowing a move of type", Arg::Type),
    m(31, "Level up in rain or fog", Arg::None),
    m(32, "Level up (day)", Arg::None),
    m(33, "Level up (night)", Arg::None),
    m(34, "Level up (female form)", Arg::None),
    m(36, "Level up in game version", Arg::None),
    m(39, "Level up at a summit", Arg::None),
    m(40, "Level up (dusk)", Arg::None),
    m(45, "Spin holding a Sweet", Arg::None),
    m(46, "Level up (Amped nature)", Arg::None),
    m(47, "Level up (Low Key nature)", Arg::None),
    m(48, "Tower of Darkness", Arg::None),
    m(49, "Tower of Waters", Arg::None),
    m(50, "Level up after walking with Let's Go, steps:", Arg::Count),
    m(51, "Level up in a Union Circle", Arg::None),
    m(52, "Level up in battle (family of four)", Arg::None),
    m(53, "Level up in battle (family of three)", Arg::None),
    m(54, "Level up with Gimmighoul Coins:", Arg::Count),
    m(55, "Level up after beating Bisharp leaders:", Arg::Count),
    Method {
        needs: Needs::Move("Rage Fist"),
        ..m(56, "Level up after using Rage Fist, times:", Arg::Count)
    },
    m(57, "Level up knowing (two segments)", Arg::Move),
    m(58, "Level up knowing (three segments)", Arg::Move),
    m(59, "Level up after losing HP from recoil (male):", Arg::Count),
    m(60, "Level up after losing HP from recoil (female):", Arg::Count),
    m(61, "Level up in Hisui", Arg::None),
];

pub fn method(code: u16) -> Option<&'static Method> {
    METHODS.iter().find(|m| m.code == code)
}

pub fn needs(code: u16) -> Needs {
    method(code).map_or(Needs::Nothing, |m| m.needs)
}
