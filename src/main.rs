//! Driftmoor Cove (Rust edition): an HD-2D fishing game compiled to WebAssembly.
mod audio;
mod fishing;
mod geo;
mod gfx;
mod pixel;
mod platform;
mod ui;
mod world;

use audio::{Audio, Sfx};
use fishing::{Species, Zone, RODS};
use geo::{cube1, v3, Atlas, Geo, Uv};
use gfx::{Frame, Gfx, LitU, PGeo, MAX_LAMPS};
use macroquad::miniquad;
use macroquad::prelude::*;
use nanoserde::{DeJson, SerJson};
use pixel::{hex, Look};
use world::{World, PLAY_X0, PLAY_X1, T};

// ---------- save data ----------
#[derive(SerJson, DeJson, Clone)]
pub struct BagFish { pub id: String, pub size: f32, pub value: u32 }
#[derive(SerJson, DeJson, Clone)]
pub struct Entry { pub id: String, pub n: u32, pub best: f32 }
#[derive(SerJson, DeJson, Clone)]
pub struct Save { pub coins: u32, pub rod: usize, pub bag: Vec<BagFish>, pub journal: Vec<Entry>, pub total: u32, pub earned: u32, pub day: u32, pub hour: f32 }
impl Default for Save {
    fn default() -> Self { Save { coins: 0, rod: 0, bag: vec![], journal: vec![], total: 0, earned: 0, day: 1, hour: 7.0 } }
}
impl Save {
    pub fn entry(&self, id: &str) -> Option<&Entry> { self.journal.iter().find(|e| e.id == id) }
}

// ---------- actors ----------
#[derive(Clone, Copy, PartialEq)]
pub enum Face { Down, Up, Left, Right }
impl Face {
    fn row(self) -> usize { match self { Face::Down => 0, Face::Up => 1, Face::Left => 2, Face::Right => 3 } }
    pub fn dir(self) -> Vec3 { match self { Face::Down => v3(0., 0., 1.), Face::Up => v3(0., 0., -1.), Face::Left => v3(-1., 0., 0.), Face::Right => v3(1., 0., 0.) } }
}
#[derive(Clone, Copy, PartialEq)]
pub enum Pose { None, Cast, Cheer }

pub struct Actor { pub pos: Vec3, pub face: Face, pub walk_t: f32, pub moving: bool, pub running: bool, pub pose: Pose, pub sheet: Uv, pub name: &'static str }
impl Actor {
    fn new(sheet: Uv, x: f32, z: f32, name: &'static str) -> Actor { Actor { pos: v3(x, 0., z), face: Face::Down, walk_t: 0., moving: false, running: false, pose: Pose::None, sheet, name } }
    fn frame_uv(&mut self, dt: f32) -> Uv {
        let col = match self.pose {
            Pose::Cast => 3,
            Pose::Cheer => { self.face = Face::Down; 3 }
            Pose::None if self.moving => { self.walk_t += dt * if self.running { 11.0 } else { 8.0 }; [1, 0, 2, 0][self.walk_t as usize % 4] }
            _ => 0,
        };
        let r = self.face.row() as f32;
        self.sheet.sub(col as f32 / 4.0, r / 4.0, (col + 1) as f32 / 4.0, (r + 1.0) / 4.0)
    }
}

// ---------- fishing state ----------
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FState { Idle, Charge, Flight, Wait, Bite, Reel, Reward }

pub struct Fish {
    pub state: FState, pub t: f32, pub power: f32, power_dir: f32, ft: f32, from: Vec3, target: Vec3, dist: f32, dry: bool,
    pub fish: usize, timer: f32, nibble_t: f32, dip: f32, window: f32, pub tension: f32, pub progress: f32, pub run: bool, phase: f32, tick_t: f32,
    hand: Vec3, tip: Vec3, bob: Vec3,
}

pub enum Action { Nothing, Sell(u32), Buy }
pub struct Choice { pub label: String, pub disabled: bool, pub action: Action }
pub struct Dialog { pub name: String, pub lines: Vec<String>, pub line: usize, pub sel: usize, pub choices: Vec<Choice> }

pub struct CatchCard { pub fish: usize, pub size: f32, pub value: u32, pub new: bool }

struct Particle { p: Vec3, v: Vec3, life: f32, seed: f32 }
struct Ripple { p: Vec3, t: f32, size: f32 }

#[derive(Default)]
pub struct Act { pub held: bool, pub pressed: bool, pub ui_press: bool }

pub struct Touch { pub stick: Option<(u64, Vec2, Vec2)>, pub a_id: Option<u64>, pub used: bool }

pub struct Game {
    pub world: World, gfx: Gfx, pub audio: Audio, pub species: Vec<Species>,
    pub actors: Vec<Actor>, cat: Actor, fish_uv: Vec<Uv>, white: Uv,
    pub icons: Vec<Texture2D>, pub silhouettes: Vec<Texture2D>,
    pub save: Save, pub f: Fish,
    pub dialog: Option<Dialog>, pub journal: bool, pub help: bool, pub toast: (String, f32), pub card: Option<CatchCard>,
    pub started: bool, pub title_fade: f32, time: f32, pub held_fish: Option<usize>,
    cam_target: Vec3, shake: f32, pub vp: Mat4, pub env_night: f32,
    pub act: Act, pub touch: Touch, tip_idx: usize, save_t: f32, report_t: f32, rand: pixel::Rng,
    sparks: Vec<Particle>, leaves: Vec<Particle>, smoke: Vec<Particle>, drops: Vec<Particle>, mist: Vec<Particle>, ripples: Vec<Ripple>,
    dyn_geo: Geo, alpha: PGeo, add: PGeo, boat_t: f32, pub prompt: Option<String>,
}

fn srgb(c: [u8; 4]) -> Vec3 { v3(c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0) }
fn lin(c: Vec3) -> Vec3 { v3(c.x.powf(2.2), c.y.powf(2.2), c.z.powf(2.2)) }
fn arr(v: Vec3) -> [f32; 3] { [v.x, v.y, v.z] }

// time, sky, sun color, sun intensity, hemi sky, hemi ground, hemi intensity, night
const KEYS: [(f32, &str, &str, f32, &str, &str, f32, f32); 9] = [
    (0.0, "#0b1230", "#7088c8", 0.55, "#34488a", "#141826", 0.75, 1.0),
    (5.0, "#1c2048", "#7a88c8", 0.5, "#3a4478", "#16161e", 0.7, 0.9),
    (6.3, "#f0a878", "#ffb07a", 1.4, "#f8c8a0", "#4a4a3a", 0.7, 0.3),
    (8.0, "#f6d0a0", "#ffe2b0", 2.6, "#ffe8cc", "#5a6040", 0.95, 0.0),
    (12.0, "#a8d4f0", "#fff6e8", 3.1, "#dbeeff", "#5a6b3a", 1.05, 0.0),
    (16.5, "#f2c890", "#ffd8a0", 2.6, "#ffe2c0", "#5a5a3a", 0.95, 0.0),
    (18.6, "#e88a68", "#ff9a5a", 1.4, "#f0a080", "#3a3040", 0.7, 0.35),
    (20.0, "#2a2450", "#8a7ac8", 0.55, "#3e3a70", "#16141e", 0.7, 0.9),
    (24.0, "#0b1230", "#7088c8", 0.55, "#34488a", "#141826", 0.75, 1.0),
];

struct Env { sky: Vec3, sun: Vec3, sun_i: f32, hemi_s: Vec3, hemi_g: Vec3, hemi_i: f32, night: f32 }
fn env_at(h: f32) -> Env {
    let mut k = 0; while k < KEYS.len() - 2 && KEYS[k + 1].0 <= h { k += 1; }
    let (a, b) = (KEYS[k], KEYS[k + 1]);
    let t = (h - a.0) / (b.0 - a.0);
    let mix = |x: &str, y: &str| lin(srgb(hex(x))).lerp(lin(srgb(hex(y))), t);
    Env { sky: srgb(hex(a.1)).lerp(srgb(hex(b.1)), t), sun: mix(a.2, b.2), sun_i: a.3 + (b.3 - a.3) * t, hemi_s: mix(a.4, b.4), hemi_g: mix(a.5, b.5), hemi_i: a.6 + (b.6 - a.6) * t, night: a.7 + (b.7 - a.7) * t }
}

const TIPS: [&str; 6] = [
    "The deep water past the end of this pier holds the big ones. Bream by day, squid by night.",
    "Folk say a golden koi shows itself in the pond, but only when the sky turns orange.",
    "When a fish runs, let the line go slack. Fight it and you'll snap your line.",
    "An old coelacanth swims these waters at dawn and dusk. Nobody's landed one in forty years.",
    "Marla at the market pays fair. Save up and get yourself a sturdier rod.",
    "Lantern eels glow down deep after dark. Fierce fighters, those.",
];

fn conf() -> miniquad::conf::Conf {
    miniquad::conf::Conf {
        window_title: "Driftmoor Cove".into(), window_width: 1280, window_height: 720, high_dpi: false,
        platform: miniquad::conf::Platform { webgl_version: miniquad::conf::WebGLVersion::WebGL2, ..Default::default() },
        ..Default::default()
    }
}

#[macroquad::main(conf)]
async fn main() {
    let mut g = Game::new().await;
    loop {
        let dt = get_frame_time().min(0.05);
        g.update(dt);
        g.render(dt);
        g.draw_ui();
        next_frame().await;
    }
}

impl Game {
    async fn new() -> Game {
        rand::srand(miniquad::date::now() as u64);
        simulate_mouse_with_touch(false);
        let mut atlas = Atlas::new(1024);
        let world = World::new(&mut atlas);
        let look = |coat: &str, pants: &str, boots: &str, skin: &str, hair: &str, hat: Option<&str>, band: Option<&str>, beard: Option<&str>, apron: Option<&str>, basket: bool| Look {
            coat: hex(coat), pants: hex(pants), boots: hex(boots), skin: hex(skin), hair: hex(hair), hat: hat.map(hex), band: band.map(hex), beard: beard.map(hex), apron: apron.map(hex), basket };
        let sheets = [
            look("#e9b528", "#2f4f5a", "#4a3326", "#f2c9a0", "#8a4a2a", Some("#6d7b3a"), Some("#4a5426"), None, None, true),
            look("#c8463a", "#4a3a5a", "#3a2a20", "#e8b88a", "#3a2418", Some("#3f6aa8"), Some("#2f4f80"), None, Some("#f2ead8"), false),
            look("#2f3f5f", "#3a3a3a", "#2a2020", "#e8b890", "#c8c8c0", Some("#1f2a44"), Some("#e4e4dc"), Some("#e4e4dc"), None, false),
            look("#5cb85c", "#6a4a2a", "#3a2a20", "#f6d0a8", "#e8c060", None, None, None, None, false),
        ].map(|l| atlas.add(&pixel::character_sheet(&l)));
        let cat_uv = atlas.add(&pixel::cat_sheet());
        let species = fishing::species();
        let fish_uv: Vec<Uv> = species.iter().map(|s| atlas.add(&pixel::fish_icon(&s.look, false))).collect();
        let white = atlas.add(&pixel::swatch(255));
        let mk = |img: &pixel::Img| { let t = Texture2D::from_rgba8(img.w as u16, img.h as u16, &img.bytes()); t.set_filter(FilterMode::Nearest); t };
        let icons = species.iter().map(|s| mk(&pixel::fish_icon(&s.look, false))).collect();
        let silhouettes = species.iter().map(|s| mk(&pixel::fish_icon(&s.look, true))).collect();

        let mut info = vec![0u8; (world::W * world::H * 4) as usize];
        for k in 0..(world::W * world::H) as usize {
            info[k * 4] = (world.dist[k].min(10.0) * 24.0).round().min(255.0) as u8;
            info[k * 4 + 1] = if world.ty[k] == T::Fresh { 255 } else { 0 };
            info[k * 4 + 3] = 255;
        }
        let gfx = {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            Gfx::new(gl.quad_context, &atlas.img.bytes(), atlas.img.w as u16, &world.mesh, &world.falls, &info, world::W as u16, world::H as u16)
        };
        let audio = Audio::new().await;

        let s0 = world.dock_start as f32;
        let (spawn, stall) = (world.spawn, world.stall);
        let mut actors = vec![
            Actor::new(sheets[0], spawn.x, spawn.z, "You"),
            Actor::new(sheets[1], stall.x, stall.z - 1.0, "Marla"),
            Actor::new(sheets[2], 40.5, s0 + 8.5, "Old Tobin"),
            Actor::new(sheets[3], 27.5, 24.5, "Pip"),
        ];
        actors[3].face = Face::Left;
        let save = platform::load().and_then(|s| Save::deserialize_json(&s).ok()).unwrap_or_default();
        let mut g = Game {
            world, gfx, audio, species, actors, cat: Actor::new(cat_uv, 38.6, s0 + 1.2, "cat"), fish_uv, white, icons, silhouettes, save,
            f: Fish { state: FState::Idle, t: 0., power: 0., power_dir: 1., ft: 0., from: Vec3::ZERO, target: Vec3::ZERO, dist: 0., dry: false, fish: 0, timer: 0.,
                nibble_t: 0., dip: 0., window: 0., tension: 0., progress: 0., run: false, phase: 0., tick_t: 0., hand: Vec3::ZERO, tip: Vec3::ZERO, bob: Vec3::ZERO },
            dialog: None, journal: false, help: false, toast: (String::new(), 0.), card: None, started: false, title_fade: 1.0, time: 0., held_fish: None,
            cam_target: v3(spawn.x, 0., spawn.z), shake: 0., vp: Mat4::IDENTITY, env_night: 0., act: Act::default(),
            touch: Touch { stick: None, a_id: None, used: false }, tip_idx: 0, save_t: 0., report_t: 0., rand: pixel::Rng::new(miniquad::date::now() as u32 | 1),
            sparks: vec![], leaves: vec![], smoke: vec![], drops: vec![], mist: vec![], ripples: vec![],
            dyn_geo: Geo::new(), alpha: PGeo::default(), add: PGeo::default(), boat_t: 0., prompt: None,
        };
        for a in 1..4 { let p = g.actors[a].pos; g.world.block(p.x.floor() as i32, p.z.floor() as i32); }
        g.init_particles();
        g.apply_query();
        g
    }

    /// `?hour=18&at=dock&start=1` style overrides, handy for screenshots and automated play-testing.
    fn apply_query(&mut self) {
        let q = platform::query();
        if let Some(h) = platform::param(&q, "hour").and_then(|h| h.parse::<f32>().ok()) { self.save.hour = h.rem_euclid(24.0); }
        let s0 = self.world.dock_start as f32;
        let spot = match platform::param(&q, "at").as_deref() {
            Some("dock") => Some((37.5, s0 + 8.5, Face::Down)),
            Some("shop") => Some((41.5, 28.6, Face::Up)),
            Some("pond") => Some((26.0, 22.5, Face::Left)),
            Some("plaza") => Some((38.0, 24.0, Face::Down)),
            _ => None,
        };
        if let Some((x, z, f)) = spot { self.actors[0].pos = v3(x, 0., z); self.actors[0].face = f; self.cam_target = v3(x, 0., z); }
        if platform::param(&q, "start").is_some() { self.start(); self.title_fade = 0.0; }
    }

    pub fn rnd(&mut self) -> f32 { self.rand.f() }
    pub fn rod(&self) -> &'static fishing::Rod { &RODS[self.save.rod.min(RODS.len() - 1)] }
    pub fn persist(&self) { platform::save(&self.save.serialize_json()); }
    pub fn toast(&mut self, s: &str) { self.toast = (s.to_string(), 2.4); }
    pub fn start(&mut self) {
        if self.started { return; }
        self.started = true;
        self.audio.start();
        let msg = if self.save.total > 0 { "Welcome back to Driftmoor Cove!" } else { "Walk to the water and press Space to fish." };
        self.toast = (msg.into(), 3.5);
    }
    pub fn any_panel(&self) -> bool { self.journal || self.help }

    // ---------- input ----------
    fn read_input(&mut self) -> Vec2 {
        const ACTION: [KeyCode; 4] = [KeyCode::Space, KeyCode::E, KeyCode::Enter, KeyCode::Z];
        let (sw, sh) = (screen_width(), screen_height());
        let mut pressed = ACTION.iter().any(|k| is_key_pressed(*k));
        let mut held = ACTION.iter().any(|k| is_key_down(*k));
        if !self.started && (get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left)) { self.start(); pressed = false; }
        let a_center = vec2(sw - 70.0, sh - 80.0);
        for t in touches() {
            self.touch.used = true;
            match t.phase {
                TouchPhase::Started => {
                    if !self.started { self.start(); continue; }
                    if t.position.distance(a_center) < 60.0 { self.touch.a_id = Some(t.id); pressed = true; }
                    else if self.touch.stick.is_none() && t.position.x < sw * 0.6 && !self.ui_hit(t.position) { self.touch.stick = Some((t.id, t.position, Vec2::ZERO)); }
                }
                TouchPhase::Moved | TouchPhase::Stationary => {
                    if let Some((id, o, _)) = self.touch.stick { if id == t.id {
                        let d = (t.position - o).clamp_length_max(50.0);
                        self.touch.stick = Some((id, o, d / 50.0));
                    } }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if self.touch.stick.map_or(false, |s| s.0 == t.id) { self.touch.stick = None; }
                    if self.touch.a_id == Some(t.id) { self.touch.a_id = None; }
                }
            }
        }
        if self.touch.a_id.is_some() { held = true; }
        let ui_press = std::mem::take(&mut self.act.ui_press);
        self.act = Act { held, pressed: pressed || ui_press, ui_press: false };
        if is_key_pressed(KeyCode::H) { self.help = !self.help; self.journal = false; }
        if is_key_pressed(KeyCode::J) && self.f.state == FState::Idle { self.journal = !self.journal; self.help = false; }
        if is_key_pressed(KeyCode::M) { self.audio.toggle(); }
        if is_key_pressed(KeyCode::Escape) { self.journal = false; self.help = false; }
        if let Some(d) = &mut self.dialog {
            let n = d.choices.len();
            if n > 0 && d.line + 1 == d.lines.len() {
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) { d.sel = (d.sel + n - 1) % n; }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) { d.sel = (d.sel + 1) % n; }
            }
        }
        let mut m = Vec2::ZERO;
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) { m.x -= 1.0; }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) { m.x += 1.0; }
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) { m.y -= 1.0; }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) { m.y += 1.0; }
        if let Some((_, _, d)) = self.touch.stick { if d.length() > 0.25 { m = d; } }
        m
    }

    // ---------- update ----------
    fn update(&mut self, dt: f32) {
        self.time += dt;
        if self.started {
            self.title_fade = (self.title_fade - dt * 1.6).max(0.0);
            self.save.hour += dt * (24.0 / 480.0);
            if self.save.hour >= 24.0 { self.save.hour -= 24.0; self.save.day += 1; }
        }
        if self.toast.1 > 0.0 { self.toast.1 -= dt; }
        let mv = self.read_input();
        let locked = !self.started || self.dialog.is_some() || self.f.state != FState::Idle || self.any_panel();
        let p = &mut self.actors[0];
        let len = if locked { 0.0 } else { mv.length() };
        p.moving = len > 0.01;
        p.running = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) || (self.touch.stick.is_some() && len > 0.95);
        if p.moving {
            let m = mv / len.max(1.0);
            if m.x.abs() > m.y.abs() + 0.05 { p.face = if m.x > 0.0 { Face::Right } else { Face::Left }; }
            else if m.y.abs() > 0.05 { p.face = if m.y > 0.0 { Face::Down } else { Face::Up }; }
            let sp = if p.running { 5.2 } else { 3.1 } * dt;
            let r = 0.28;
            let w = &self.world;
            let ok = |x: f32, z: f32| w.walkable(x - r, z - r * 0.6) && w.walkable(x + r, z - r * 0.6) && w.walkable(x - r, z + r * 0.4) && w.walkable(x + r, z + r * 0.4);
            let (nx, nz) = (p.pos.x + m.x * sp, p.pos.z + m.y * sp);
            if ok(nx, p.pos.z) { p.pos.x = nx; }
            if ok(p.pos.x, nz) { p.pos.z = nz; }
        }

        // interaction
        self.prompt = None;
        if self.started && !self.any_panel() {
            if self.dialog.is_some() { if self.act.pressed { self.advance_dialog(); } }
            else if self.f.state != FState::Idle { self.fishing_update(dt); }
            else if let Some(n) = self.npc_in_front() {
                self.prompt = Some(format!("Talk to {}", self.actors[n].name));
                if self.act.pressed { self.talk(n); }
            } else if self.water_ahead() {
                self.prompt = Some("Cast".into());
                if self.act.pressed { self.f.state = FState::Charge; self.f.power = 0.0; self.f.power_dir = 1.0; self.actors[0].pose = Pose::Cast; }
            }
        }
        if !self.dialog.is_some() && self.time.rem_euclid(10.0) < dt && self.dialog.is_none() {
            let r = self.rnd();
            self.actors[3].face = if r > 0.5 { Face::Left } else { Face::Down };
        }
        // camera follow
        let p = self.actors[0].pos;
        let tx = p.x.clamp(PLAY_X0 as f32 + 6.0, PLAY_X1 as f32 - 6.0);
        let tz = p.z.clamp(13.0, 50.0);
        self.cam_target = self.cam_target.lerp(v3(tx, 0., tz), 1.0 - (-dt * 4.0).exp());
        if self.shake > 0.0 { self.shake = (self.shake - dt * 0.8).max(0.0); }
        self.update_particles(dt);
        self.audio.update(dt, self.time, 1.0 - self.env_night);
        self.save_t += dt;
        if self.save_t > 10.0 { self.save_t = 0.0; self.persist(); }
        self.report_t -= dt;
        if self.report_t <= 0.0 { self.report_t = 0.1; self.report(); }
    }

    fn report(&self) {
        let fish = if self.f.state == FState::Idle { String::new() } else { self.species[self.f.fish].name.to_string() };
        let caught = self.species.iter().filter(|s| self.save.entry(s.id()).is_some()).count();
        platform::report(&format!(
            r#"{{"started":{},"state":"{:?}","run":{},"tension":{:.3},"progress":{:.3},"fish":"{}","bag":{},"coins":{},"hour":{:.2},"dialog":{},"journal":{},"species":{},"total":{},"x":{:.2},"z":{:.2}}}"#,
            self.started, self.f.state, self.f.run, self.f.tension / self.rod().max_t, self.f.progress, fish, self.save.bag.len(), self.save.coins, self.save.hour,
            self.dialog.is_some(), self.journal, caught, self.save.total, self.actors[0].pos.x, self.actors[0].pos.z));
    }

    fn npc_in_front(&self) -> Option<usize> {
        let p = &self.actors[0];
        let mut best = None; let mut bd = 2.3;
        for k in 1..4 {
            let d = self.actors[k].pos - p.pos; let dist = d.length();
            if dist < bd && d.normalize_or_zero().dot(p.face.dir()) > 0.5 { best = Some(k); bd = dist; }
        }
        best
    }

    fn water_ahead(&self) -> bool {
        let p = &self.actors[0];
        let mut d = 0.8;
        while d <= 2.2 { let q = p.pos + p.face.dir() * d; if self.world.is_water(q.x.floor() as i32, q.z.floor() as i32) { return true; } d += 0.35; }
        false
    }

    // ---------- NPCs and dialog ----------
    fn talk(&mut self, n: usize) {
        self.audio.play(Sfx::Blip);
        let (a, b) = (self.actors[n].pos, self.actors[0].pos);
        let d = b - a;
        self.actors[n].face = if d.x.abs() > d.z.abs() { if d.x > 0.0 { Face::Right } else { Face::Left } } else if d.z > 0.0 { Face::Down } else { Face::Up };
        let name = self.actors[n].name.to_string();
        let dlg = |lines: Vec<String>, choices: Vec<Choice>| Dialog { name: name.clone(), lines, line: 0, sel: 0, choices };
        self.dialog = Some(match n {
            2 => {
                let line = if self.save.total > 0 { let t = TIPS[self.tip_idx % TIPS.len()]; self.tip_idx += 1; t.to_string() }
                    else { "Ahoy, youngster. First time out? Face the water, hold Space to wind up, and let go to cast.".into() };
                dlg(vec![line], vec![])
            }
            3 => {
                let caught = self.species.iter().filter(|s| self.save.entry(s.id()).is_some()).count();
                let n = self.species.len();
                dlg(if caught < n { vec![format!("You've found {caught} of {n} kinds of fish! I bet there's more in the pond."), "Press J to look at your journal.".into()] }
                    else { vec!["You caught every kind of fish?! You're the best angler in Driftmoor Cove!".into()] }, vec![])
            }
            _ => {
                let worth: u32 = self.save.bag.iter().map(|f| f.value).sum();
                let mut ch = vec![];
                if !self.save.bag.is_empty() { ch.push(Choice { label: format!("Sell {} fish for {} coins", self.save.bag.len(), worth), disabled: false, action: Action::Sell(worth) }); }
                if let Some(next) = RODS.get(self.save.rod + 1) { ch.push(Choice { label: format!("Buy {} ({} coins)", next.name, next.cost), disabled: self.save.coins < next.cost, action: Action::Buy }); }
                ch.push(Choice { label: "Just browsing".into(), disabled: false, action: Action::Nothing });
                let line = if self.save.bag.is_empty() { "Fresh fish, fair prices! Bring me what you catch and I'll pay you for it." } else { "Ooh, a fresh haul! Let's have a look." };
                dlg(vec![line.into()], ch)
            }
        });
    }

    pub fn advance_dialog(&mut self) {
        let Some(d) = &mut self.dialog else { return };
        if d.line + 1 < d.lines.len() { d.line += 1; return; }
        let d = self.dialog.take().unwrap();
        let Some(c) = d.choices.get(d.sel) else { return };
        if c.disabled { return; }
        match c.action {
            Action::Sell(w) => { self.save.coins += w; self.save.earned += w; self.save.bag.clear(); self.persist(); self.audio.play(Sfx::Coin); self.toast(&format!("+{w} coins")); }
            Action::Buy => {
                let next = &RODS[self.save.rod + 1];
                self.save.coins -= next.cost; self.save.rod += 1; self.persist(); self.audio.play(Sfx::Catch);
                self.toast = (format!("You got the {}!", next.name), 3.0);
            }
            Action::Nothing => {}
        }
    }

    // ---------- fishing ----------
    fn fishing_update(&mut self, dt: f32) {
        self.f.t += dt;
        let p = self.actors[0].pos; let dir = self.actors[0].face.dir();
        self.f.hand = v3(p.x + dir.x * 0.25, 0.75, p.z + dir.z * 0.25 + 0.05);
        let up = Vec3::Y;
        let hand = self.f.hand;
        let pressed = self.act.pressed; let held = self.act.held;
        match self.f.state {
            FState::Charge => {
                self.f.power += self.f.power_dir * dt * 1.25;
                if self.f.power >= 1.0 { self.f.power = 1.0; self.f.power_dir = -1.0; } else if self.f.power <= 0.0 { self.f.power = 0.0; self.f.power_dir = 1.0; }
                self.f.tip = hand + dir * (0.3 - self.f.power) + up * (1.1 + self.f.power * 0.35);
                if !held { self.cast(); }
            }
            FState::Flight => {
                self.f.ft += dt / 0.6;
                let k = self.f.ft.min(1.0);
                self.f.tip = hand + dir * 0.9 + up;
                self.f.bob = self.f.from.lerp(self.f.target, k) + up * ((k * std::f32::consts::PI).sin() * (1.2 + self.f.dist * 0.25));
                if k >= 1.0 { self.land(); }
            }
            FState::Wait => {
                self.f.tip = hand + dir * 0.9 + up;
                self.f.timer -= dt;
                self.f.bob = v3(self.f.target.x, -0.2 + (self.f.t * 2.5).sin() * 0.02, self.f.target.z);
                self.f.nibble_t -= dt;
                if self.f.nibble_t <= 0.0 {
                    self.f.nibble_t = 0.6 + self.rnd() * 1.5;
                    if self.f.timer > 0.6 { self.f.dip = 0.25; self.audio.play(Sfx::Nibble); let b = self.f.bob; self.ripple(b, 0.5); }
                }
                if pressed { self.toast("Too early... reeled in."); self.reset_fishing(); return; }
                if self.f.timer <= 0.0 {
                    self.f.state = FState::Bite; self.f.window = 0.85 - self.species[self.f.fish].str_ * 0.25;
                    self.audio.play(Sfx::Bite); self.audio.play(Sfx::Splash);
                    let b = self.f.bob; self.ripple(b, 1.4); self.shake = self.shake.max(0.15);
                }
            }
            FState::Bite => {
                self.f.tip = hand + dir * 0.95 + up * 0.7;
                self.f.window -= dt;
                let j = (self.rnd() - 0.5) * 0.05;
                self.f.bob = v3(self.f.target.x + j, -0.32, self.f.target.z);
                if pressed { self.hook(); }
                else if self.f.window <= 0.0 { self.toast("It got away..."); self.audio.play(Sfx::Lose); self.reset_fishing(); return; }
            }
            FState::Reel => {
                if !self.reel_update(dt) { return; }
                let rod = self.rod();
                let jit = if self.f.run { (self.rnd() - 0.5) * 0.06 } else { 0.0 };
                self.f.tip = hand + dir * 1.0 + up * (0.95 - self.f.tension / rod.max_t * 0.45 + jit);
                let sway = (self.f.t * if self.f.run { 7.0 } else { 2.0 }).sin() * if self.f.run { 0.5 } else { 0.2 };
                let side = v3(-dir.z, 0., dir.x);
                let near = v3(p.x + dir.x * 1.2, 0., p.z + dir.z * 1.2);
                self.f.bob = self.f.target.lerp(near, self.f.progress.clamp(0.0, 1.0) * 0.85) + side * sway;
                self.f.bob.y = -0.3 + (self.rnd() - 0.5) * if self.f.run { 0.08 } else { 0.02 };
                let b = self.f.bob;
                if self.rnd() < dt * if self.f.run { 8.0 } else { 2.0 } { self.splash(b, if self.f.run { 6 } else { 2 }); }
                if self.rnd() < dt * 2.0 { self.ripple(b, 0.8); }
            }
            FState::Reward => {
                self.f.tip = hand + up * 1.5;
                self.f.bob = self.f.tip;
                if pressed && self.f.t > 0.6 { self.card = None; self.held_fish = None; self.reset_fishing(); return; }
            }
            FState::Idle => {}
        }
        if self.f.dip > 0.0 { self.f.dip = (self.f.dip - dt).max(0.0); self.f.bob.y -= self.f.dip * 0.4; }
    }

    fn cast(&mut self) {
        self.audio.play(Sfx::Cast);
        let p = self.actors[0].pos; let dir = self.actors[0].face.dir();
        let dist = 1.6 + self.f.power * 5.2;
        let mut land = None;
        let mut d = dist;
        while d >= 0.8 {
            let q = p + dir * d;
            let (i, j) = (q.x.floor() as i32, q.z.floor() as i32);
            if self.world.is_water(i, j) { land = Some(v3(q.x, -0.2, q.z)); break; }
            d -= 0.25;
        }
        self.f.from = self.f.hand + dir * 0.9 + Vec3::Y;
        self.f.target = land.unwrap_or(v3(p.x + dir.x * dist, 0.05, p.z + dir.z * dist));
        self.f.dist = (self.f.target - p).with_y(0.0).length();
        self.f.dry = land.is_none();
        self.f.state = FState::Flight; self.f.ft = 0.0;
    }

    fn land(&mut self) {
        if self.f.dry { self.toast("Your line landed on dry ground."); self.reset_fishing(); return; }
        self.audio.play(Sfx::Plop);
        let t = self.f.target;
        self.ripple(t, 1.0); self.splash(t, 6);
        let (i, j) = (t.x.floor() as i32, t.z.floor() as i32);
        let zone = if self.world.typ(i, j) == T::Fresh { Zone::Fresh } else { Zone::Sea };
        let deep = self.world.depth(i, j) >= if zone == Zone::Sea { 4.0 } else { 3.0 };
        let roll = self.rnd();
        self.f.fish = fishing::choose(&self.species, zone, deep, self.save.hour, self.rod().boost, roll);
        self.f.timer = 2.0 + self.rnd() * 5.0 * (1.0 - self.rod().boost * 0.15);
        self.f.nibble_t = 0.8 + self.rnd();
        self.f.state = FState::Wait;
    }

    fn hook(&mut self) {
        self.audio.play(Sfx::Splash);
        self.f.state = FState::Reel; self.f.tension = 0.25; self.f.progress = 0.22; self.f.run = false;
        self.f.phase = 0.8 + self.rnd(); self.f.tick_t = 0.0;
    }

    /// Tension mini-game. Returns false once the encounter has ended.
    fn reel_update(&mut self, dt: f32) -> bool {
        let rod = self.rod(); let strength = self.species[self.f.fish].str_;
        self.f.phase -= dt;
        if self.f.phase <= 0.0 {
            self.f.run = !self.f.run;
            let r = self.rnd();
            self.f.phase = if self.f.run { 0.5 + r * (0.6 + strength) } else { 0.7 + r * (1.8 - strength) };
            if self.f.run { self.audio.play(Sfx::Splash); self.shake = self.shake.max(0.1 + strength * 0.15); }
        }
        let pull = 0.5 + strength * 1.7;
        if self.act.held {
            self.f.progress += rod.reel * dt * if self.f.run { 0.25 } else { 1.0 } * (1.15 - strength * 0.4);
            self.f.tension += if self.f.run { 0.55 * pull } else { 0.28 } * dt;
            self.f.tick_t -= dt;
            if self.f.tick_t <= 0.0 { self.audio.play(Sfx::Tick); self.f.tick_t = 0.07; }
            if self.f.tension > rod.max_t * 0.8 && self.rnd() < dt * 6.0 { self.audio.play(Sfx::Strain); }
        } else {
            self.f.tension -= 0.75 * dt;
            self.f.progress -= if self.f.run { 0.07 + 0.09 * strength } else { 0.02 } * dt;
        }
        self.f.tension = self.f.tension.max(0.0);
        if self.f.tension >= rod.max_t { self.audio.play(Sfx::Snap); self.toast("SNAP! The line broke."); self.shake = 0.3; self.reset_fishing(); return false; }
        if self.f.progress <= 0.0 { self.audio.play(Sfx::Lose); self.toast("The fish slipped away..."); self.reset_fishing(); return false; }
        if self.f.progress >= 1.0 { self.land_fish(); return false; }
        true
    }

    fn land_fish(&mut self) {
        let k = self.f.fish;
        let (smin, smax, value) = { let s = &self.species[k]; (s.size.0, s.size.1, s.value) };
        let r = self.rnd().powf(1.6);
        let size = ((smin + (smax - smin) * r) * 10.0).round() / 10.0;
        let value = ((value as f32 * (0.75 + 0.6 * r)).round() as u32).max(1);
        let id = self.species[k].id().to_string();
        let new = self.save.entry(&id).is_none();
        match self.save.journal.iter_mut().find(|e| e.id == id) {
            Some(e) => { e.n += 1; e.best = e.best.max(size); }
            None => self.save.journal.push(Entry { id: id.clone(), n: 1, best: size }),
        }
        self.save.bag.push(BagFish { id, size, value });
        self.save.total += 1;
        self.persist();
        self.audio.play(Sfx::Catch); self.audio.play(Sfx::Splash);
        let b = self.f.bob; self.splash(b, 14);
        self.card = Some(CatchCard { fish: k, size, value, new });
        self.held_fish = Some(k);
        self.f.state = FState::Reward; self.f.t = 0.0;
        self.actors[0].pose = Pose::Cheer;
    }

    fn reset_fishing(&mut self) { self.f.state = FState::Idle; self.actors[0].pose = Pose::None; }

    // ---------- particles ----------
    fn init_particles(&mut self) {
        for _ in 0..70 { let (x, z, y, s) = (10.0 + self.rnd() * 50.0, 12.0 + self.rnd() * 24.0, 0.4 + self.rnd() * 1.5, self.rnd() * 10.0); self.sparks.push(Particle { p: v3(x, y, z), v: Vec3::ZERO, life: 0., seed: s }); }
        for _ in 0..60 { let s = self.rnd() * 10.0; self.leaves.push(Particle { p: v3(0., -10., 0.), v: Vec3::ZERO, life: 0., seed: s }); }
        let nh = self.world.houses.len();
        for k in 0..60 { let l = self.rnd(); self.smoke.push(Particle { p: Vec3::ZERO, v: Vec3::ZERO, life: l, seed: (k % nh) as f32 }); }
        for _ in 0..40 { let l = self.rnd(); self.drops.push(Particle { p: Vec3::ZERO, v: Vec3::ZERO, life: l, seed: 0. }); }
        for _ in 0..40 { let l = self.rnd(); self.mist.push(Particle { p: Vec3::ZERO, v: Vec3::ZERO, life: l, seed: 0. }); }
    }
    pub fn splash(&mut self, pos: Vec3, n: usize) {
        for _ in 0..n {
            let a = self.rnd() * std::f32::consts::TAU; let s = 0.5 + self.rnd() * 1.2;
            let life = 0.5 + self.rnd() * 0.4; let vy = 2.0 + self.rnd() * 2.0;
            self.drops_splash(Particle { p: v3(pos.x, -0.15, pos.z), v: v3(a.cos() * s, vy, a.sin() * s), life, seed: 1.0 });
        }
    }
    fn drops_splash(&mut self, p: Particle) { self.mist.push(p); }
    pub fn ripple(&mut self, p: Vec3, size: f32) { self.ripples.push(Ripple { p, t: 0.0, size }); }

    fn update_particles(&mut self, dt: f32) {
        let t = self.time; let pp = self.actors[0].pos;
        for l in self.leaves.iter_mut() {
            if l.p.y < -0.1 {
                let (a, b, c, d) = (self.rand.f(), self.rand.f(), self.rand.f(), self.rand.f());
                l.p = v3(pp.x + (a - 0.5) * 30.0, 4.0 + b * 4.0, pp.z + (c - 0.7) * 20.0); l.v.x = 0.3 + d * 0.5;
            }
            l.p.y -= dt * 0.55; l.p.x += (t * 2.0 + l.seed).sin() * dt * 0.6 + l.v.x * dt;
        }
        for s in self.smoke.iter_mut() { s.life += dt * 0.25; if s.life > 1.0 { s.life -= 1.0; } }
        for d in self.drops.iter_mut() {
            d.life += dt * 1.4;
            if d.life > 1.0 { d.life = 0.0; let a = self.rand.f() * std::f32::consts::TAU; d.v = v3(a.cos() * 0.45, 0., a.sin() * 0.45); }
        }
        let fb = self.world.fall_base;
        self.mist.retain_mut(|m| {
            if m.seed > 0.5 { m.life -= dt; m.v.y -= 9.0 * dt; m.p += m.v * dt; return m.life > 0.0 && m.p.y > -0.25; }
            m.life += dt * 0.6;
            if m.life > 1.0 { m.life = 0.0; m.p = v3(fb.x + (self.rand.f() - 0.5) * 2.0, 0., fb.z + self.rand.f() * 0.6); }
            true
        });
        self.ripples.retain_mut(|r| { r.t += dt * 0.9; r.t < 1.0 });
        self.boat_t = t;
    }

    // ---------- rendering ----------
    fn render(&mut self, dt: f32) {
        let (sw, sh) = (screen_width().max(1.0), screen_height().max(1.0));
        let h = self.save.hour;
        let env = env_at(h);
        self.env_night = env.night;
        let fov: f32 = if sw < sh { 44.0 } else { 30.0 };
        let mut eye = self.cam_target + v3(0., 15.5, 17.5);
        let mut look = self.cam_target + v3(0., 0.6, 0.);
        if self.shake > 0.0 { eye.x += (self.rnd() - 0.5) * self.shake; eye.y += (self.rnd() - 0.5) * self.shake; }
        if self.title_fade > 0.0 { let a = self.time * 0.08; eye.x += a.sin() * 3.0 * self.title_fade; look = self.cam_target + v3(0., 0.6, 0.); }
        let proj = Mat4::perspective_rh_gl(fov.to_radians(), sw / sh, 0.5, 200.0);
        let view = Mat4::look_at_rh(eye, look, Vec3::Y);
        self.vp = proj * view;

        let ang = (h - 6.0) / 12.0 * std::f32::consts::PI;
        let is_day = (5.5..19.5).contains(&h);
        let (sx, sy) = if is_day { (-ang.cos() * 14.0, 6.0 + ang.sin().max(0.0) * 18.0) } else { (8.0, 22.0) };
        let sun_dir = v3(sx, sy, 10.0).normalize();
        let tgt = self.cam_target;
        let light_vp = Mat4::orthographic_rh_gl(-20., 20., -20., 20., 1., 80.) * Mat4::look_at_rh(tgt + sun_dir * 40.0, tgt, Vec3::Y);

        let mut lamps = [[0f32; 4]; MAX_LAMPS];
        for (k, l) in self.world.lanterns.iter().take(MAX_LAMPS).enumerate() { lamps[k] = [l.x, l.y, l.z, env.night * (16.0 + (self.time * 7.0 + k as f32 * 2.1).sin() * 1.2)]; }
        let water_light = env.sun.lerp(Vec3::ONE, 0.45) * (0.55 + (env.sun_i / 3.0).min(1.0) * 0.5);
        let water_light = water_light.lerp(lin(srgb(hex("#4a5c9a"))), env.night * 0.55);
        let frame = Frame {
            lit: LitU { vp: self.vp.to_cols_array(), light_vp: light_vp.to_cols_array(), time: self.time, sun_dir: arr(sun_dir), sun_col: arr(env.sun * env.sun_i),
                sky: arr(env.hemi_s * env.hemi_i), ground: arr(env.hemi_g * env.hemi_i), fog_col: arr(env.sky), cam: arr(eye), lamp_col: arr(lin(srgb(hex("#ffb35c")))),
                misc: [30.0, 70.0, env.night, 1.05], lamps },
            water_light: arr(water_light), fog_srgb: arr(lin(env.sky)), sky_srgb: arr(env.sky), bloom: 0.35 + env.night * 0.45, warm: 1.0 - env.night,
        };
        self.build_dynamic(dt, eye, look);
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        self.gfx.render(gl.quad_context, &frame, self.time, &self.dyn_geo, &self.alpha, &self.add, sw as u32, sh as u32);
    }

    fn sprite(g: &mut Geo, pos: Vec3, w: f32, h: f32, uv: Uv) {
        let n = v3(0., 0.45, 0.9).normalize();
        g.quad_n([v3(pos.x - w / 2., pos.y, pos.z), v3(pos.x + w / 2., pos.y, pos.z), v3(pos.x + w / 2., pos.y + h, pos.z), v3(pos.x - w / 2., pos.y + h, pos.z)], uv, geo::WHITE, [0.; 4], n);
    }

    fn beam(&self, g: &mut Geo, a: Vec3, b: Vec3, thick: f32, c: [u8; 4]) {
        let d = b - a; let len = d.length();
        if len < 1e-4 { return; }
        let m = Mat4::from_scale_rotation_translation(v3(thick, len, thick), Quat::from_rotation_arc(Vec3::Y, d / len), (a + b) * 0.5);
        g.add(&cube1(Vec3::ONE, self.white, c), m, geo::WHITE);
    }

    fn build_dynamic(&mut self, dt: f32, eye: Vec3, look: Vec3) {
        let mut g = std::mem::take(&mut self.dyn_geo); g.clear();
        let mut al = std::mem::take(&mut self.alpha); al.clear();
        let mut ad = std::mem::take(&mut self.add); ad.clear();
        let fwd = (look - eye).normalize(); let right = fwd.cross(Vec3::Y).normalize(); let up = right.cross(fwd);
        let bb = |pg: &mut PGeo, p: Vec3, s: f32, c: [u8; 4], k: f32| {
            let (r, u) = (right * s, up * s);
            pg.quad([(p - r - u).into(), (p + r - u).into(), (p + r + u).into(), (p - r + u).into()], c, k);
        };
        let flat = |pg: &mut PGeo, p: Vec3, sx: f32, sz: f32, c: [u8; 4], k: f32| {
            pg.quad([[p.x - sx, p.y, p.z + sz], [p.x + sx, p.y, p.z + sz], [p.x + sx, p.y, p.z - sz], [p.x - sx, p.y, p.z - sz]], c, k);
        };

        for k in 0..4 {
            let uv = self.actors[k].frame_uv(dt);
            let a = &self.actors[k];
            Self::sprite(&mut g, a.pos, 1.0, 1.5, uv);
            flat(&mut al, a.pos + v3(0., 0.02, 0.05), 0.42, 0.25, [0, 0, 0, 115], 0.);
        }
        let cf = ((self.time * 1.5) as usize % 2) as f32;
        let cat = self.cat.sheet.sub(cf / 2.0, 0., (cf + 1.0) / 2.0, 1.);
        Self::sprite(&mut g, self.cat.pos, 0.7, 0.7, cat);
        flat(&mut al, self.cat.pos + v3(0., 0.02, 0.05), 0.3, 0.17, [0, 0, 0, 100], 0.);
        if let Some(k) = self.held_fish {
            let p = self.actors[0].pos;
            let y = 1.95 + (self.time * 4.0).sin() * 0.05;
            Self::sprite(&mut g, v3(p.x, y - 0.35, p.z + 0.1), 1.4, 0.7, self.fish_uv[k]);
        }
        // boat bobbing at its mooring
        let bp = self.world.boat_pos + v3(0., (self.boat_t * 1.3).sin() * 0.04, 0.);
        let bm = Mat4::from_translation(bp) * Mat4::from_rotation_y(0.12) * Mat4::from_rotation_z((self.boat_t * 1.1).sin() * 0.04);
        g.add(&self.world.boat, bm, geo::WHITE);

        // rod, line and bobber
        if self.f.state != FState::Idle {
            let (hand, tip, bob) = (self.f.hand, self.f.tip, self.f.bob);
            self.beam(&mut g, hand, tip, 0.035, hex("#7a5230"));
            if !matches!(self.f.state, FState::Charge | FState::Reward) {
                g.add(&cube1(v3(0.16, 0.1, 0.16), self.white, hex("#e8402e")), Mat4::from_translation(bob + v3(0., 0.05, 0.)), geo::WHITE);
                g.add(&cube1(v3(0.16, 0.1, 0.16), self.white, hex("#f4f0e8")), Mat4::from_translation(bob - v3(0., 0.05, 0.)), geo::WHITE);
                let sag = match self.f.state { FState::Reel => 0.05, FState::Flight => 0.2, _ => 0.6 };
                let mid = tip.lerp(bob, 0.5) - v3(0., sag, 0.);
                let pt = |t: f32| { let u = 1.0 - t; tip * (u * u) + mid * (2.0 * u * t) + bob * (t * t) };
                for s in 0..23 {
                    let (a, b) = (pt(s as f32 / 23.0), pt((s + 1) as f32 / 23.0));
                    let side = (b - a).cross(eye - a).normalize_or_zero() * 0.012;
                    al.quad([(a - side).into(), (b - side).into(), (b + side).into(), (a + side).into()], [244, 244, 240, 220], 1.);
                }
            }
        }
        for r in &self.ripples { let s = (0.1 + r.t * 0.6) * r.size; flat(&mut al, v3(r.p.x, -0.19, r.p.z), s, s, [255, 255, 255, ((1.0 - r.t) * 150.0) as u8], 2.); }

        // ambient particles
        let night = self.env_night; let t = self.time;
        let ffa = (night * (0.6 + 0.4 * (t * 3.0).sin()) * 255.0) as u8;
        if ffa > 2 { for f in &self.sparks { bb(&mut ad, f.p + v3((t * 0.5 + f.seed).sin() * 0.8, (t * 1.3 + f.seed * 2.0).sin() * 0.3, (t * 0.4 + f.seed).cos() * 0.8), 0.06, [216, 255, 122, ffa], 0.); } }
        for l in &self.leaves { bb(&mut al, l.p, 0.06, [232, 144, 46, 230], 1.); }
        let pp = self.actors[0].pos;
        let ma = ((1.0 - night) * 150.0) as u8;
        if ma > 2 { for k in 0..40 { let kf = k as f32; bb(&mut ad, v3(pp.x + (kf * 12.9 + t * 0.1).sin() * 12.0, 0.5 + (kf * 0.37 + t * 0.05).rem_euclid(3.0), pp.z + (kf * 7.3 + t * 0.08).cos() * 8.0), 0.03, [255, 242, 192, ma], 0.); } }
        for s in &self.smoke {
            let c = self.world.houses[s.seed as usize].chimney;
            bb(&mut al, v3(c.x + (s.life * 6.0 + s.seed).sin() * 0.2 + s.life * 0.6, c.y + s.life * 2.5, c.z - s.life * 0.3), 0.14 + s.life * 0.08, [216, 212, 204, ((1.0 - s.life) * 120.0) as u8], 0.);
        }
        let f = self.world.fountain;
        for d in &self.drops { let tt = d.life * 0.6; bb(&mut ad, v3(f.x + d.v.x * tt * 1.6, 1.62 + 1.6 * tt - 4.5 * tt * tt, f.z + d.v.z * tt * 1.6), 0.04, [207, 239, 255, 200], 0.); }
        for m in &self.mist {
            if m.seed > 0.5 { bb(&mut al, m.p, 0.05, [232, 248, 255, 240], 1.); }
            else { bb(&mut ad, v3(m.p.x, -0.1 + m.life * 0.9, m.p.z + m.life * 0.3), 0.07, [255, 255, 255, ((1.0 - m.life) * 120.0) as u8], 0.); }
        }
        al.sort_back_to_front(eye.into());
        self.dyn_geo = g; self.alpha = al; self.add = ad;
    }

    /// Projects a world point to screen pixels.
    pub fn to_screen(&self, p: Vec3) -> Vec2 {
        let c = self.vp * p.extend(1.0);
        let n = c.truncate() / c.w;
        vec2((n.x * 0.5 + 0.5) * screen_width(), (0.5 - n.y * 0.5) * screen_height())
    }
}
