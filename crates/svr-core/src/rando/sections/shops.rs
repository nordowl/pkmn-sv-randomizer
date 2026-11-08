//! Shop lineups, the badges they need and the prices of what they sell.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::store::ITEMS;
use crate::fb::{FieldError, TableValue, Value};
use crate::rando::itempool::{ItemCatalog, ItemInfo, ItemKind};
use crate::rando::options::{Opt, OptionDef, Page, Settings, Source, Status};
use crate::rando::{Ctx, RandoError, Rng, Section};

pub const SECTION: Section = Section {
    id: "shops",
    options,
    run,
};

const STOCK: &str = "shops.stock";
const POKEMART: &str = "shops.pokemart";
const POKEMART_EXTRAS: &str = "shops.pokemart_extras";
const POKEMART_PER_BADGE: &str = "shops.pokemart_extras_per_badge";
const NO_REPEATS: &str = "shops.no_repeats";
const FOOD: &str = "shops.food";
const BAN_OP: &str = "shops.ban_op";
const BAN_BAD: &str = "shops.ban_bad";
const BAN_REGULAR: &str = "shops.ban_regular";
const BALANCE: &str = "shops.balance_prices";
const GUARANTEE_EVOLUTION: &str = "shops.guarantee_evolution";
const GUARANTEE_X: &str = "shops.guarantee_x";
const BADGE_GATING: &str = "shops.badge_gating";

const SHOPS: &str = "world/data/ui/shop/friendlyshop/friendlyshop_data/friendlyshop_data_array.bin";
const LINEUPS: &str = "world/data/ui/shop/friendlyshop/friendlyshop_lineup_data/friendlyshop_lineup_data_array.bin";
const TM_RECIPES: &str = "world/data/ui/shop/shop_wazamachine/shop_wazamachine_data/shop_wazamachine_data_array.bin";
/// The Poké Mart counter in every Pokémon Center.
const REGULAR_LINEUP: &str = "shop_00_lineup";
// `PayType` and `CondEnum` values from the shop schemas.
const PAY_BP: i64 = 2;
const COND_NONE: i64 = 0;
const COND_BADGES: i64 = 3;
/// Paldea's gym badges, over which the Pokémon Center extras unlock.
const BADGES: usize = 8;

fn options() -> Vec<OptionDef> {
    let page = Page::Shops;
    vec![
        Opt::choice(
            STOCK,
            page,
            "Stock",
            "Shop items",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Every shop sells the game's items."),
                ("shuffle", "Shuffle", "Each item the shops sell is swapped for another sold item, the same one everywhere."),
                ("random", "Random", "Every shop sells random items."),
            ],
        )
        .help("What the Poké Marts, Delibird Presents, food shops and the Blueberry Academy shops sell. TMs are only swapped for TMs and picnic food for picnic food. A shop never lists an item twice, and every slot still needs the badges it needed before.")
        .sources(&[Source::Upr])
        .status(Status::Test)
        .build(),
        Opt::choice(
            POKEMART,
            page,
            "Stock",
            "Pokémon Center counter",
            "extras",
            &[
                (
                    "extras",
                    "Standard items plus extras",
                    "The counter keeps its Potions, Poké Balls and status heals, which unlock with badges as usual, and sells random extra items after them.",
                ),
                ("random", "Random", "The counter sells random items like every other shop."),
            ],
        )
        .help("The Poké Mart counter in every Pokémon Center. Keeping its standard items means you can always buy healing items and Poké Balls.")
        .not(STOCK, "unchanged")
        .sources(&[Source::Upr])
        .status(Status::Test)
        .build(),
        Opt::number(POKEMART_EXTRAS, page, "Stock", "Extra counter items", 8, 0, 16, "items")
            .help("How many random items the counter sells after its standard items from the start.")
            .not(STOCK, "unchanged")
            .is(POKEMART, "extras")
            .sources(&[Source::New])
            .build(),
        Opt::number(POKEMART_PER_BADGE, page, "Stock", "More counter items per gym badge", 2, 0, 4, "items")
            .help("Each gym badge adds this many new random items to the counter. With 8 from the start and 2 per badge, the counter ends with 24 extras.")
            .not(STOCK, "unchanged")
            .is(POKEMART, "extras")
            .sources(&[Source::New])
            .build(),
        Opt::choice(
            FOOD,
            page,
            "Stock",
            "Food and picnic items",
            "keep",
            &[
                (
                    "keep",
                    "Unchanged",
                    "Food shops keep their usual ingredients, condiments and picnic gear, so sandwich recipes always work.",
                ),
                ("random", "Random", "Food shops sell random food and picnic items too. Common ingredients can still turn up in several shops."),
            ],
        )
        .not(STOCK, "unchanged")
        .sources(&[Source::New])
        .build(),
        Opt::toggle(NO_REPEATS, page, "Stock", "No repeats across shops", true)
            .help("Each random item is sold by only one kind of shop, so every shop you visit offers something different. Shops of the same kind, such as all Pokémon Centers or all bakeries, always share one list. If a kind of item runs short, a few repeat.")
            .is(STOCK, "random")
            .sources(&[Source::New])
            .build(),
        Opt::toggle(BAN_OP, page, "Stock", "Ban overpowered items", false)
            .help("Random shops never sell the Master Ball, Rare Candy, Ability Patch, Exp. Candy XL, Bottle Caps, the Lucky Egg, the Amulet Coin or treasures worth 10,000 or more.")
            .is(STOCK, "random")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(BAN_BAD, page, "Stock", "Ban bad items", false)
            .help("Random shops never sell items that only hurt their holder or do nothing in this game, such as the Lagging Tail.")
            .is(STOCK, "random")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(BAN_REGULAR, page, "Stock", "Ban regular shop items", false)
            .help("Random shops never sell what the Poké Mart counter in Pokémon Centers sells, such as Potions and Poké Balls, so they offer something new.")
            .is(STOCK, "random")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(BALANCE, page, "Stock", "Balance prices", false)
            .help("Items without a price get the usual price of shop items of their kind, so shops can sell them. Off, shops only stock items that already have a price; at the Blueberry Academy BP shop that means the TMs it sells in the game. Nothing is ever sold for free.")
            .not(STOCK, "unchanged")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(GUARANTEE_EVOLUTION, page, "Stock", "Guarantee evolution items", false)
            .help("Every evolution item, such as the stones, the Sweets and the items that evolve Pokémon through trades, is sold in at least one shop.")
            .not(STOCK, "unchanged")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(GUARANTEE_X, page, "Stock", "Guarantee X items", false)
            .help("X Attack, X Defense, Dire Hit, Guard Spec. and the other battle items that raise stats are each sold in at least one shop.")
            .not(STOCK, "unchanged")
            .sources(&[Source::Upr])
            .build(),
        Opt::choice(
            BADGE_GATING,
            page,
            "Badges",
            "Badge requirements",
            "keep",
            &[
                ("keep", "Keep", "Shop items unlock as you earn gym badges, as in the game."),
                ("remove", "Remove", "Every shop item is for sale from the start."),
            ],
        )
        .help("Many shop items only appear once you have enough gym badges. This works with any stock setting.")
        .sources(&[Source::New])
        .build(),
    ]
}

#[derive(Clone, Copy)]
struct Bans {
    overpowered: bool,
    bad: bool,
    regular: bool,
}

#[derive(Clone, Copy)]
enum StockMode {
    Shuffle,
    Random(Bans),
}

/// Random items the Pokémon Center counter sells after its standard ones: some from the start and
/// more with each gym badge.
#[derive(Clone, Copy)]
struct Extras {
    start: usize,
    per_badge: usize,
}

impl Extras {
    fn count(self) -> usize {
        self.start + self.per_badge * BADGES
    }

    /// The badges the `k`th extra needs.
    fn badges(self, k: usize) -> i64 {
        if k < self.start { 0 } else { (1 + (k - self.start) / self.per_badge) as i64 }
    }
}

struct Restock {
    mode: StockMode,
    /// `Some` keeps the Pokémon Center counter's standard items and adds extras.
    pokemart_extras: Option<Extras>,
    /// Random items are spread so no two kinds of shop sell the same one.
    no_repeats: bool,
    /// Food and picnic items change too, instead of keeping their shops' usual stock.
    food: bool,
    balance: bool,
    evolution: bool,
    x_items: bool,
}

struct Plan {
    restock: Option<Restock>,
    remove_badges: bool,
}

impl Plan {
    fn read(s: &Settings) -> Plan {
        let mode = match s.choice(STOCK) {
            "shuffle" => Some(StockMode::Shuffle),
            "random" => Some(StockMode::Random(Bans {
                overpowered: s.on(BAN_OP),
                bad: s.on(BAN_BAD),
                regular: s.on(BAN_REGULAR),
            })),
            _ => None,
        };
        Plan {
            restock: mode.map(|mode| Restock {
                mode,
                pokemart_extras: (s.choice(POKEMART) == "extras").then(|| Extras {
                    start: s.number(POKEMART_EXTRAS).max(0) as usize,
                    per_badge: s.number(POKEMART_PER_BADGE).max(0) as usize,
                }),
                no_repeats: s.on(NO_REPEATS),
                food: s.choice(FOOD) == "random",
                balance: s.on(BALANCE),
                evolution: s.on(GUARANTEE_EVOLUTION),
                x_items: s.on(GUARANTEE_X),
            }),
            remove_badges: s.choice(BADGE_GATING) == "remove",
        }
    }
}

/// Slots only ever trade items within their family, so a TM slot stays a TM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Family {
    Goods,
    Tm,
    Picnic,
}

impl Family {
    const ALL: [Family; 3] = [Family::Goods, Family::Tm, Family::Picnic];

    fn of(item: Option<&ItemInfo>) -> Family {
        match item.map(|i| i.kind) {
            Some(ItemKind::Tm) => Family::Tm,
            Some(ItemKind::Picnic) => Family::Picnic,
            _ => Family::Goods,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Currency {
    Money,
    Bp,
}

impl Currency {
    fn field(self) -> &'static str {
        match self {
            Currency::Money => "Price",
            Currency::Bp => "BP",
        }
    }

    /// The price of an item whose kind no vanilla shop sells in this currency. 50 BP is the
    /// median of the vanilla BP shop.
    fn fallback(self) -> i64 {
        match self {
            Currency::Money => 10_000,
            Currency::Bp => 50,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Currency::Money => "money",
            Currency::Bp => "BP",
        }
    }
}

struct Lineup {
    id: String,
    /// What the shops showing this lineup charge. The League Point shop counts as money.
    currencies: BTreeSet<Currency>,
}

struct Slot {
    /// The lineup row, or `None` for a Pokémon Center extra that becomes a new row.
    row: Option<usize>,
    lineup: usize,
    sortnum: i64,
    item: u16,
    family: Family,
    gate: String,
    badges: i64,
    /// A standard Pokémon Center item, or a food item while food stays unchanged, that keeps its place.
    kept: bool,
}

impl Slot {
    fn restocked(&self) -> bool {
        !self.kept && self.row.is_some()
    }
}

/// The vanilla lineups, one slot per lineup row that sells something, plus the Pokémon Center
/// extras, which start empty.
struct Stock {
    lineups: Vec<Lineup>,
    slots: Vec<Slot>,
    /// The row after the last Pokémon Center row, where the extras go so the lineup stays together.
    extras_at: usize,
}

/// Picks `n` distinct items from `candidates`, those not in `used` first. Returns them and how
/// many had to come from `used`. With nothing used, this is one plain shuffle.
fn draw(candidates: Vec<u16>, used: &BTreeSet<u16>, n: usize, rng: &mut Rng) -> (Vec<u16>, usize) {
    let (mut fresh, mut seen): (Vec<u16>, Vec<u16>) = candidates.into_iter().partition(|id| !used.contains(id));
    rng.shuffle(&mut fresh);
    rng.shuffle(&mut seen);
    let repeats = n.saturating_sub(fresh.len());
    fresh.extend(seen);
    fresh.truncate(n);
    (fresh, repeats)
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Section {
        section: "shops",
        detail: e.to_string(),
    }
}

fn item_of(value: Result<i64, FieldError>) -> u16 {
    value.ok().and_then(|v| u16::try_from(v).ok()).unwrap_or(0)
}

impl Stock {
    fn load(ctx: &mut Ctx, catalog: &ItemCatalog, pokemart_extras: Option<Extras>, food: bool) -> Result<Stock, RandoError> {
        let mut currencies: BTreeMap<String, BTreeSet<Currency>> = BTreeMap::new();
        for shop in ctx.store.get(SHOPS)?.tables("values") {
            let Some(lineup) = shop.string("lineup") else { continue };
            let currency = if shop.int("payKind").unwrap_or(0) == PAY_BP { Currency::Bp } else { Currency::Money };
            currencies.entry(lineup.to_owned()).or_default().insert(currency);
        }
        let mut lineups = Vec::new();
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        let mut slots = Vec::new();
        let mut extras_at = None;
        for (row, values) in ctx.store.get(LINEUPS)?.tables("values").enumerate() {
            let id = values.string("lineupid").unwrap_or_default();
            if id == REGULAR_LINEUP {
                extras_at = Some(row + 1);
            }
            let item = item_of(values.int("item"));
            if item == 0 {
                continue;
            }
            let lineup = *index.entry(id.to_owned()).or_insert_with(|| {
                lineups.push(Lineup {
                    id: id.to_owned(),
                    currencies: currencies.get(id).cloned().unwrap_or_else(|| [Currency::Money].into()),
                });
                lineups.len() - 1
            });
            let family = Family::of(catalog.get(item));
            slots.push(Slot {
                row: Some(row),
                lineup,
                sortnum: values.int("sortnum").unwrap_or(0),
                item,
                family,
                gate: values.string("item_condvalue").unwrap_or_default().to_owned(),
                badges: values.int("gym_badge_num").unwrap_or(0),
                kept: (pokemart_extras.is_some() && id == REGULAR_LINEUP) || (family == Family::Picnic && !food),
            });
        }
        let (Some(extras_at), Some(&regular)) = (extras_at, index.get(REGULAR_LINEUP)) else {
            return Err(RandoError::Section {
                section: "shops",
                detail: format!("the shop data has no {REGULAR_LINEUP}"),
            });
        };
        let last = slots.iter().filter(|s| s.lineup == regular).map(|s| s.sortnum).max().unwrap_or(0);
        let count = pokemart_extras.map_or(0, Extras::count);
        for k in 0..count {
            let badges = pokemart_extras.map_or(0, |e| e.badges(k));
            slots.push(Slot {
                row: None,
                lineup: regular,
                sortnum: last + 1 + k as i64,
                item: 0,
                family: Family::Goods,
                gate: if badges == 0 { String::new() } else { format!("BADGE{badges}") },
                badges,
                kept: false,
            });
        }
        Ok(Stock { lineups, slots, extras_at })
    }

    fn lineup_of(&self, slot: usize) -> &Lineup {
        &self.lineups[self.slots[slot].lineup]
    }
}

/// `Price` and `BP` of every item.
struct Prices(BTreeMap<u16, (i64, i64)>);

impl Prices {
    fn load(ctx: &mut Ctx) -> Result<Prices, RandoError> {
        let rows = ctx.store.get(ITEMS)?.tables("values");
        let prices = rows
            .map(|row| {
                let price = |c: Currency| row.int(c.field()).unwrap_or(0);
                (item_of(row.int("Id")), (price(Currency::Money), price(Currency::Bp)))
            })
            .collect();
        Ok(Prices(prices))
    }

    fn get(&self, item: u16, currency: Currency) -> i64 {
        let (money, bp) = self.0.get(&item).copied().unwrap_or_default();
        match currency {
            Currency::Money => money,
            Currency::Bp => bp,
        }
    }
}

/// The vanilla data every stock rule consults.
struct Market<'a> {
    stock: &'a Stock,
    catalog: &'a ItemCatalog,
    prices: &'a Prices,
    /// TMs the TM machine can make. Other TM rows, such as TM00, are placeholders.
    craftable: BTreeSet<u16>,
    balance: bool,
}

impl Market<'_> {
    /// Whether `lineup` may sell `item`: never for free, unless balancing gives it a price.
    fn sells(&self, item: u16, lineup: &Lineup) -> bool {
        self.balance || lineup.currencies.iter().all(|&c| self.prices.get(item, c) > 0)
    }

    /// The items a family may stock. `bans` only apply to goods in random mode.
    fn pool(&self, family: Family, bans: Option<Bans>) -> Vec<u16> {
        let sold: BTreeSet<u16> = self.stock.slots.iter().map(|s| s.item).collect();
        let regular: BTreeSet<u16> = self
            .stock
            .slots
            .iter()
            .filter(|s| self.stock.lineups[s.lineup].id == REGULAR_LINEUP)
            .map(|s| s.item)
            .collect();
        let banned = |i: &ItemInfo| {
            bans.is_some_and(|b| {
                (b.overpowered && i.is_shop_overpowered()) || (b.bad && i.is_bad()) || (b.regular && regular.contains(&i.id))
            })
        };
        self.catalog
            .all()
            .iter()
            .filter(|i| {
                i.named()
                    && match family {
                        Family::Goods => {
                            !matches!(i.kind, ItemKind::Key | ItemKind::Tm | ItemKind::Material | ItemKind::Picnic)
                                && !i.is_form_or_species_item()
                                && !banned(i)
                        }
                        Family::Tm => i.kind == ItemKind::Tm && (self.craftable.contains(&i.id) || sold.contains(&i.id)),
                        Family::Picnic => i.kind == ItemKind::Picnic && sold.contains(&i.id),
                    }
            })
            .map(|i| i.id)
            .collect()
    }

    /// Swaps every sold item for another of its family through one random bijection per family
    /// and currency, so an item is replaced by the same item everywhere and stays priced.
    fn shuffled(&self, rng: &mut Rng) -> Vec<u16> {
        let key = |s: &Slot| (s.family, &self.stock.lineups[s.lineup].currencies);
        let mut groups: BTreeMap<_, BTreeSet<u16>> = BTreeMap::new();
        for slot in self.stock.slots.iter().filter(|s| s.restocked()) {
            groups.entry(key(slot)).or_default().insert(slot.item);
        }
        let mut swap = BTreeMap::new();
        for (group, items) in groups {
            let from: Vec<u16> = items.into_iter().collect();
            let mut to = from.clone();
            rng.shuffle(&mut to);
            for (a, b) in from.into_iter().zip(to) {
                swap.insert((group, a), b);
            }
        }
        self.stock
            .slots
            .iter()
            .map(|s| if s.restocked() { swap[&(key(s), s.item)] } else { s.item })
            .collect()
    }

    /// Fills each lineup's slots of a family with distinct random items from its pool. With
    /// `no_repeats`, items another lineup already sells come last. Returns the items and how many
    /// slots had to repeat another lineup's item.
    fn random(&self, bans: Bans, no_repeats: bool, rng: &mut Rng) -> Result<(Vec<u16>, usize), RandoError> {
        let mut items: Vec<u16> = self.stock.slots.iter().map(|s| s.item).collect();
        let mut repeats = 0;
        for family in Family::ALL {
            let pool = self.pool(family, (family == Family::Goods).then_some(bans));
            let mut used = BTreeSet::new();
            for (l, lineup) in self.stock.lineups.iter().enumerate() {
                let slots: Vec<usize> = (0..items.len())
                    .filter(|&i| {
                        let s = &self.stock.slots[i];
                        s.restocked() && s.lineup == l && s.family == family
                    })
                    .collect();
                if slots.is_empty() {
                    continue;
                }
                let candidates: Vec<u16> = pool.iter().copied().filter(|&id| self.sells(id, lineup)).collect();
                if candidates.len() < slots.len() {
                    return Err(RandoError::Section {
                        section: "shops",
                        detail: format!(
                            "{} needs {} {family:?} items but only {} qualify. Turn off a ban or turn on price balancing.",
                            lineup.id,
                            slots.len(),
                            candidates.len()
                        ),
                    });
                }
                let (chosen, repeated) = draw(candidates, &used, slots.len(), rng);
                repeats += repeated;
                if no_repeats && family != Family::Picnic {
                    used.extend(chosen.iter().copied());
                }
                for (slot, item) in slots.into_iter().zip(chosen) {
                    items[slot] = item;
                }
            }
        }
        Ok((items, repeats))
    }

    /// Fills the Pokémon Center extras with distinct items from `pool` that the counter does not
    /// already sell, preferring ones not in `elsewhere`. Returns how many repeat an item there.
    fn fill_extras(&self, items: &mut [u16], pool: &[u16], elsewhere: &BTreeSet<u16>, rng: &mut Rng) -> Result<usize, RandoError> {
        let extras: Vec<usize> = (0..items.len()).filter(|&i| self.stock.slots[i].row.is_none()).collect();
        let Some(&first) = extras.first() else { return Ok(0) };
        let lineup = self.stock.lineup_of(first);
        let regular = self.stock.slots[first].lineup;
        let sold: BTreeSet<u16> =
            (0..items.len()).filter(|&i| self.stock.slots[i].lineup == regular).map(|i| items[i]).collect();
        let candidates: Vec<u16> =
            pool.iter().copied().filter(|&id| !sold.contains(&id) && self.sells(id, lineup)).collect();
        if candidates.len() < extras.len() {
            return Err(RandoError::Section {
                section: "shops",
                detail: format!(
                    "the Pokémon Center counter needs {} extra items but only {} qualify. Lower the number of extras, turn off a ban or turn on price balancing.",
                    extras.len(),
                    candidates.len()
                ),
            });
        }
        let (chosen, repeats) = draw(candidates, elsewhere, extras.len(), rng);
        for (i, item) in extras.into_iter().zip(chosen) {
            items[i] = item;
        }
        Ok(repeats)
    }

    /// Puts each of `wanted` in at least one goods slot that may sell it, overwriting a slot whose
    /// item is also sold elsewhere, or else one holding an item nobody asked for. Kept Pokémon
    /// Center items are never overwritten. Returns the items that found no slot.
    fn guarantee(&self, items: &mut [u16], wanted: &[u16], rng: &mut Rng) -> Vec<u16> {
        let goods: Vec<usize> = (0..items.len()).filter(|&i| self.stock.slots[i].family == Family::Goods).collect();
        let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
        for &i in &goods {
            *counts.entry(items[i]).or_default() += 1;
        }
        let goods: Vec<usize> = goods.into_iter().filter(|&i| !self.stock.slots[i].kept).collect();
        let mut missing = Vec::new();
        for &item in wanted {
            if counts.get(&item).is_some_and(|&n| n > 0) {
                continue;
            }
            // An item absent from every goods slot is in no lineup, so placing it cannot repeat it.
            let fits = |i: &usize| self.sells(item, self.stock.lineup_of(*i));
            let mut spare: Vec<usize> = goods.iter().copied().filter(|i| counts[&items[*i]] > 1 && fits(i)).collect();
            if spare.is_empty() {
                spare = goods.iter().copied().filter(|i| !wanted.contains(&items[*i]) && fits(i)).collect();
            }
            let Some(&i) = rng.pick(&spare) else {
                missing.push(item);
                continue;
            };
            *counts.get_mut(&items[i]).expect("counted above") -= 1;
            items[i] = item;
            counts.insert(item, 1);
        }
        missing
    }

    /// New prices for items that end up sold for a currency they have no price in: the median
    /// vanilla price of shop items of the same kind and currency.
    fn balanced_prices(&self, items: &[u16]) -> BTreeMap<(u16, Currency), i64> {
        let kind = |id: u16| self.catalog.get(id).map(|i| i.kind);
        let mut vanilla: BTreeMap<(ItemKind, Currency), BTreeMap<u16, i64>> = BTreeMap::new();
        for (i, slot) in self.stock.slots.iter().enumerate() {
            let Some(k) = kind(slot.item) else { continue };
            for &c in &self.stock.lineup_of(i).currencies {
                let price = self.prices.get(slot.item, c);
                if price > 0 {
                    vanilla.entry((k, c)).or_default().insert(slot.item, price);
                }
            }
        }
        let median = |k: Option<ItemKind>, c: Currency| {
            let Some(prices) = k.and_then(|k| vanilla.get(&(k, c))) else { return c.fallback() };
            let mut sorted: Vec<i64> = prices.values().copied().collect();
            sorted.sort_unstable();
            sorted[sorted.len() / 2]
        };
        let mut out = BTreeMap::new();
        for (i, &item) in items.iter().enumerate() {
            for &c in &self.stock.lineup_of(i).currencies {
                if self.prices.get(item, c) == 0 {
                    out.insert((item, c), median(kind(item), c));
                }
            }
        }
        out
    }
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let plan = Plan::read(&ctx.settings);
    if let Some(restock) = &plan.restock {
        restock_shops(ctx, restock)?;
    }
    if plan.remove_badges {
        remove_badge_gates(ctx)?;
    }
    Ok(())
}

fn restock_shops(ctx: &mut Ctx, restock: &Restock) -> Result<(), RandoError> {
    let catalog = ItemCatalog::load(ctx)?;
    let prices = Prices::load(ctx)?;
    let stock = Stock::load(ctx, &catalog, restock.pokemart_extras, restock.food)?;
    let craftable = ctx.store.get(TM_RECIPES)?.tables("values").map(|r| item_of(r.int("WazaItemID"))).collect();
    let market = Market {
        stock: &stock,
        catalog: &catalog,
        prices: &prices,
        craftable,
        balance: restock.balance,
    };
    let mut rng = ctx.rng(STOCK);
    let (mut items, mut repeats) = match restock.mode {
        StockMode::Shuffle => (market.shuffled(&mut rng), 0),
        StockMode::Random(bans) => market.random(bans, restock.no_repeats, &mut rng)?,
    };
    let no_repeats = restock.no_repeats && matches!(restock.mode, StockMode::Random(_));
    let extras_pool: Vec<u16> = match restock.mode {
        StockMode::Random(bans) => market.pool(Family::Goods, Some(bans)),
        StockMode::Shuffle => {
            let sold = stock.slots.iter().filter(|s| s.row.is_some() && s.family == Family::Goods).map(|s| s.item);
            sold.collect::<BTreeSet<u16>>().into_iter().collect()
        }
    };
    let elsewhere: BTreeSet<u16> = if no_repeats {
        stock.slots.iter().zip(&items).filter(|(s, _)| s.restocked() && s.family == Family::Goods).map(|(_, &i)| i).collect()
    } else {
        BTreeSet::new()
    };
    repeats += market.fill_extras(&mut items, &extras_pool, &elsewhere, &mut ctx.rng(POKEMART))?;

    let wanted: Vec<u16> = market
        .pool(Family::Goods, None)
        .into_iter()
        .filter(|&id| {
            catalog
                .get(id)
                .is_some_and(|i| (restock.evolution && i.is_evolution()) || (restock.x_items && i.is_x_item()))
        })
        .collect();
    let missing = market.guarantee(&mut items, &wanted, &mut ctx.rng("shops.guarantee"));
    let new_prices = if restock.balance { market.balanced_prices(&items) } else { BTreeMap::new() };

    let changed: Vec<(&Slot, u16)> =
        stock.slots.iter().zip(&items).filter(|(s, &new)| s.item != new).map(|(s, &new)| (s, new)).collect();
    if !changed.is_empty() {
        let table = ctx.store.get_mut(LINEUPS)?;
        let mut rows: Vec<&mut TableValue> = table.tables_mut("values").collect();
        for &(slot, new) in &changed {
            if let Some(row) = slot.row {
                rows[row].set_int("item", i64::from(new)).map_err(shape)?;
            }
        }
        let mut added = Vec::new();
        for &(slot, new) in changed.iter().filter(|(s, _)| s.row.is_none()) {
            let mut row = table.new_table("values").map_err(shape)?;
            row.set_string("lineupid", REGULAR_LINEUP).map_err(shape)?;
            row.set_int("sortnum", slot.sortnum).map_err(shape)?;
            row.set_int("item", i64::from(new)).map_err(shape)?;
            if slot.badges > 0 {
                row.set_int("item_condkind", COND_BADGES).map_err(shape)?;
                row.set_string("item_condvalue", slot.gate.clone()).map_err(shape)?;
                row.set_int("gym_badge_num", slot.badges).map_err(shape)?;
            }
            added.push(Value::Table(row));
        }
        let at = stock.extras_at;
        table.vector_mut("values").map_err(shape)?.splice(at..at, added);
    }
    if !new_prices.is_empty() {
        for row in ctx.store.get_mut(ITEMS)?.tables_mut("values") {
            let id = item_of(row.int("Id"));
            for c in [Currency::Money, Currency::Bp] {
                if let Some(&price) = new_prices.get(&(id, c)) {
                    row.set_int(c.field(), price).map_err(shape)?;
                }
            }
        }
    }

    let names = &ctx.names;
    let log = ctx.log.section("Shops");
    let extras = stock.slots.iter().filter(|s| s.row.is_none()).count();
    let vanilla_slots = stock.slots.len() - extras;
    log.note(format!("{} of {vanilla_slots} shop slots sell something else.", changed.len() - extras));
    if no_repeats && repeats > 0 {
        log.note(format!("{repeats} shop slots repeat an item another kind of shop sells, because their kind of item ran short."));
    }
    if restock.pokemart_extras.is_some() {
        log.note(format!(
            "The Pokémon Center counter keeps its standard items and sells {extras} extra items after them."
        ));
    }
    if !wanted.is_empty() {
        let placed: Vec<String> = wanted.iter().filter(|id| !missing.contains(id)).map(|&id| names.item(id)).collect();
        log.note(format!("Sold in at least one shop: {}.", placed.join(", ")));
    }
    if !missing.is_empty() {
        let unplaced: Vec<String> = missing.iter().map(|&id| names.item(id)).collect();
        log.note(format!("No shop slot could take: {}.", unplaced.join(", ")));
    }
    let table = log.table("Shop stock", &["Lineup", "Slot", "Was", "Now", "Needs"]);
    for (slot, new) in changed {
        let gate = if slot.gate.is_empty() { "-".to_owned() } else { slot.gate.clone() };
        let was = if slot.row.is_some() { names.item(slot.item) } else { "(new slot)".to_owned() };
        table.row(vec![stock.lineups[slot.lineup].id.clone(), slot.sortnum.to_string(), was, names.item(new), gate]);
    }
    if !new_prices.is_empty() {
        let table = log.table("New prices", &["Item", "Currency", "Was", "Now"]);
        for (&(item, c), &price) in &new_prices {
            table.row(vec![names.item(item), c.label().to_owned(), prices.get(item, c).to_string(), price.to_string()]);
        }
    }
    Ok(())
}

fn remove_badge_gates(ctx: &mut Ctx) -> Result<(), RandoError> {
    let gated = |row: &TableValue| row.int("item_condkind").unwrap_or(COND_NONE) == COND_BADGES;
    let count = ctx.store.get(LINEUPS)?.tables("values").filter(|row| gated(row)).count();
    if count > 0 {
        for row in ctx.store.get_mut(LINEUPS)?.tables_mut("values").filter(|row| gated(row)) {
            row.set_int("item_condkind", COND_NONE).map_err(shape)?;
            row.set_string("item_condvalue", "").map_err(shape)?;
            row.set_int("gym_badge_num", 0).map_err(shape)?;
        }
    }
    ctx.log
        .section("Shops")
        .note(format!("{count} shop slots no longer need gym badges."));
    Ok(())
}
