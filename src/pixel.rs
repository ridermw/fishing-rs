//! Procedural pixel art. Every texture and sprite in the game is drawn here at startup.

pub type Rgba = [u8; 4];
pub const CLEAR: Rgba = [0, 0, 0, 0];

pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Self { Rng(if seed == 0 { 1 } else { seed }) }
    pub fn f(&mut self) -> f32 {
        let mut s = self.0;
        s ^= s << 13; s ^= s >> 17; s ^= s << 5;
        self.0 = s;
        s as f32 / 4294967296.0
    }
    pub fn int(&mut self, n: usize) -> usize { ((self.f() * n as f32) as usize).min(n.saturating_sub(1)) }
    pub fn pick<T: Copy>(&mut self, a: &[T]) -> T { a[self.int(a.len())] }
}

pub fn hex(s: &str) -> Rgba {
    let n = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
    [(n >> 16) as u8, (n >> 8) as u8, n as u8, 255]
}
pub fn shade(c: Rgba, f: f32) -> Rgba {
    let m = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
    [m(c[0]), m(c[1]), m(c[2]), c[3]]
}
fn pal(a: &[&str]) -> Vec<Rgba> { a.iter().map(|s| hex(s)).collect() }

#[derive(Clone)]
pub struct Img { pub w: usize, pub h: usize, pub px: Vec<Rgba> }
impl Img {
    pub fn new(w: usize, h: usize) -> Self { Img { w, h, px: vec![CLEAR; w * h] } }
    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h { self.px[y as usize * self.w + x as usize] = c; }
    }
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h { self.px[y as usize * self.w + x as usize] } else { CLEAR }
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for j in 0..h { for i in 0..w { self.set(x + i, y + j, c); } }
    }
    fn noise(&mut self, r: &mut Rng, p: &[Rgba]) {
        for y in 0..self.h as i32 { for x in 0..self.w as i32 { let c = r.pick(p); self.set(x, y, c); } }
    }
    pub fn blit(&mut self, s: &Img, ox: i32, oy: i32, flip: bool) {
        for y in 0..s.h as i32 { for x in 0..s.w as i32 {
            let c = s.get(x, y);
            if c[3] > 0 { self.set(ox + if flip { s.w as i32 - 1 - x } else { x }, oy + y, c); }
        } }
    }
    pub fn outline(&mut self, c: Rgba) {
        let mut add = vec![];
        for y in 0..self.h as i32 { for x in 0..self.w as i32 {
            if self.get(x, y)[3] == 0 && (self.get(x - 1, y)[3] > 0 || self.get(x + 1, y)[3] > 0 || self.get(x, y - 1)[3] > 0 || self.get(x, y + 1)[3] > 0) { add.push((x, y)); }
        } }
        for (x, y) in add { self.set(x, y, c); }
    }
    pub fn scaled(&self, s: usize) -> Img {
        let mut o = Img::new(self.w * s, self.h * s);
        for y in 0..o.h { for x in 0..o.w { o.px[y * o.w + x] = self.px[(y / s) * self.w + x / s]; } }
        o
    }
    pub fn bytes(&self) -> Vec<u8> { self.px.iter().flat_map(|c| c.iter().copied()).collect() }
}

// ---------- tileable voronoi used for cobbles and cliff stone ----------
fn stoneish(n: usize, seed: u32, p: &[Rgba], mortar: Rgba) -> Img {
    let (w, h) = (16usize, 16usize);
    let mut r = Rng::new(seed);
    let pts: Vec<(f32, f32, f32)> = (0..n).map(|_| (r.f() * w as f32, r.f() * h as f32, r.f())).collect();
    let mut img = Img::new(w, h);
    for y in 0..h { for x in 0..w {
        let (mut d1, mut d2, mut best, mut bdx, mut bdy) = (1e9f32, 1e9f32, 0usize, 0f32, 0f32);
        for (i, pt) in pts.iter().enumerate() {
            let mut dx = x as f32 + 0.5 - pt.0; let mut dy = y as f32 + 0.5 - pt.1;
            dx -= (dx / w as f32).round() * w as f32; dy -= (dy / h as f32).round() * h as f32;
            let d = (dx * dx + dy * dy).sqrt();
            if d < d1 { d2 = d1; d1 = d; best = i; bdx = dx; bdy = dy; } else if d < d2 { d2 = d; }
        }
        let c = if d2 - d1 < 1.15 { mortar } else {
            let base = p[((pts[best].2 * p.len() as f32) as usize).min(p.len() - 1)];
            let mut f = if bdy < -1.6 && bdx < 0.5 { 1.15 } else if bdy > 1.4 { 0.82 } else { 1.0 };
            f *= 0.95 + r.f() * 0.1;
            shade(base, f)
        };
        img.set(x as i32, y as i32, c);
    } }
    img
}

const GRASS: [&str; 6] = ["#5a9a36", "#5a9a36", "#66a83c", "#4e8c30", "#71b343", "#4a8530"];
const DIRT: [&str; 4] = ["#7a5232", "#6b4529", "#8a5f3a", "#734c2f"];

pub fn grass(seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    g.noise(&mut r, &pal(&GRASS));
    for _ in 0..14 { let x = r.int(16) as i32; let y = r.int(15) as i32; g.set(x, y, hex("#8cc957")); g.set(x, y + 1, hex("#3f7628")); }
    if r.f() < 0.6 { let c = r.pick(&pal(&["#f4e26b", "#ffffff", "#e86a8a"])); g.set(r.int(16) as i32, r.int(16) as i32, c); }
    g
}
pub fn grass_side(seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    let gp = pal(&GRASS);
    g.noise(&mut r, &pal(&DIRT));
    for x in 0..16 {
        let d = 3 + r.int(3) as i32;
        for y in 0..d { let c = r.pick(&gp); g.set(x, y, c); }
        g.set(x, d, hex("#3f7628"));
    }
    for _ in 0..5 { g.set(r.int(16) as i32, 7 + r.int(9) as i32, hex("#9a7a5a")); }
    g
}
pub fn dirt(seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    g.noise(&mut r, &pal(&["#a97c4f", "#9c7046", "#b48858", "#a07448"]));
    for _ in 0..6 { let x = r.int(15) as i32; let y = r.int(15) as i32; g.set(x, y, hex("#c9a273")); g.set(x + 1, y + 1, hex("#7d5634")); }
    g
}
pub fn dirt_side(seed: u32) -> Img { let mut r = Rng::new(seed); let mut g = Img::new(16, 16); g.noise(&mut r, &pal(&DIRT)); g }
pub fn sand(seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    g.noise(&mut r, &pal(&["#ead59e", "#e2cb8f", "#f1dfac", "#dcc386"]));
    for _ in 0..4 { g.set(r.int(16) as i32, r.int(16) as i32, hex("#c9ad72")); }
    if r.f() < 0.5 { g.set(r.int(16) as i32, r.int(16) as i32, hex("#fff4e0")); }
    g
}
pub fn sand_side(seed: u32) -> Img { let mut r = Rng::new(seed); let mut g = Img::new(16, 16); g.noise(&mut r, &pal(&["#cdb07a", "#c2a46e", "#d6bb85", "#b8996a"])); g }
pub fn cobble(seed: u32) -> Img { stoneish(7, seed, &pal(&["#9b958c", "#a8a196", "#8d877e", "#b3ab9f"]), hex("#5d5850")) }
pub fn stone(seed: u32) -> Img {
    let mut g = stoneish(5, seed, &pal(&["#7c7a76", "#6e6d6a", "#878480", "#73706a"]), hex("#46443f"));
    let mut r = Rng::new(seed + 9);
    let moss = pal(&["#5d7a3a", "#6b8a40"]);
    for _ in 0..6 { let c = r.pick(&moss); g.set(r.int(16) as i32, r.int(16) as i32, c); }
    g
}
pub fn plank(seed: u32, horizontal: bool) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    let p = pal(&["#9a6a3e", "#8d5f36", "#a57446"]);
    for row in 0..4 {
        let base = r.pick(&p);
        for y in 0..4 { for x in 0..16 {
            let mut c = shade(base, 0.94 + r.f() * 0.12);
            if y == 3 { c = hex("#4f331d"); }
            if y == 0 { c = shade(base, 1.12); }
            if horizontal { g.set(x, row * 4 + y, c) } else { g.set(row * 4 + y, x, c) }
        } }
        let nx = r.int(14) as i32 + 1;
        if horizontal { g.set(nx, row * 4 + 1, hex("#3a2a1e")) } else { g.set(row * 4 + 1, nx, hex("#3a2a1e")) }
    }
    g
}
pub fn bark(seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    let p = pal(&["#6b4a2f", "#5d3f27", "#795537"]);
    for x in 0..16 { let b = r.pick(&p); for y in 0..16 { let c = if r.f() < 0.15 { hex("#4a321f") } else { b }; g.set(x, y, c); } }
    g
}
/// Grayscale foliage, tinted per-vertex so one texture serves green, gold and red trees.
pub fn leaf(seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    g.noise(&mut r, &pal(&["#c8c8c8", "#b4b4b4", "#d8d8d8", "#a6a6a6"]));
    for _ in 0..18 { let x = r.int(15) as i32; let y = r.int(15) as i32; g.set(x, y, hex("#f4f4f4")); g.set(x + 1, y + 1, hex("#8a8a8a")); }
    g
}
pub fn crate_tex(seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    g.noise(&mut r, &pal(&["#b0834f", "#a67a48", "#ba8d58"]));
    let d = hex("#6a4826");
    g.rect(0, 0, 16, 2, d); g.rect(0, 14, 16, 2, d); g.rect(0, 0, 2, 16, d); g.rect(14, 0, 2, 16, d);
    for i in 0..14 { g.set(i + 1, i + 1, hex("#7a5530")); g.set(i + 1, i, hex("#7a5530")); }
    g
}
pub fn barrel(seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    let p = pal(&["#9a6a3e", "#a57446", "#8d5f36"]);
    for x in 0..16 { let b = if x % 4 == 3 { hex("#5e3e22") } else { r.pick(&p) }; for y in 0..16 { g.set(x, y, b); } }
    g.rect(0, 2, 16, 2, hex("#4a4a50")); g.rect(0, 12, 16, 2, hex("#4a4a50"));
    g.rect(0, 2, 16, 1, hex("#7a7a84")); g.rect(0, 12, 16, 1, hex("#7a7a84"));
    g
}
pub fn roof(kind: &str, seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    if kind == "thatch" {
        g.noise(&mut r, &pal(&["#c9a14a", "#b8903e", "#d6b05a", "#a8812f"]));
        for _ in 0..30 { let x = r.int(16) as i32; let y = r.int(14) as i32; g.set(x, y, hex("#e2c278")); g.set(x, y + 1, hex("#8a6a2a")); }
        g.rect(0, 7, 16, 1, hex("#7a5a22")); g.rect(0, 15, 16, 1, hex("#7a5a22"));
        return g;
    }
    let p = if kind == "blue" { pal(&["#4c6d8e", "#557a9c", "#43617f"]) } else { pal(&["#b8492f", "#c4553a", "#a84129"]) };
    for row in 0..4i32 {
        let off = if row % 2 == 1 { 2 } else { 0 };
        for x in 0..16i32 {
            let tile = (x + off) / 4; let base = p[((tile + row) % p.len() as i32) as usize];
            for y in 0..4 {
                let mut c = shade(base, 0.95 + r.f() * 0.1);
                if y == 3 { c = shade(base, 0.6); }
                if (x + off) % 4 == 0 { c = shade(base, 0.75); }
                if y == 0 { c = shade(base, 1.15); }
                g.set(x, row * 4 + y, c);
            }
        }
    }
    g
}
/// Awning stripes with a scalloped lower edge (transparent below).
pub fn stripes(a: &str, b: &str) -> Img {
    let mut g = Img::new(16, 16);
    let (a, b) = (hex(a), hex(b));
    for x in 0..16i32 {
        let s = 13 + if x % 4 == 1 || x % 4 == 2 { 2 } else { 1 };
        for y in 0..s { g.set(x, y, if (x / 4) % 2 == 1 { b } else { a }); }
    }
    g
}

/// Alpha marks emissive pixels for the lit shader: 200 = window glow at night.
pub const A_WINDOW: u8 = 200;
pub const A_LAMP: u8 = 150;
pub const A_GLOW: u8 = 100;

/// Timber-framed plaster wall; window panes are tagged emissive through alpha.
pub fn wall(w_tiles: usize, kind: &str, seed: u32) -> Img {
    let (w, h) = (w_tiles * 16, 32usize);
    let (wi, hi) = (w as i32, h as i32);
    let mut r = Rng::new(seed);
    let mut g = Img::new(w, h);
    g.noise(&mut r, &pal(&["#ece0c4", "#e4d6b8", "#f1e7cf", "#ddcfb0"]));
    let beam = hex("#5a3a22");
    g.rect(0, 0, wi, 2, beam); g.rect(0, 15, wi, 2, beam); g.rect(0, hi - 2, wi, 2, beam);
    g.rect(0, 0, 2, hi, beam); g.rect(wi - 2, 0, 2, hi, beam);
    let mut x = 16; while x < wi - 4 { g.rect(x, 0, 2, hi, beam); x += 16; }
    let base = pal(&["#8d877e", "#9b958c", "#7c776e"]);
    for x in 0..wi { for y in hi - 5..hi { let c = r.pick(&base); g.set(x, y, c); } }
    let flowers = pal(&["#d9475a", "#f2c94c", "#3f7628", "#e8789a"]);
    let win = |g: &mut Img, r: &mut Rng, x: i32, y: i32| {
        let frame = hex("#4a2e1a");
        g.rect(x - 1, y - 1, 8, 9, frame);
        let mut pane = hex("#2c3b52"); pane[3] = A_WINDOW;
        g.rect(x, y, 6, 7, pane);
        let mut hl = hex("#5b7394"); hl[3] = A_WINDOW;
        g.rect(x, y, 2, 2, hl);
        g.rect(x + 3, y, 1, 7, frame); g.rect(x, y + 3, 6, 1, frame);
        for i in 0..8 { g.set(x - 1 + i, y + 8, hex("#6a4826")); let c = r.pick(&flowers); g.set(x - 1 + i, y + 7, c); }
    };
    let door = |g: &mut Img, r: &mut Rng, x: i32| {
        g.rect(x - 1, hi - 15, 10, 15, hex("#3a2414"));
        let dp = pal(&["#8a5a30", "#7d5129"]);
        for y in hi - 14..hi { for i in 0..8 { let c = if i % 3 == 2 { hex("#5e3a1e") } else { r.pick(&dp) }; g.set(x + i, y, c); } }
        g.set(x + 6, hi - 8, hex("#f2c94c"));
        let mut lit = hex("#e8c070"); lit[3] = A_WINDOW;
        g.rect(x + 2, hi - 13, 4, 2, lit);
    };
    if kind == "front" {
        let mid = wi / 2 - 4;
        door(&mut g, &mut r, mid);
        let mut x = 6; while x < wi - 10 { if (x - mid).abs() > 10 { win(&mut g, &mut r, x, 18); } x += 16; }
        let mut x = 6; while x < wi - 6 { win(&mut g, &mut r, x + 2, 4); x += 16; }
    } else {
        let mut x = 5; while x < wi - 8 { win(&mut g, &mut r, x, 4); win(&mut g, &mut r, x, 18); x += 16; }
    }
    g
}

// ---------- tiny 3x5 bitmap font for signs ----------
fn glyph(ch: char) -> &'static str {
    match ch {
        'A' => "010101111101101", 'B' => "110101110101110", 'C' => "011100100100011", 'D' => "110101101101110",
        'E' => "111100110100111", 'F' => "111100110100100", 'G' => "011100101101011", 'H' => "101101111101101",
        'I' => "111010010010111", 'K' => "101101110101101", 'L' => "100100100100111", 'M' => "101111111101101",
        'N' => "110101101101101", 'O' => "010101101101010", 'P' => "110101110100100", 'R' => "110101110101101",
        'S' => "011100010001110", 'T' => "111010010010010", 'U' => "101101101101111", 'V' => "101101101101010",
        'W' => "101101111111101", 'Y' => "101101010010010", '\'' => "010010000000000", _ => "000000000000000",
    }
}
pub fn sign(text: &str, w: usize, h: usize) -> Img {
    let mut r = Rng::new(text.len() as u32 * 31);
    let bg = hex("#7a5232"); let fg = hex("#f3e3b8"); let edge = hex("#3e2714");
    let mut g = Img::new(w, h);
    for y in 0..h as i32 { for x in 0..w as i32 { let c = shade(bg, 0.9 + r.f() * 0.2); g.set(x, y, c); } }
    g.rect(0, 0, w as i32, 1, edge); g.rect(0, h as i32 - 1, w as i32, 1, edge); g.rect(0, 0, 1, h as i32, edge); g.rect(w as i32 - 1, 0, 1, h as i32, edge);
    let tw = text.len() as i32 * 4 - 1;
    let mut x0 = (w as i32 - tw) / 2; let y0 = (h as i32 - 5) / 2;
    for ch in text.chars() {
        for (i, b) in glyph(ch).bytes().enumerate() {
            if b == b'1' { let (gx, gy) = (x0 + (i % 3) as i32, y0 + (i / 3) as i32); g.set(gx, gy + 1, hex("#2a1a0c")); g.set(gx, gy, fg); }
        }
        x0 += 4;
    }
    g
}

// ---------- characters ----------
#[derive(Clone)]
pub struct Look { pub coat: Rgba, pub pants: Rgba, pub boots: Rgba, pub skin: Rgba, pub hair: Rgba,
    pub hat: Option<Rgba>, pub band: Option<Rgba>, pub beard: Option<Rgba>, pub apron: Option<Rgba>, pub basket: bool }

#[derive(PartialEq, Clone, Copy)]
enum Dir { Down, Up, Side }

fn draw_char(p: &Look, dir: Dir, frame: i32) -> Img {
    let mut s = Img::new(16, 24);
    let b = if frame == 1 || frame == 2 { 1 } else { 0 };
    let dk = |c: Rgba| shade(c, 0.72);
    let (coat, skin) = (p.coat, p.skin);
    let leg = |s: &mut Img, x: i32, y: i32, w: i32, h: i32| { s.rect(x, y, w, h, p.pants); s.rect(x, y + h, w, 1, p.boots); };
    if dir == Dir::Side {
        match frame { 1 => { leg(&mut s, 5, 18, 2, 3); leg(&mut s, 8, 18, 2, 4); s.set(10, 22, p.boots); }
            2 => { leg(&mut s, 8, 18, 2, 3); leg(&mut s, 5, 18, 2, 4); s.set(7, 22, p.boots); }
            _ => { leg(&mut s, 6, 18, 3, 4); s.set(9, 22, p.boots); } }
    } else {
        match frame { 1 => { leg(&mut s, 5, 18, 3, 3); leg(&mut s, 8, 18, 3, 4); }
            2 => { leg(&mut s, 5, 18, 3, 4); leg(&mut s, 8, 18, 3, 3); }
            _ => { leg(&mut s, 5, 18, 3, 4); leg(&mut s, 8, 18, 3, 4); } }
    }
    let hat = |s: &mut Img| if let Some(h) = p.hat {
        s.rect(5, 2 + b, 6, 3, h); s.rect(5, 4 + b, 6, 1, p.band.unwrap_or(dk(h))); s.rect(3, 5 + b, 10, 1, dk(h));
    };
    if dir == Dir::Side {
        s.rect(5, 12 + b, 6, 6, coat); s.rect(5, 17 + b, 6, 1, dk(coat));
        if let Some(a) = p.apron { s.rect(9, 13 + b, 2, 5, a); }
        if p.basket { s.rect(3, 13 + b, 3, 4, hex("#b08a4a")); s.rect(3, 14 + b, 3, 1, hex("#8a6a32")); }
        s.rect(7, 11 + b, 2, 1, skin);
        s.rect(5, 6 + b, 6, 6, skin); s.set(11, 9 + b, skin);
        s.rect(4, 5 + b, 7, 2, p.hair); s.rect(4, 7 + b, 2, 4, p.hair);
        s.set(9, 8 + b, hex("#2a1d1a"));
        if let Some(bd) = p.beard { s.rect(7, 10 + b, 5, 3, bd); }
        hat(&mut s);
        if frame == 3 { s.rect(9, 13 + b, 3, 1, dk(coat)); s.set(12, 13 + b, skin); }
        else { let ax = match frame { 1 => 8, 2 => 6, _ => 7 }; s.rect(ax, 13 + b, 2, 4, dk(coat)); s.rect(ax, 17 + b, 2, 1, skin); }
    } else {
        s.rect(4, 12 + b, 8, 6, coat); s.rect(11, 12 + b, 1, 6, dk(coat)); s.rect(4, 17 + b, 8, 1, dk(coat));
        if dir == Dir::Down { s.rect(8, 13 + b, 1, 4, dk(coat)); if let Some(a) = p.apron { s.rect(5, 14 + b, 6, 5, a); } }
        if dir == Dir::Up && p.basket { s.rect(5, 13 + b, 6, 4, hex("#b08a4a")); s.rect(5, 14 + b, 6, 1, hex("#8a6a32")); s.rect(5, 16 + b, 6, 1, hex("#8a6a32")); }
        s.rect(7, 11 + b, 2, 1, skin);
        s.rect(5, 6 + b, 6, 6, skin);
        if dir == Dir::Down {
            s.rect(4, 5 + b, 8, 2, p.hair); s.rect(4, 7 + b, 1, 3, p.hair); s.rect(11, 7 + b, 1, 3, p.hair);
            s.set(6, 8 + b, hex("#2a1d1a")); s.set(9, 8 + b, hex("#2a1d1a"));
            s.set(5, 10 + b, hex("#e8968a")); s.set(10, 10 + b, hex("#e8968a"));
            if let Some(bd) = p.beard { s.rect(5, 10 + b, 6, 3, bd); }
        } else { s.rect(4, 5 + b, 8, 7, p.hair); }
        hat(&mut s);
        if frame == 3 {
            if dir == Dir::Down { s.rect(5, 14 + b, 2, 2, dk(coat)); s.rect(9, 14 + b, 2, 2, dk(coat)); s.rect(7, 15 + b, 2, 1, skin); }
            else { s.rect(3, 11 + b, 1, 3, dk(coat)); s.rect(12, 11 + b, 1, 3, dk(coat)); }
        } else {
            let la = if frame == 1 { -1 } else { 0 }; let ra = if frame == 2 { -1 } else { 0 };
            s.rect(3, 13 + b + la, 1, 4, dk(coat)); s.set(3, 17 + b + la, skin);
            s.rect(12, 13 + b + ra, 1, 4, dk(coat)); s.set(12, 17 + b + ra, skin);
        }
    }
    s.outline(hex("#1e1612"));
    s
}

/// Sprite sheet: rows = down, up, left, right; cols = stand, walkA, walkB, cast.
pub fn character_sheet(p: &Look) -> Img {
    let mut g = Img::new(64, 96);
    let rows = [(Dir::Down, false), (Dir::Up, false), (Dir::Side, true), (Dir::Side, false)];
    for (r, (d, flip)) in rows.iter().enumerate() {
        for f in 0..4 { g.blit(&draw_char(p, *d, f), f * 16, r as i32 * 24, *flip); }
    }
    g
}

pub fn cat_sheet() -> Img {
    let mut g = Img::new(32, 16);
    for f in 0..2 {
        let mut s = Img::new(16, 16);
        let (fur, dk) = (hex("#e0954a"), hex("#b06a2a"));
        s.rect(3, 9, 9, 4, fur); s.rect(3, 12, 9, 1, dk);
        s.rect(9, 5, 5, 5, fur); s.set(9, 4, fur); s.set(13, 4, fur);
        s.set(10, 7, hex("#2a1d1a")); s.set(12, 7, hex("#2a1d1a")); s.set(11, 8, hex("#e8968a"));
        s.rect(4, 10, 6, 1, dk);
        if f == 0 { s.rect(1, 7, 1, 3, fur); s.set(2, 9, fur); } else { s.rect(1, 8, 2, 1, fur); s.set(0, 7, fur); }
        s.set(4, 13, fur); s.set(10, 13, fur);
        s.outline(hex("#2a1a10"));
        g.blit(&s, f * 16, 0, false);
    }
    g
}

// ---------- fish icons (32 x 16) ----------
#[derive(Clone, Copy, PartialEq)]
pub enum Shape { Fish, Round, Flat, Squid, Boot }
#[derive(Clone, Copy, PartialEq)]
pub enum Pattern { None, Stripes, HStripe, Spots, Glow }
pub struct FishLook { pub id: &'static str, pub h: i32, pub shape: Shape, pub pattern: Pattern,
    pub body: &'static str, pub belly: &'static str, pub fin: &'static str, pub accent: Option<&'static str> }

pub fn fish_icon(sp: &FishLook, silhouette: bool) -> Img {
    let mut s = Img::new(32, 16);
    let mut r = Rng::new(sp.id.len() as u32 * 97 + sp.id.as_bytes()[0] as u32);
    let (body, belly, fin) = (hex(sp.body), hex(sp.belly), hex(sp.fin));
    let dk = |c: Rgba| shade(c, 0.7);
    match sp.shape {
        Shape::Boot => {
            let b = hex("#6b4a2f");
            s.rect(10, 2, 7, 9, b); s.rect(10, 9, 14, 4, b); s.rect(10, 12, 14, 1, hex("#3a2614"));
            s.rect(10, 2, 7, 1, hex("#8a6a42"));
            for (x, y) in [(12, 5), (14, 5), (12, 7), (14, 7)] { s.set(x, y, hex("#c9b28a")); }
            s.set(22, 8, hex("#4a8a3a")); s.set(23, 7, hex("#4a8a3a"));
        }
        Shape::Squid => {
            for x in 14..29 {
                let hh = (4.0 * (std::f32::consts::PI * (x - 13) as f32 / 17.0).sin() + 0.5).round() as i32;
                s.rect(x, 8 - hh, 1, hh * 2, if x > 25 { fin } else { body });
            }
            s.rect(26, 3, 3, 2, fin); s.rect(26, 11, 3, 2, fin);
            for t in 0..4 { let y = 5 + t * 2; for x in 3..15 { s.set(x, y + ((x as f32 * 0.8 + t as f32).sin() * 0.8).round() as i32, belly); } }
            s.set(16, 7, hex("#ffffff")); s.set(16, 8, hex("#1a1a1a"));
        }
        _ => {
            let (x0, x1, cy) = (7, if sp.shape == Shape::Round { 24 } else { 27 }, 8);
            let len = (x1 - x0) as f32;
            let accent = sp.accent.map(hex).unwrap_or(fin);
            for x in x0..=x1 {
                let t = (x - x0) as f32 / len;
                let q = 1.0 - ((t - 0.52) / 0.55).powi(2);
                let prof = ((sp.h as f32 * q.max(0.0).sqrt()).round() as i32).max(1);
                for y in cy - prof..=cy + prof {
                    let mut c = if y as f32 > cy as f32 + prof as f32 * 0.15 { belly } else { body };
                    if (y as f32) < cy as f32 - prof as f32 * 0.55 { c = dk(body); }
                    if sp.pattern == Pattern::Stripes && x % 4 == 0 && y < cy { c = dk(body); }
                    if sp.pattern == Pattern::HStripe && y == cy { c = accent; }
                    s.set(x, y, c);
                }
            }
            if matches!(sp.pattern, Pattern::Spots | Pattern::Glow) {
                let spot = sp.accent.map(hex).unwrap_or(hex("#ffffff"));
                for _ in 0..9 {
                    let x = x0 + 2 + r.int((len as usize).saturating_sub(4).max(1)) as i32; let y = cy - 3 + r.int(5) as i32;
                    if s.get(x, y)[3] > 0 { s.set(x, y, spot); }
                }
            }
            for x in 2..x0 { let th = ((x0 - x) as f32 * 0.8).round() as i32 + 1; s.rect(x, cy - th, 1, th * 2 + 1, fin); }
            let top = cy - sp.h;
            for i in 0..6 { s.set(x0 + 5 + i, top - 1 - if i < 3 { 1 } else { 0 }, fin); }
            s.rect(x0 + 6, top - 1, 6, 1, fin);
            s.rect(x0 + 8, cy + sp.h, 3, 1, fin);
            if sp.shape == Shape::Round {
                for i in 0..8 { let a = i as f32 / 8.0 * std::f32::consts::TAU;
                    s.set((16.0 + a.cos() * (sp.h + 2) as f32).round() as i32, (cy as f32 + a.sin() * (sp.h + 1) as f32).round() as i32, fin); }
            }
            let ex = x1 - 4;
            s.set(ex, cy - 2, hex("#ffffff")); s.set(ex + 1, cy - 2, hex("#1a1a1a"));
            if sp.shape == Shape::Flat { s.set(ex - 2, cy - 3, hex("#ffffff")); s.set(ex - 1, cy - 3, hex("#1a1a1a")); }
            s.set(x1, cy + 1, dk(belly));
        }
    }
    let line = hex("#1b1b24");
    s.outline(line);
    if silhouette { for c in s.px.iter_mut() { if c[3] > 0 { *c = if *c == line { hex("#0f1118") } else { hex("#2e3442") }; } } }
    s
}

pub fn tuft(kind: &str, seed: u32) -> Img {
    let mut r = Rng::new(seed); let mut g = Img::new(16, 16);
    let greens = pal(&["#5a9a36", "#71b343", "#4a8530", "#86c653"]);
    for _ in 0..7 {
        let mut x = 2 + r.int(12) as i32; let h = 5 + r.int(8) as i32;
        let lean = if r.f() < 0.5 { -1 } else { 1 }; let col = r.pick(&greens);
        for y in 0..h { g.set(x, 15 - y, col); if y > h / 2 && r.f() < 0.4 { x += lean; } }
    }
    let petal = match kind { "red" => Some("#e0475a"), "yellow" => Some("#f4d04a"), "white" => Some("#f4f0ea"), "blue" => Some("#7a9ae8"), _ => None };
    if let Some(p) = petal {
        let p = hex(p);
        for _ in 0..3 {
            let x = 3 + r.int(10) as i32; let y = 3 + r.int(6) as i32;
            for k in y + 2..16 { g.set(x, k, hex("#4a8530")); }
            for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] { g.set(x + dx, y + dy, p); }
            g.set(x, y, hex("#f4b43a"));
        }
    }
    g
}

/// Solid swatch used for flat-colored geometry (tinted by vertex color).
pub fn swatch(alpha: u8) -> Img { let mut g = Img::new(4, 4); g.rect(0, 0, 4, 4, [255, 255, 255, alpha]); g }
