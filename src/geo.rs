//! Vertex format, texture atlas and primitive mesh builders.
use crate::pixel::{Img, Rgba};
use macroquad::math::{Mat3, Mat4, Vec3};

/// `k` is a per-vertex tag the shaders read: foliage sway weight, or waterfall speed.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct V { pub p: [f32; 3], pub uv: [f32; 2], pub n: [f32; 3], pub c: [u8; 4], pub k: f32 }

#[derive(Clone, Copy, Default, Debug)]
pub struct Uv { pub u0: f32, pub v0: f32, pub u1: f32, pub v1: f32 }
impl Uv {
    pub fn at(&self, s: f32, t: f32) -> [f32; 2] { [self.u0 + (self.u1 - self.u0) * s, self.v0 + (self.v1 - self.v0) * t] }
    pub fn sub(&self, s0: f32, t0: f32, s1: f32, t1: f32) -> Uv {
        let a = self.at(s0, t0); let b = self.at(s1, t1);
        Uv { u0: a[0], v0: a[1], u1: b[0], v1: b[1] }
    }
    pub fn flip_x(&self) -> Uv { Uv { u0: self.u1, v0: self.v0, u1: self.u0, v1: self.v1 } }
}

pub const WHITE: Rgba = [255, 255, 255, 255];

#[derive(Default, Clone)]
pub struct Geo { pub v: Vec<V>, pub i: Vec<u32> }

impl Geo {
    pub fn new() -> Self { Self::default() }
    pub fn clear(&mut self) { self.v.clear(); self.i.clear(); }
    /// Quad with corners bottom-left, bottom-right, top-right, top-left (counter-clockwise from the front).
    pub fn quad(&mut self, p: [Vec3; 4], uv: Uv, c: Rgba, k: [f32; 4]) {
        let n = (p[1] - p[0]).cross(p[3] - p[0]).normalize_or_zero();
        self.quad_n(p, uv, c, k, n);
    }
    pub fn quad_n(&mut self, p: [Vec3; 4], uv: Uv, c: Rgba, k: [f32; 4], n: Vec3) {
        let b = self.v.len() as u32;
        let uvs = [uv.at(0., 1.), uv.at(1., 1.), uv.at(1., 0.), uv.at(0., 0.)];
        for j in 0..4 { self.v.push(V { p: p[j].into(), uv: uvs[j], n: n.into(), c, k: k[j] }); }
        self.i.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    pub fn tri(&mut self, p: [Vec3; 3], uv: [[f32; 2]; 3], c: Rgba) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        let b = self.v.len() as u32;
        for j in 0..3 { self.v.push(V { p: p[j].into(), uv: uv[j], n: n.into(), c, k: 0. }); }
        self.i.extend_from_slice(&[b, b + 1, b + 2]);
    }
    /// Append `o` transformed by `m`, multiplying vertex colors by `tint`.
    pub fn add(&mut self, o: &Geo, m: Mat4, tint: Rgba) {
        let b = self.v.len() as u32;
        let nm = Mat3::from_mat4(m).inverse().transpose();
        for v in &o.v {
            let p = m.transform_point3(Vec3::from(v.p));
            let n = (nm * Vec3::from(v.n)).normalize_or_zero();
            let mut c = v.c;
            for j in 0..4 { c[j] = ((c[j] as u32 * tint[j] as u32) / 255) as u8; }
            self.v.push(V { p: p.into(), uv: v.uv, n: n.into(), c, k: v.k });
        }
        self.i.extend(o.i.iter().map(|i| i + b));
    }
    pub fn set_k(mut self, k: f32) -> Self { for v in self.v.iter_mut() { v.k = k; } self }
}

pub fn v3(x: f32, y: f32, z: f32) -> Vec3 { Vec3::new(x, y, z) }

/// Unit-centered box; faces ordered +x, -x, +y, -y, +z, -z (textures upright on the sides).
pub fn cube(s: Vec3, f: [Uv; 6], c: Rgba) -> Geo {
    let (x, y, z) = (s.x / 2., s.y / 2., s.z / 2.);
    let mut g = Geo::new();
    let k = [0.; 4];
    g.quad([v3(x, -y, z), v3(x, -y, -z), v3(x, y, -z), v3(x, y, z)], f[0], c, k);
    g.quad([v3(-x, -y, -z), v3(-x, -y, z), v3(-x, y, z), v3(-x, y, -z)], f[1], c, k);
    g.quad([v3(-x, y, z), v3(x, y, z), v3(x, y, -z), v3(-x, y, -z)], f[2], c, k);
    g.quad([v3(-x, -y, -z), v3(x, -y, -z), v3(x, -y, z), v3(-x, -y, z)], f[3], c, k);
    g.quad([v3(-x, -y, z), v3(x, -y, z), v3(x, y, z), v3(-x, y, z)], f[4], c, k);
    g.quad([v3(x, -y, -z), v3(-x, -y, -z), v3(-x, y, -z), v3(x, y, -z)], f[5], c, k);
    g
}
pub fn cube1(s: Vec3, uv: Uv, c: Rgba) -> Geo { cube(s, [uv; 6], c) }

/// Cylinder centered on the origin; `rt`/`rb` top and bottom radius.
pub fn cylinder(rt: f32, rb: f32, h: f32, seg: usize, side: Uv, cap: Uv, c: Rgba) -> Geo {
    let mut g = Geo::new();
    let hy = h / 2.;
    let slope = (rb - rt) / h;
    for i in 0..seg {
        let (a0, a1) = (i as f32 / seg as f32 * std::f32::consts::TAU, (i + 1) as f32 / seg as f32 * std::f32::consts::TAU);
        let (c0, s0, c1, s1) = (a0.cos(), a0.sin(), a1.cos(), a1.sin());
        let p = [v3(c0 * rb, -hy, s0 * rb), v3(c1 * rb, -hy, s1 * rb), v3(c1 * rt, hy, s1 * rt), v3(c0 * rt, hy, s0 * rt)];
        let am = (a0 + a1) / 2.;
        let n = v3(am.cos(), slope, am.sin()).normalize();
        let uv = side.sub(i as f32 / seg as f32, 0., (i + 1) as f32 / seg as f32, 1.);
        // wind so the outward face is front-facing
        g.quad_n([p[1], p[0], p[3], p[2]], uv.flip_x(), c, [0.; 4], n);
        if rt > 0.001 { g.tri([v3(0., hy, 0.), p[2], p[3]], [cap.at(0.5, 0.5), cap.at(0.5 + c1 * 0.5, 0.5 + s1 * 0.5), cap.at(0.5 + c0 * 0.5, 0.5 + s0 * 0.5)], c); }
    }
    g
}

pub fn cone(r: f32, h: f32, seg: usize, uv: Uv, c: Rgba) -> Geo { cylinder(0.0, r, h, seg, uv, uv, c) }

/// Flat-shaded icosahedron with planar UVs, used for foliage clumps and rocks.
pub fn ico(r: f32, uv: Uv, c: Rgba) -> Geo {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let raw = [(-1., t, 0.), (1., t, 0.), (-1., -t, 0.), (1., -t, 0.), (0., -1., t), (0., 1., t), (0., -1., -t), (0., 1., -t), (t, 0., -1.), (t, 0., 1.), (-t, 0., -1.), (-t, 0., 1.)];
    let p: Vec<Vec3> = raw.iter().map(|&(x, y, z)| v3(x, y, z).normalize() * r).collect();
    let f = [[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]];
    let mut g = Geo::new();
    for (k, tri) in f.iter().enumerate() {
        let q = [p[tri[0]], p[tri[1]], p[tri[2]]];
        let o = (k % 4) as f32 * 0.25;
        let m = |v: Vec3| uv.at((o + (v.x / r * 0.5 + 0.5) * 0.5).fract(), (v.y / r * 0.5 + 0.5).clamp(0.0, 1.0) * 0.98);
        g.tri(q, [m(q[0]), m(q[1]), m(q[2])], c);
    }
    g
}

/// Shelf-packed texture atlas with 1px edge-extended gutters (safe for nearest filtering).
pub struct Atlas { pub img: Img, x: usize, y: usize, row: usize }
impl Atlas {
    pub fn new(size: usize) -> Self { Atlas { img: Img::new(size, size), x: 0, y: 0, row: 0 } }
    pub fn add(&mut self, s: &Img) -> Uv {
        let (w, h) = (s.w + 2, s.h + 2);
        if self.x + w > self.img.w { self.x = 0; self.y += self.row; self.row = 0; }
        assert!(self.y + h <= self.img.h, "texture atlas full");
        let (ox, oy) = (self.x as i32 + 1, self.y as i32 + 1);
        for y in -1..=s.h as i32 { for x in -1..=s.w as i32 {
            let c = s.get(x.clamp(0, s.w as i32 - 1), y.clamp(0, s.h as i32 - 1));
            self.img.set(ox + x, oy + y, c);
        } }
        self.x += w; self.row = self.row.max(h);
        let sz = self.img.w as f32;
        Uv { u0: ox as f32 / sz, v0: oy as f32 / sz, u1: (ox as f32 + s.w as f32) / sz, v1: (oy as f32 + s.h as f32) / sz }
    }
}
