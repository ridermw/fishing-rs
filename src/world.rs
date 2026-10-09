//! World generation for Driftmoor Cove, baked into a single static mesh.
use crate::geo::*;
use crate::pixel::{self as px, hex, Rng};
use macroquad::math::{Mat4, Quat, Vec3};

pub const W: i32 = 72;
pub const H: i32 = 58;
pub const PLAY_X0: i32 = 8;
pub const PLAY_X1: i32 = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum T { Grass, Sand, Path, Cobble, Sea, Fresh, Dock, Cliff }

pub struct House { pub i: i32, pub j: i32, pub w: i32, pub d: i32, pub roof: &'static str, pub chimney: Vec3 }

pub struct World {
    pub ty: Vec<T>,
    pub height: Vec<u8>,
    pub blocked: Vec<bool>,
    pub dist: Vec<f32>,
    pub shore: Vec<i32>,
    pub cliff: Vec<i32>,
    pub river: [i32; 2],
    pub houses: Vec<House>,
    pub lanterns: Vec<Vec3>,
    pub spawn: Vec3,
    pub fountain: Vec3,
    pub stall: Vec3,
    pub dock_start: i32,
    pub fall_base: Vec3,
    /// Static lit geometry, waterfall geometry and the (animated) boat in local space.
    pub mesh: Geo,
    pub falls: Geo,
    pub boat: Geo,
    pub boat_pos: Vec3,
}

fn idx(i: i32, j: i32) -> usize { (j * W + i) as usize }
pub fn inside(i: i32, j: i32) -> bool { i >= 0 && j >= 0 && i < W && j < H }

fn trs(t: Vec3, ry: f32, s: Vec3) -> Mat4 { Mat4::from_scale_rotation_translation(s, Quat::from_rotation_y(ry), t) }
fn at(x: f32, y: f32, z: f32) -> Mat4 { Mat4::from_translation(v3(x, y, z)) }
fn tint(c: &str, f: f32) -> [u8; 4] { px::shade(hex(c), f) }
fn gray(f: f32) -> [u8; 4] { let v = (255.0 * f).clamp(0.0, 255.0) as u8; [v, v, v, 255] }

pub struct Tex {
    pub grass: Vec<Uv>, pub grass_side: Uv, pub dirt: Uv, pub dirt_side: Uv, pub sand: Uv, pub sand_side: Uv,
    pub cobble: Uv, pub stone: Uv, pub plank: Uv, pub plank_v: Uv, pub bark: Uv, pub leaf: Uv, pub crate_: Uv, pub barrel: Uv,
    pub white: Uv, pub lamp: Uv, pub glow: Uv,
}

impl World {
    pub fn typ(&self, i: i32, j: i32) -> T { if inside(i, j) { self.ty[idx(i, j)] } else if j > 40 { T::Sea } else { T::Grass } }
    pub fn is_water(&self, i: i32, j: i32) -> bool { matches!(self.typ(i, j), T::Sea | T::Fresh) }
    pub fn depth(&self, i: i32, j: i32) -> f32 { if inside(i, j) { self.dist[idx(i, j)] } else { 10.0 } }
    pub fn block(&mut self, i: i32, j: i32) { if inside(i, j) { self.blocked[idx(i, j)] = true; } }
    pub fn walkable(&self, x: f32, z: f32) -> bool {
        let (i, j) = (x.floor() as i32, z.floor() as i32);
        if !inside(i, j) || i < PLAY_X0 || i >= PLAY_X1 { return false; }
        !matches!(self.ty[idx(i, j)], T::Sea | T::Fresh | T::Cliff) && !self.blocked[idx(i, j)]
    }

    pub fn new(atlas: &mut Atlas) -> World {
        let n = (W * H) as usize;
        let mut w = World {
            ty: vec![T::Grass; n], height: vec![0; n], blocked: vec![false; n], dist: vec![0.0; n],
            shore: vec![], cliff: vec![], river: [19, 20], houses: vec![], lanterns: vec![],
            spawn: v3(37.5, 0., 31.5), fountain: v3(36., 0., 24.), stall: v3(41.5, 0., 27.5), dock_start: 0,
            fall_base: Vec3::ZERO, mesh: Geo::new(), falls: Geo::new(), boat: Geo::new(), boat_pos: Vec3::ZERO,
        };
        w.generate();
        w.compute_distance();
        let tex = Tex {
            grass: (1..5).map(|s| atlas.add(&px::grass(s))).collect(), grass_side: atlas.add(&px::grass_side(2)),
            dirt: atlas.add(&px::dirt(3)), dirt_side: atlas.add(&px::dirt_side(4)), sand: atlas.add(&px::sand(5)),
            sand_side: atlas.add(&px::sand_side(6)), cobble: atlas.add(&px::cobble(7)), stone: atlas.add(&px::stone(8)),
            plank: atlas.add(&px::plank(9, true)), plank_v: atlas.add(&px::plank(21, false)), bark: atlas.add(&px::bark(10)),
            leaf: atlas.add(&px::leaf(11)), crate_: atlas.add(&px::crate_tex(12)), barrel: atlas.add(&px::barrel(13)),
            white: atlas.add(&px::swatch(255)), lamp: atlas.add(&px::swatch(px::A_LAMP)), glow: atlas.add(&px::swatch(px::A_GLOW)),
        };
        w.build_terrain(&tex);
        w.build_dock(&tex);
        w.build_houses(&tex, atlas);
        w.build_trees(&tex);
        w.build_props(&tex, atlas);
        w.build_plants(atlas);
        w.build_falls();
        w
    }

    fn set(&mut self, i: i32, j: i32, t: T) { if inside(i, j) { self.ty[idx(i, j)] = t; } }
    fn rect(&mut self, i0: i32, j0: i32, ww: i32, hh: i32, t: T) { for j in j0..j0 + hh { for i in i0..i0 + ww { self.set(i, j, t); } } }

    fn generate(&mut self) {
        for i in 0..W {
            let f = i as f32;
            self.shore.push(40 + (1.5 * (f * 0.21).sin() + 0.8 * (f * 0.57 + 1.3).sin()).round() as i32);
            self.cliff.push(9 + (1.2 * (f * 0.27).sin() + 0.8 * (f * 0.11 + 2.0).sin()).round() as i32);
        }
        for j in 0..H { for i in 0..W {
            let mut t = T::Grass;
            let s = self.shore[i as usize];
            if j >= s { t = T::Sea } else if j >= s - 3 { t = T::Sand }
            let c = self.cliff[i as usize];
            if j < c { t = T::Cliff; self.height[idx(i, j)] = if j < c - 3 { 3 } else { 2 }; }
            self.set(i, j, t);
        } }
        // pond and the stream that feeds it from the waterfall
        for j in 0..H { for i in 0..W {
            let dx = (i as f32 + 0.5 - 20.0) / 6.2; let dz = (j as f32 + 0.5 - 19.0) / 4.2;
            if dx * dx + dz * dz < 1.0 + 0.12 * (i as f32 * 1.3 + j as f32 * 0.7).sin() { self.set(i, j, T::Fresh); }
        } }
        for j in 0..19 { for i in self.river {
            if self.ty[idx(i, j)] == T::Cliff { self.height[idx(i, j)] = 2; } else { self.set(i, j, T::Fresh); }
        } }
        // village plaza, roads and the pier
        self.rect(31, 21, 16, 8, T::Cobble);
        let s = self.shore[37].min(self.shore[38]);
        self.dock_start = s;
        for j in 29..s { self.set(37, j, T::Cobble); self.set(38, j, T::Cobble); }
        for j in s..s + 9 { self.set(37, j, T::Dock); self.set(38, j, T::Dock); }
        self.rect(34, s + 7, 8, 2, T::Dock);
        self.rect(25, 24, 6, 2, T::Path);
        self.rect(47, 24, 17, 2, T::Path);
        self.rect(14, 24, 2, 1, T::Path);
        let hs = [(29, 15, 5, 4, "red"), (36, 14, 5, 4, "blue"), (43, 15, 4, 4, "thatch"), (50, 19, 5, 4, "red"), (55, 28, 4, 3, "blue"), (24, 29, 4, 3, "thatch")];
        for (i, j, ww, d, roof) in hs {
            let door = i + ww / 2;
            for b in j..j + d { for a in i..i + ww { self.block(a, b); } }
            for b in j + d..H {
                if matches!(self.typ(door, b), T::Cobble | T::Path | T::Sand | T::Sea) { break; }
                self.set(door, b, T::Cobble);
            }
            self.houses.push(House { i, j, w: ww, d, roof, chimney: Vec3::ZERO });
        }
        for (i, j) in [(35, 23), (36, 23), (35, 24), (36, 24)] { self.block(i, j); }
        for i in 40..=42 { self.block(i, 27); }
    }

    fn compute_distance(&mut self) {
        let mut q = std::collections::VecDeque::new();
        for j in 0..H { for i in 0..W {
            let water = matches!(self.ty[idx(i, j)], T::Sea | T::Fresh | T::Dock);
            self.dist[idx(i, j)] = if water { 1e9 } else { 0.0 };
            if !water { q.push_back((i, j)); }
        } }
        while let Some((i, j)) = q.pop_front() {
            let d = self.dist[idx(i, j)];
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i + di, j + dj);
                if inside(a, b) && self.dist[idx(a, b)] > d + 1.0 { self.dist[idx(a, b)] = d + 1.0; q.push_back((a, b)); }
            }
        }
    }

    /// Top surface height of a terrain column, or None where only the deep sea bed shows.
    fn top(&self, i: i32, j: i32) -> Option<f32> {
        match self.typ(i, j) {
            T::Cliff => Some(if inside(i, j) { self.height[idx(i, j)] as f32 } else { 2.0 }),
            T::Sea | T::Fresh | T::Dock => { let d = self.depth(i, j); if d <= 2.0 && self.typ(i, j) != T::Dock { Some(-0.6 - d * 0.25) } else { None } }
            _ => Some(0.0),
        }
    }

    fn build_terrain(&mut self, tx: &Tex) {
        let mut g = Geo::new();
        let mut r = Rng::new(2);
        for j in 0..H { for i in 0..W {
            let Some(t) = self.top(i, j) else { continue };
            let ty = self.typ(i, j);
            let f = 0.96 + r.f() * 0.08;
            let c = gray(f);
            let (top, edge, deep) = match ty {
                T::Grass | T::Cliff => (tx.grass[((i * 7 + j * 13) % 4) as usize], tx.grass_side, if ty == T::Cliff { tx.stone } else { tx.dirt_side }),
                T::Sand => (tx.sand, tx.sand_side, tx.sand_side),
                T::Path => (tx.dirt, tx.dirt_side, tx.dirt_side),
                T::Cobble => (tx.cobble, tx.stone, tx.stone),
                T::Fresh => (tx.dirt, tx.dirt_side, tx.dirt_side),
                _ => (tx.sand, tx.sand_side, tx.sand_side),
            };
            let (x0, x1, z0, z1) = (i as f32, i as f32 + 1., j as f32, j as f32 + 1.);
            g.quad([v3(x0, t, z1), v3(x1, t, z1), v3(x1, t, z0), v3(x0, t, z0)], top, c, [0.; 4]);
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let nt = if inside(i + di, j + dj) { self.top(i + di, j + dj).unwrap_or(-2.2) } else { t };
                if nt >= t { continue; }
                let mut y1 = t; let mut seg = 0;
                while y1 > nt + 0.001 {
                    let y0 = (y1 - 1.0).max(nt);
                    let uv = (if seg == 0 { edge } else { deep }).sub(0., 0., 1., y1 - y0);
                    let p = match (di, dj) {
                        (1, 0) => [v3(x1, y0, z1), v3(x1, y0, z0), v3(x1, y1, z0), v3(x1, y1, z1)],
                        (-1, 0) => [v3(x0, y0, z0), v3(x0, y0, z1), v3(x0, y1, z1), v3(x0, y1, z0)],
                        (0, 1) => [v3(x0, y0, z1), v3(x1, y0, z1), v3(x1, y1, z1), v3(x0, y1, z1)],
                        _ => [v3(x1, y0, z0), v3(x0, y0, z0), v3(x0, y1, z0), v3(x1, y1, z0)],
                    };
                    g.quad(p, uv, gray(f * if seg == 0 { 1.0 } else { 0.92 }), [0.; 4]);
                    y1 = y0; seg += 1;
                }
            }
        } }
        // land beyond the map edges so the diorama never shows a void
        let mut far = |x0: i32, z0: i32, ww: i32, dd: i32, y: f32, g: &mut Geo| {
            for b in z0..z0 + dd { for a in x0..x0 + ww {
                let (x, z) = (a as f32, b as f32);
                g.quad([v3(x, y, z + 1.), v3(x + 1., y, z + 1.), v3(x + 1., y, z), v3(x, y, z)], tx.grass[((a * 7 + b * 13).rem_euclid(4)) as usize], gray(0.97 + r.f() * 0.06), [0.; 4]);
            } }
        };
        far(-40, -40, 40, 78, 0.0, &mut g);
        far(W, -40, 40, 78, 0.0, &mut g);
        far(0, -40, W, 40, 3.0, &mut g);
        // deep sea bed
        g.quad([v3(-164., -2.2, 229.), v3(236., -2.2, 229.), v3(236., -2.2, -171.), v3(-164., -2.2, -171.)], tx.white, hex("#2c4f5e"), [0.; 4]);
        self.mesh.add(&g, Mat4::IDENTITY, geo_white());
    }

    fn build_dock(&mut self, tx: &Tex) {
        let mut g = Geo::new();
        let mut r = Rng::new(7);
        let plank = cube1(v3(1., 0.14, 1.), tx.plank, WHITE);
        for j in 0..H { for i in 0..W {
            if self.typ(i, j) == T::Dock { g.add(&plank, at(i as f32 + 0.5, -0.07, j as f32 + 0.5), gray(0.94 + r.f() * 0.12)); }
        } }
        let post = cylinder(0.09, 0.1, 1.0, 6, tx.bark, tx.bark, WHITE);
        let s = self.dock_start;
        let mut posts = vec![];
        for j in s..s + 9 { for i in [37, 39] { if j % 2 == 0 { posts.push(v3(i as f32, -0.6, j as f32 + 0.5)); } } }
        for i in (34..=42).step_by(2) { for j in [s + 7, s + 9] { posts.push(v3(i as f32, -0.6, j as f32)); } }
        for (k, p) in posts.iter().enumerate() {
            g.add(&post, trs(*p, 0., v3(1., 1.5, 1.)), WHITE);
            if k % 2 == 0 { g.add(&post, trs(v3(p.x, 0.15, p.z), 0., v3(1., 0.4, 1.)), WHITE); }
        }
        self.mesh.add(&g, Mat4::IDENTITY, WHITE);

        // moored rowing boat, tapered at the bow
        let mut hull = Geo::new();
        for sgm in 0..4 {
            let z = -1.3 + 0.325 + sgm as f32 * 0.65;
            hull.add(&cube1(v3(1., 0.42, 0.65), tx.plank_v, hex("#c8d8e8")), at(0., 0., z), WHITE);
        }
        for v in hull.v.iter_mut() {
            let (x, y, z) = (v.p[0], v.p[1], v.p[2]);
            let taper = if z > 0.6 { 1.0 - (z - 0.6) / 0.9 } else { 1.0 };
            v.p[0] = x * taper.max(0.05) * if y < 0.0 { 0.75 } else { 1.0 };
            if z > 1.0 { v.p[1] = y + 0.12; }
        }
        self.boat.add(&hull, Mat4::IDENTITY, WHITE);
        self.boat.add(&cube1(v3(0.8, 0.06, 0.25), tx.plank, WHITE), at(0., 0.16, -0.2), WHITE);
        self.boat.add(&cube1(v3(1.02, 0.08, 1.5), tx.white, hex("#c9473a")), at(0., 0.12, -0.45), WHITE);
        self.boat_pos = v3(35.6, -0.12, s as f32 + 3.5);
    }

    fn build_houses(&mut self, tx: &Tex, atlas: &mut Atlas) {
        let mut seed = 30;
        let mut g = Geo::new();
        for hi in 0..self.houses.len() {
            let (i, j, hw, hd, roof) = { let h = &self.houses[hi]; (h.i, h.j, h.w, h.d, h.roof) };
            let (wf, df) = (hw as f32, hd as f32);
            let mut local = Geo::new();
            let wall_h = 2.2;
            let front = atlas.add(&px::wall(hw as usize, "front", seed)); seed += 1;
            let back = atlas.add(&px::wall(hw as usize, "back", seed)); seed += 1;
            let side = atlas.add(&px::wall(hd as usize, "side", seed)); seed += 1;
            local.add(&cube(v3(wf - 0.1, wall_h, df - 0.1), [side, side, tx.white, tx.white, front, back], WHITE), at(0., wall_h / 2., 0.), WHITE);
            // gabled roof, ridge along x
            let roof_uv = atlas.add(&px::roof(roof, seed));
            let (ow, od) = (wf + 0.5, df + 0.7);
            let rh = df * 0.55;
            let na = ow.ceil() as i32; let nb = ((od / 2.0).hypot(rh)).ceil() as i32;
            for side_z in [1.0f32, -1.0] {
                let eave_z = side_z * od / 2.0;
                for a in 0..na { for b in 0..nb {
                    let (s0, s1) = (a as f32 / na as f32, (a + 1) as f32 / na as f32);
                    let (t0, t1) = (b as f32 / nb as f32, (b + 1) as f32 / nb as f32);
                    let p = |s: f32, t: f32| {
                        let x = if side_z > 0.0 { -ow / 2.0 + ow * s } else { ow / 2.0 - ow * s };
                        v3(x, wall_h + rh * t, eave_z * (1.0 - t))
                    };
                    local.quad([p(s0, t0), p(s1, t0), p(s1, t1), p(s0, t1)], roof_uv, WHITE, [0.; 4]);
                } }
            }
            let (gw, gd) = ((wf - 0.1) / 2.0, (df - 0.1) / 2.0);
            let apex = wall_h + rh * (df - 0.1) / od;
            let pv = tx.plank_v;
            for sx in [1.0f32, -1.0] {
                let x = sx * gw;
                let (a, b) = if sx > 0.0 { (v3(x, wall_h, gd), v3(x, wall_h, -gd)) } else { (v3(x, wall_h, -gd), v3(x, wall_h, gd)) };
                local.tri([a, b, v3(x, apex, 0.)], [pv.at(0., 1.), pv.at(1., 1.), pv.at(0.5, 0.)], WHITE);
            }
            local.add(&cube1(v3(ow + 0.1, 0.12, 0.16), tx.white, hex("#5a3a22")), at(0., wall_h + rh, 0.), WHITE);
            let chim = v3(wf / 2.0 - 1.0, wall_h + rh * 0.7, -0.4);
            local.add(&cube1(v3(0.45, 1.2, 0.45), tx.stone, WHITE), at(chim.x, chim.y, chim.z), WHITE);
            let origin = v3(i as f32 + wf / 2.0, 0.0, j as f32 + df / 2.0);
            self.houses[hi].chimney = origin + chim + v3(0., 0.7, 0.);
            g.add(&local, Mat4::from_translation(origin), WHITE);
        }
        self.mesh.add(&g, Mat4::IDENTITY, WHITE);
    }

    fn near(&self, i: i32, j: i32, f: impl Fn(T, i32, i32) -> bool) -> bool {
        for b in j - 1..=j + 1 { for a in i - 1..=i + 1 { if f(self.typ(a, b), a, b) { return true; } } }
        false
    }

    fn build_trees(&mut self, tx: &Tex) {
        let mut r = Rng::new(99);
        let mut g = Geo::new();
        let autumn = ["#5c9a3e", "#4a8a36", "#6aa843", "#5c9a3e", "#e08a2e", "#cf4b2e", "#e4b83c", "#4a8a36"];
        let trunk = cylinder(0.12, 0.17, 1.0, 6, tx.bark, tx.bark, WHITE);
        let leaf = ico(0.62, tx.leaf, WHITE);
        let pine = cone(0.75, 1.3, 7, tx.leaf, WHITE);
        let mut pines: Vec<(f32, f32, f32, f32)> = vec![];
        for j in 0..H { for i in 0..W {
            let t = self.ty[idx(i, j)];
            if t == T::Cliff {
                if j < self.cliff[i as usize] - 1 && !self.river.contains(&i) && r.f() < 0.22 {
                    let h = self.height[idx(i, j)] as f32;
                    pines.push((i as f32 + 0.5 + (r.f() - 0.5) * 0.4, h, j as f32 + 0.5, 0.8 + r.f() * 0.5));
                }
                continue;
            }
            if t != T::Grass || self.blocked[idx(i, j)] { continue; }
            let border = i < PLAY_X0 + 1 || i >= PLAY_X1 - 1;
            if !border && self.near(i, j, |tt, _, _| matches!(tt, T::Path | T::Cobble | T::Dock | T::Fresh | T::Sand)) { continue; }
            if !border && self.near(i, j, |_, a, b| inside(a, b) && self.blocked[idx(a, b)]) { continue; }
            if (i as f32 + 0.5 - self.spawn.x).abs() < 3.0 && (j as f32 + 0.5 - self.spawn.z).abs() < 3.0 { continue; }
            let n = (i as f32 * 0.35).sin() * (j as f32 * 0.41).cos() + ((i + j) as f32 * 0.17).sin();
            let ci = self.cliff[i as usize];
            let p = if border { 0.8 } else if j < ci + 3 { 0.3 } else if n > 0.7 { 0.45 } else { 0.05 };
            if r.f() > p { continue; }
            self.block(i, j);
            let x = i as f32 + 0.5 + (r.f() - 0.5) * 0.3; let z = j as f32 + 0.5 + (r.f() - 0.5) * 0.3;
            if j < ci + 3 && r.f() < 0.6 { pines.push((x, 0.0, z, 0.9 + r.f() * 0.5)); continue; }
            let th = 1.0 + r.f() * 0.7;
            let col = autumn[r.int(autumn.len())];
            g.add(&trunk, trs(v3(x, th / 2.0, z), 0., v3(1., th, 1.)), gray(0.92 + r.f() * 0.16));
            let clumps = 3 + r.int(2);
            for k in 0..clumps {
                let s = 0.75 + r.f() * 0.45;
                let pos = v3(x + (r.f() - 0.5) * 0.9, th + 0.35 + k as f32 * 0.38 + r.f() * 0.2, z + (r.f() - 0.5) * 0.7);
                let ry = r.f() * 6.0;
                g.add(&leaf, trs(pos, ry, Vec3::splat(s)), tint(col, 0.91 + r.f() * 0.18));
            }
        } }
        for (x, y, z, s) in pines {
            g.add(&pine, trs(v3(x, y + 0.3 * s, z), 0., v3(s * 0.2, s * 0.6, s * 0.2)), hex("#6b4a2f"));
            for k in 0..3 {
                let col = if k == 2 { "#3f7a52" } else { "#2f6644" };
                let sc = s * (1.0 - k as f32 * 0.22);
                g.add(&pine, trs(v3(x, y + (0.85 + k as f32 * 0.55) * s, z), k as f32, Vec3::splat(sc)), tint(col, 0.93 + r.f() * 0.14));
            }
        }
        // rocks on the beach and around the pond
        let rock = ico(1.0, tx.stone, WHITE);
        for _ in 0..70 {
            let i = PLAY_X0 + r.int((PLAY_X1 - PLAY_X0) as usize) as i32; let j = 12 + r.int(34) as i32;
            let t = self.typ(i, j);
            if (t == T::Sand || (t == T::Grass && self.near(i, j, |tt, _, _| tt == T::Fresh))) && !self.blocked[idx(i, j)] && (i as f32 - 37.5).abs() > 2.0 {
                let s = v3(0.35 + r.f() * 0.3, 0.25 + r.f() * 0.2, 0.35 + r.f() * 0.3);
                g.add(&rock, trs(v3(i as f32 + 0.5, 0.1, j as f32 + 0.5), r.f() * 6.0, s), gray(0.85 + r.f() * 0.3));
                self.block(i, j);
            }
        }
        self.mesh.add(&g, Mat4::IDENTITY, WHITE);
    }

    fn build_plants(&mut self, atlas: &mut Atlas) {
        let mut r = Rng::new(7);
        let kinds = ["grass", "grass", "grass", "red", "yellow", "white", "blue"];
        let uv: Vec<Uv> = ["grass", "red", "yellow", "white", "blue"].iter().enumerate().map(|(k, n)| atlas.add(&px::tuft(n, 50 + k as u32))).collect();
        let kind_uv = |k: &str| uv[match k { "grass" => 0, "red" => 1, "yellow" => 2, "white" => 3, _ => 4 }];
        let mut g = Geo::new();
        for j in 0..H { for i in PLAY_X0 - 2..PLAY_X1 + 2 {
            if self.typ(i, j) != T::Grass || !inside(i, j) || self.blocked[idx(i, j)] { continue; }
            let n = 1 + r.int(3);
            for _ in 0..n {
                if r.f() >= 0.35 { continue; }
                let k = kinds[r.int(kinds.len())];
                let (x, z, s, ry) = (i as f32 + r.f(), j as f32 + r.f(), 0.4 + r.f() * 0.35, r.f() * 3.0);
                let c = gray(0.9 + r.f() * 0.2);
                for rot in [0.0, std::f32::consts::FRAC_PI_2] {
                    let a = ry + rot;
                    let d = v3(a.cos(), 0., -a.sin()) * (s / 2.0);
                    let base = v3(x, 0.0, z);
                    let up = v3(0., s, 0.);
                    g.quad_n([base - d, base + d, base + d + up, base - d + up], kind_uv(k), c, [0., 0., 1., 1.], v3(0., 1., 0.));
                }
            }
        } }
        self.mesh.add(&g, Mat4::IDENTITY, WHITE);
    }

    fn build_props(&mut self, tx: &Tex, atlas: &mut Atlas) {
        let mut g = Geo::new();
        let s = self.dock_start as f32;
        let crate_box = cube1(v3(0.6, 0.6, 0.6), tx.crate_, WHITE);
        let crates: [(f32, f32, f32, f32); 6] = [(36.5, 0.3, s - 1.5, 1.0), (39.6, 0.3, s + 7.5, 1.0), (39.6, 0.9, s + 7.6, 0.6), (43.5, 0.3, 27.3, 1.0), (33.4, 0.3, 21.6, 1.0), (56.5, 0.3, 31.6, 1.0)];
        for (x, y, z, sc) in crates { self.block(x.floor() as i32, z.floor() as i32); g.add(&crate_box, trs(v3(x, y, z), 0.2, Vec3::splat(sc)), WHITE); }
        let barrel = cylinder(0.3, 0.3, 0.7, 10, tx.barrel, tx.plank, WHITE);
        for (x, y, z) in [(39.5f32, 0.35f32, s - 1.4), (34.4, 0.35, 21.6), (28.6, 0.35, 19.4), (49.6, 0.35, 23.4)] {
            self.block(x.floor() as i32, z.floor() as i32); g.add(&barrel, at(x, y, z), WHITE);
        }
        // fountain
        let f = self.fountain;
        g.add(&cylinder(1.0, 1.1, 0.5, 8, tx.stone, tx.stone, WHITE), at(f.x, 0.25, f.z), WHITE);
        g.add(&cylinder(0.86, 0.86, 0.05, 8, tx.glow, tx.glow, hex("#5fc4d6")), at(f.x, 0.48, f.z), WHITE);
        g.add(&cylinder(0.14, 0.2, 1.1, 8, tx.stone, tx.stone, WHITE), at(f.x, 0.95, f.z), WHITE);
        g.add(&cylinder(0.45, 0.25, 0.18, 8, tx.stone, tx.stone, WHITE), at(f.x, 1.5, f.z), WHITE);
        // market stall
        let st = self.stall;
        g.add(&cube1(v3(3., 0.9, 0.9), tx.crate_, WHITE), at(st.x, 0.45, st.z), WHITE);
        g.add(&cube1(v3(2.6, 0.08, 0.6), tx.white, hex("#cfeaf2")), at(st.x, 0.93, st.z), WHITE);
        for (k, c) in ["#9fb8c8", "#d98a8a", "#c8b060", "#7a9ab8", "#b8c8d0"].iter().enumerate() {
            let odd = k % 2 == 1;
            g.add(&cube1(v3(0.34, 0.07, 0.14), tx.white, hex(c)), trs(v3(st.x - 1.0 + k as f32 * 0.5, 1.0, st.z + if odd { 0.1 } else { -0.1 }), if odd { 0.3 } else { -0.2 }, Vec3::ONE), WHITE);
        }
        for dx in [-1.45, 1.45] { for dz in [-0.4f32, 0.4] {
            let h = if dz < 0.0 { 2.4 } else { 2.0 };
            g.add(&cube1(v3(0.1, h, 0.1), tx.plank, WHITE), at(st.x + dx, h / 2.0, st.z + dz), WHITE);
        } }
        let awn = atlas.add(&px::stripes("#2f8f8a", "#f2ead8"));
        let tilt = Mat4::from_translation(v3(st.x, 2.2, st.z + 0.15)) * Mat4::from_rotation_x(-1.05);
        let mut aw = Geo::new();
        for k in 0..3 {
            let x0 = -1.65 + k as f32 * 1.1;
            aw.quad([v3(x0, -0.65, 0.), v3(x0 + 1.1, -0.65, 0.), v3(x0 + 1.1, 0.65, 0.), v3(x0, 0.65, 0.)], awn, WHITE, [0.; 4]);
        }
        g.add(&aw, tilt, WHITE);
        let fresh = atlas.add(&px::sign("FRESH FISH", 64, 16));
        g.quad([v3(st.x - 1., 2.5, st.z - 0.38), v3(st.x + 1., 2.5, st.z - 0.38), v3(st.x + 1., 3.0, st.z - 0.38), v3(st.x - 1., 3.0, st.z - 0.38)], fresh, WHITE, [0.; 4]);
        // welcome sign
        let welcome = atlas.add(&px::sign("DRIFTMOOR COVE", 64, 16));
        g.add(&cube(v3(2.2, 0.55, 0.08), [tx.plank, tx.plank, tx.plank, tx.plank, welcome, tx.plank], WHITE), at(35.4, 1.25, 29.62), WHITE);
        for dx in [-0.95, 0.95] { g.add(&cube1(v3(0.1, 1.3, 0.1), tx.plank, WHITE), at(35.4 + dx, 0.65, 29.5), WHITE); }
        for i in 34..=36 { self.block(i, 29); }
        // bunting across the plaza
        let pal = ["#d9475a", "#f2c94c", "#3f9ad8", "#5cb85c", "#f08a3a"];
        for k in 0..=22 {
            let t = k as f32 / 22.0;
            let (x, y, z) = (31.6 + t * 14.8, 3.1 - (t * std::f32::consts::PI).sin() * 0.6, 21.4);
            let w = tx.white;
            g.tri([v3(x - 0.16, y + 0.18, z), v3(x + 0.16, y + 0.18, z), v3(x, y - 0.18, z)], [w.at(0., 0.), w.at(1., 0.), w.at(0.5, 1.)], hex(pal[k % pal.len()]));
        }
        // lanterns (their light comes from the lit shader's point-light array)
        let post = cube1(v3(0.1, 1.7, 0.1), tx.white, hex("#2b2622"));
        let lamp = cube1(v3(0.26, 0.3, 0.26), tx.lamp, hex("#ffd38a"));
        let cap = cone(0.24, 0.18, 4, tx.white, hex("#2b2622"));
        let spots: [(f32, f32); 9] = [(31.5, 21.5), (46.5, 21.5), (31.5, 28.5), (46.5, 28.5), (36.5, 33.5), (39.5, 37.5), (36.5, s + 8.5), (41.5, s + 7.5), (26.5, 23.5)];
        for (x, z) in spots {
            self.block(x.floor() as i32, z.floor() as i32);
            g.add(&post, at(x, 0.85, z), WHITE);
            g.add(&lamp, at(x, 1.8, z), WHITE);
            g.add(&cap, trs(v3(x, 2.03, z), std::f32::consts::FRAC_PI_4, Vec3::ONE), WHITE);
            self.lanterns.push(v3(x, 1.85, z));
        }
        self.mesh.add(&g, Mat4::IDENTITY, WHITE);
    }

    fn build_falls(&mut self) {
        let full = Uv { u0: 0., v0: 0., u1: 1., v1: 1. };
        for i in self.river {
            let c = self.cliff[i as usize] as f32; let h = 2.0;
            let (x0, x1) = (i as f32, i as f32 + 1.0);
            let z = c + 0.02;
            self.falls.quad([v3(x0, -0.245, z), v3(x1, -0.245, z), v3(x1, h + 0.005, z), v3(x0, h + 0.005, z)], full, WHITE, [6.; 4]);
            self.falls.quad([v3(x0, h + 0.02, c), v3(x1, h + 0.02, c), v3(x1, h + 0.02, -4.), v3(x0, h + 0.02, -4.)], full, WHITE, [2.; 4]);
        }
        self.fall_base = v3(20.0, 0.0, self.cliff[19] as f32 + 0.6);
    }
}

fn geo_white() -> [u8; 4] { WHITE }
