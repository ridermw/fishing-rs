//! Species table, rods and catch rules.
use crate::pixel::{FishLook, Pattern, Shape};

#[derive(Clone, Copy, PartialEq)] pub enum Zone { Sea, Fresh, Any }
#[derive(Clone, Copy, PartialEq)] pub enum Depth { Shallow, Deep, Any }
#[derive(Clone, Copy, PartialEq)] pub enum When { Day, Night, Twilight, Any }

pub struct Species {
    pub look: FishLook, pub name: &'static str, pub zone: Zone, pub depth: Depth, pub time: When,
    pub w: f32, pub str_: f32, pub value: u32, pub size: (f32, f32), pub hint: &'static str,
}
impl Species {
    pub fn id(&self) -> &'static str { self.look.id }
    pub fn rarity(&self) -> &'static str {
        if self.look.id == "boot" { "Junk" } else if self.w <= 1.5 { "Legendary" } else if self.w <= 6.0 { "Rare" } else if self.w <= 16.0 { "Uncommon" } else { "Common" }
    }
}

macro_rules! fish {
    ($id:expr, $name:expr, $zone:ident, $depth:ident, $time:ident, $w:expr, $s:expr, $v:expr, $size:expr, $h:expr, $shape:ident, $pat:ident, $body:expr, $belly:expr, $fin:expr, $acc:expr, $hint:expr) => {
        Species { look: FishLook { id: $id, h: $h, shape: Shape::$shape, pattern: Pattern::$pat, body: $body, belly: $belly, fin: $fin, accent: $acc },
            name: $name, zone: Zone::$zone, depth: Depth::$depth, time: When::$time, w: $w, str_: $s, value: $v, size: $size, hint: $hint }
    };
}

pub fn species() -> Vec<Species> {
    vec![
        fish!("sardine", "Silver Sardine", Sea, Any, Any, 40., 0.15, 8, (12., 22.), 3, Fish, HStripe, "#7f9fb8", "#dfe8ee", "#5d7c96", None, "Anywhere in the sea."),
        fish!("mackerel", "Tiger Mackerel", Sea, Any, Day, 28., 0.3, 16, (25., 42.), 3, Fish, Stripes, "#3f8a7a", "#e6eef0", "#2e6a5e", None, "Sea, during the day."),
        fish!("flounder", "Sandy Flounder", Sea, Shallow, Any, 18., 0.35, 24, (25., 48.), 5, Flat, Spots, "#a8865a", "#d8c4a0", "#8a6a42", Some("#6a4a2a"), "Shallow sea near the beach."),
        fish!("bream", "Rosy Sea Bream", Sea, Deep, Day, 15., 0.45, 38, (30., 52.), 5, Fish, None, "#d97a7a", "#f6d8d0", "#b85a62", None, "Deep sea, by day. Try the end of the pier."),
        fish!("puffer", "Puffer Bun", Sea, Shallow, Day, 10., 0.3, 42, (15., 30.), 6, Round, Spots, "#e4c04a", "#f8ecc0", "#c49a2a", Some("#6a5a2a"), "Shallow sea on sunny days."),
        fish!("squid", "Inkcap Squid", Sea, Deep, Night, 14., 0.5, 48, (20., 40.), 4, Squid, None, "#e8b8c8", "#f4d8e0", "#c88aa0", None, "Deep sea after dark."),
        fish!("eel", "Lantern Eel", Sea, Deep, Night, 5., 0.75, 130, (60., 120.), 2, Fish, Glow, "#2a3a5a", "#4a5a7a", "#1a2a44", Some("#7af0ff"), "Deep sea, deep in the night."),
        fish!("tuna", "Bluefin Tuna", Sea, Deep, Day, 4., 0.9, 220, (90., 210.), 5, Fish, None, "#2a4a7a", "#d8e0e8", "#e4c04a", None, "Rare. Deep sea, daytime."),
        fish!("coelacanth", "Old Coelacanth", Sea, Deep, Twilight, 1.2, 1.0, 600, (120., 190.), 5, Fish, Spots, "#3a4a6a", "#6a7a9a", "#2a3a52", Some("#e8e0c0"), "Legendary. Deep sea at dawn or dusk."),
        fish!("perch", "Reed Perch", Fresh, Any, Any, 36., 0.2, 10, (15., 30.), 4, Fish, Stripes, "#8aa04a", "#e8e0a0", "#d0603a", None, "The pond, any time."),
        fish!("trout", "Rainbow Trout", Fresh, Any, Day, 26., 0.35, 20, (25., 50.), 4, Fish, HStripe, "#8a9a7a", "#f0e4d8", "#6a7a5a", Some("#e87a8a"), "The pond, by day."),
        fish!("catfish", "Whisker Catfish", Fresh, Deep, Night, 12., 0.55, 52, (40., 90.), 4, Fish, None, "#5a5a4a", "#a8a08a", "#3a3a2e", None, "Middle of the pond at night."),
        fish!("koi", "Golden Koi", Fresh, Any, Twilight, 3., 0.7, 260, (40., 80.), 4, Fish, Spots, "#f0a830", "#f8e0a0", "#f4f0e8", Some("#f4f0e8"), "Rare. The pond at dawn or dusk."),
        fish!("boot", "Soggy Boot", Any, Any, Any, 6., 0.08, 1, (26., 30.), 4, Boot, None, "#6b4a2f", "#6b4a2f", "#6b4a2f", None, "Somebody lost this..."),
    ]
}

pub struct Rod { pub name: &'static str, pub cost: u32, pub max_t: f32, pub reel: f32, pub boost: f32 }
pub const RODS: [Rod; 4] = [
    Rod { name: "Bamboo Rod", cost: 0, max_t: 1.0, reel: 0.30, boost: 0.0 },
    Rod { name: "Fiberglass Rod", cost: 150, max_t: 1.25, reel: 0.38, boost: 0.4 },
    Rod { name: "Carbon Rod", cost: 600, max_t: 1.55, reel: 0.47, boost: 0.8 },
    Rod { name: "Driftwood Legend", cost: 2000, max_t: 1.9, reel: 0.56, boost: 1.4 },
];

pub fn time_ok(t: When, h: f32) -> bool {
    match t {
        When::Any => true,
        When::Day => (6.0..19.0).contains(&h),
        When::Night => h >= 19.0 || h < 6.0,
        When::Twilight => (5.0..8.0).contains(&h) || (17.0..20.5).contains(&h),
    }
}

/// Weighted pick of a species for the water the bobber landed in. Rarer fish get a boost from better rods.
pub fn choose(all: &[Species], zone: Zone, deep: bool, hour: f32, boost: f32, roll: f32) -> usize {
    let pool: Vec<usize> = (0..all.len()).filter(|&k| {
        let s = &all[k];
        (s.zone == Zone::Any || s.zone == zone) && (s.depth == Depth::Any || (s.depth == Depth::Deep) == deep) && time_ok(s.time, hour)
    }).collect();
    let weight = |s: &Species| {
        let boot = s.id() == "boot";
        s.w * if s.w <= 6.0 && !boot { 1.0 + boost * 1.5 } else { 1.0 } * if boot { 1.0 / (1.0 + boost) } else { 1.0 }
    };
    let total: f32 = pool.iter().map(|&k| weight(&all[k])).sum();
    let mut r = roll * total;
    for &k in &pool { r -= weight(&all[k]); if r <= 0.0 { return k; } }
    pool[0]
}
