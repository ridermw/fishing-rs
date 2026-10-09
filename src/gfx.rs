//! Raw miniquad renderer: shadow map -> lit scene -> bloom -> tilt-shift composite.
use crate::geo::{Geo, V};
use macroquad::miniquad::*;

pub const SHADOW_SIZE: u32 = 2048;
pub const MAX_LAMPS: usize = 10;

/// Unlit particle / decal vertex. `k`: 0 soft dot, 1 solid, 2 ring.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PV { pub p: [f32; 3], pub uv: [f32; 2], pub c: [u8; 4], pub k: f32 }

#[derive(Default)]
pub struct PGeo { pub v: Vec<PV>, pub i: Vec<u32> }
impl PGeo {
    pub fn clear(&mut self) { self.v.clear(); self.i.clear(); }
    /// Re-orders quads far-to-near from `eye` so blended quads composite correctly.
    pub fn sort_back_to_front(&mut self, eye: [f32; 3]) {
        let d = |q: &[PV]| { let c = q[0].p; (c[0] - eye[0]).powi(2) + (c[1] - eye[1]).powi(2) + (c[2] - eye[2]).powi(2) };
        let mut quads: Vec<&[PV]> = self.v.chunks(4).collect();
        quads.sort_by(|a, b| d(b).partial_cmp(&d(a)).unwrap_or(std::cmp::Ordering::Equal));
        let v: Vec<PV> = quads.into_iter().flatten().copied().collect();
        self.v = v;
    }
    pub fn quad(&mut self, p: [[f32; 3]; 4], c: [u8; 4], k: f32) {
        let b = self.v.len() as u32;
        let uv = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
        for j in 0..4 { self.v.push(PV { p: p[j], uv: uv[j], c, k }); }
        self.i.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
}

const COMMON: &str = r#"#version 100
precision highp float;
"#;

const LIT_VS: &str = r#"
attribute vec3 position; attribute vec2 texcoord; attribute vec3 normal; attribute vec4 color0; attribute float kind;
uniform mat4 uVP; uniform mat4 uLightVP; uniform float uTime;
varying vec2 vUv; varying vec3 vN; varying vec4 vC; varying vec3 vW; varying vec4 vL;
void main() {
  vec3 p = position;
  p.x += sin(uTime * 2.0 + p.x * 0.7 + p.z * 0.5) * 0.07 * kind;
  vUv = texcoord; vN = normal; vC = color0 / 255.0; vW = p;
  vL = uLightVP * vec4(p + normal * 0.035, 1.0);
  gl_Position = uVP * vec4(p, 1.0);
}"#;

const LIT_FS: &str = r#"
uniform sampler2D uAtlas; uniform sampler2D uShadow;
uniform vec3 uSunDir; uniform vec3 uSunCol; uniform vec3 uSky; uniform vec3 uGround; uniform vec3 uFogCol; uniform vec3 uCam; uniform vec3 uLampCol;
uniform vec4 uMisc; uniform vec4 uLamps[10];
varying vec2 vUv; varying vec3 vN; varying vec4 vC; varying vec3 vW; varying vec4 vL;
float unpackDepth(vec4 c) { return dot(c, vec4(1.0, 1.0 / 255.0, 1.0 / 65025.0, 1.0 / 16581375.0)); }
float shadowAt(vec4 l, float bias) {
  vec3 s = l.xyz / l.w * 0.5 + 0.5;
  if (s.x < 0.0 || s.x > 1.0 || s.y < 0.0 || s.y > 1.0 || s.z > 1.0) return 1.0;
  float sum = 0.0;
  for (int i = -1; i <= 1; i++) for (int j = -1; j <= 1; j++)
    sum += step(s.z - bias, unpackDepth(texture2D(uShadow, s.xy + vec2(float(i), float(j)) / 2048.0)));
  return sum / 9.0;
}
vec3 rrt(vec3 v) { vec3 a = v * (v + 0.0245786) - 0.000090537; vec3 b = v * (0.983729 * v + 0.4329510) + 0.238081; return a / b; }
vec3 aces(vec3 c) {
  mat3 i = mat3(vec3(0.59719, 0.07600, 0.02840), vec3(0.35458, 0.90834, 0.13383), vec3(0.04823, 0.01566, 0.83777));
  mat3 o = mat3(vec3(1.60475, -0.10208, -0.00327), vec3(-0.53108, 1.10813, -0.07276), vec3(-0.07367, -0.00605, 1.07602));
  return clamp(o * rrt(i * (c * uMisc.w / 0.6)), 0.0, 1.0);
}
void main() {
  vec4 t = texture2D(uAtlas, vUv);
  if (t.a < 0.25) discard;
  vec3 alb = pow(t.rgb * vC.rgb, vec3(2.2));
  vec3 n = normalize(vN);
  vec3 view = normalize(uCam - vW);
  if (dot(n, view) < 0.0) n = -n;
  float ndl = dot(n, uSunDir);
  float sh = ndl > 0.0 ? shadowAt(vL, 0.0008 + 0.0012 * (1.0 - ndl)) : 0.0;
  vec3 hemi = mix(uGround, uSky, n.y * 0.5 + 0.5);
  vec3 col = alb * 0.3183 * (uSunCol * max(ndl, 0.0) * sh + hemi);
  for (int k = 0; k < 10; k++) {
    vec3 d = uLamps[k].xyz - vW; float dist = max(length(d), 0.2);
    float win = clamp(1.0 - pow(dist / 12.0, 4.0), 0.0, 1.0);
    col += alb * 0.3183 * uLampCol * uLamps[k].w * (0.25 + 0.75 * max(dot(n, d / dist), 0.0)) * win * win / pow(dist, 1.4);
  }
  float night = uMisc.z;
  if (t.a < 0.9) {
    if (t.a > 0.7) col += vec3(1.0, 0.43, 0.06) * t.rgb * (0.05 + night * 2.2);
    else if (t.a > 0.5) col += vec3(1.0, 0.43, 0.06) * (0.3 + night * 4.5) * vC.rgb;
    else col += vec3(0.01, 0.15, 0.25) * 0.6 + alb * 0.4;
  }
  vec3 outc = pow(aces(col), vec3(1.0 / 2.2));
  float f = smoothstep(uMisc.x, uMisc.y, distance(vW, uCam));
  gl_FragColor = vec4(mix(outc, uFogCol, f), 1.0);
}"#;

const SHADOW_VS: &str = r#"
attribute vec3 position; attribute vec2 texcoord; attribute vec3 normal; attribute vec4 color0; attribute float kind;
uniform mat4 uLightVP; uniform float uTime;
varying vec2 vUv;
void main() {
  vec3 p = position;
  p.x += sin(uTime * 2.0 + p.x * 0.7 + p.z * 0.5) * 0.07 * kind;
  vUv = texcoord;
  gl_Position = uLightVP * vec4(p, 1.0);
}"#;
const SHADOW_FS: &str = r#"
uniform sampler2D uAtlas;
varying vec2 vUv;
void main() {
  if (texture2D(uAtlas, vUv).a < 0.25) discard;
  float z = gl_FragCoord.z;
  vec4 e = fract(vec4(1.0, 255.0, 65025.0, 16581375.0) * z);
  e -= e.yzww * vec4(1.0 / 255.0, 1.0 / 255.0, 1.0 / 255.0, 0.0);
  gl_FragColor = e;
}"#;

const WATER_VS: &str = r#"
attribute vec3 position; attribute vec2 texcoord; attribute vec3 normal; attribute vec4 color0; attribute float kind;
uniform mat4 uVP;
varying vec3 vW;
void main() { vW = position; gl_Position = uVP * vec4(position, 1.0); }"#;
const WATER_FS: &str = r#"
uniform sampler2D uInfo;
uniform float uTime; uniform vec2 uSize; uniform vec3 uLight; uniform vec3 uFog; uniform vec2 uFogNF; uniform float uNight; uniform vec3 uCam;
varying vec3 vW;
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
void main() {
  vec2 p = (floor(vW.xz * 16.0) + 0.5) / 16.0;
  vec4 info = texture2D(uInfo, p / uSize);
  vec2 inMap = step(vec2(0.0), p) * step(p, uSize);
  float d = mix(10.0, info.r * 255.0 / 24.0, inMap.x * inMap.y);
  float fresh = step(0.5, info.g) * inMap.x * inMap.y;
  vec3 sea = mix(vec3(0.33, 0.80, 0.80), vec3(0.06, 0.30, 0.55), smoothstep(0.55, 5.0, d));
  vec3 pond = mix(vec3(0.38, 0.66, 0.50), vec3(0.10, 0.32, 0.36), smoothstep(0.55, 3.5, d));
  vec3 col = mix(sea, pond, fresh);
  float w = sin(p.x * 2.1 + uTime * 1.1 + sin(p.y * 1.3 + uTime * 0.6) * 2.0) + sin(p.y * 2.7 - uTime * 0.8 + p.x * 0.4);
  col *= 0.9 + 0.1 * step(0.7, w);
  col += vec3(0.10, 0.14, 0.16) * step(1.55, w);
  float band = sin(d * 9.0 - uTime * 2.2);
  float foam = step(0.55, band) * (1.0 - smoothstep(0.7, 1.5, d)) + (1.0 - smoothstep(0.62, 0.72, d));
  col = mix(col, vec3(0.95, 0.98, 1.0), clamp(foam, 0.0, 1.0) * 0.85);
  col *= uLight;
  float sp = hash(floor(p * 4.0) + floor(uTime * 3.0));
  col += vec3(1.0, 0.95, 0.8) * step(0.993, sp) * (1.0 - fresh * 0.5) * (0.6 + uNight * 0.6);
  float f = smoothstep(uFogNF.x, uFogNF.y, distance(vW, uCam));
  gl_FragColor = vec4(mix(pow(col, vec3(1.0 / 2.2)), uFog, f), 0.9);
}"#;

const FALL_VS: &str = r#"
attribute vec3 position; attribute vec2 texcoord; attribute vec3 normal; attribute vec4 color0; attribute float kind;
uniform mat4 uVP;
varying vec2 vUv; varying float vSpeed;
void main() { vUv = texcoord; vSpeed = kind; gl_Position = uVP * vec4(position, 1.0); }"#;
const FALL_FS: &str = r#"
uniform float uTime; uniform vec3 uLight;
varying vec2 vUv; varying float vSpeed;
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
void main() {
  vec2 q = floor(vUv * vec2(16.0, 48.0));
  float s = hash(vec2(q.x, floor(q.y * 0.25 - uTime * vSpeed)));
  vec3 col = mix(vec3(0.45, 0.72, 0.86), vec3(0.92, 0.97, 1.0), step(0.55, s));
  col = mix(col, vec3(0.3, 0.55, 0.75), step(0.9, s) * 0.6);
  gl_FragColor = vec4(pow(col * uLight * 1.15, vec3(1.0 / 2.2)), 0.92);
}"#;

const PART_VS: &str = r#"
attribute vec3 position; attribute vec2 texcoord; attribute vec4 color0; attribute float kind;
uniform mat4 uVP;
varying vec2 vUv; varying vec4 vC; varying float vK;
void main() { vUv = texcoord; vC = color0 / 255.0; vK = kind; gl_Position = uVP * vec4(position, 1.0); }"#;
const PART_FS: &str = r#"
varying vec2 vUv; varying vec4 vC; varying float vK;
void main() {
  float d = length(vUv - 0.5) * 2.0;
  float a = 1.0;
  if (vK < 0.5) a = floor(clamp(1.0 - d, 0.0, 1.0) * 4.0 + 0.5) / 4.0;
  else if (vK > 1.5) a = step(0.78, d) * step(d, 1.0);
  if (a <= 0.0) discard;
  gl_FragColor = vec4(vC.rgb, vC.a * a);
}"#;

const POST_VS: &str = r#"
attribute vec2 position;
varying vec2 vUv;
void main() { vUv = position * 0.5 + 0.5; gl_Position = vec4(position, 0.0, 1.0); }"#;
const BRIGHT_FS: &str = r#"
uniform sampler2D uTex; uniform vec2 uThresh;
varying vec2 vUv;
void main() {
  vec3 c = texture2D(uTex, vUv).rgb;
  float l = dot(c, vec3(0.299, 0.587, 0.114));
  gl_FragColor = vec4(c * smoothstep(uThresh.x, uThresh.y, l), 1.0);
}"#;
const BLUR_FS: &str = r#"
uniform sampler2D uTex; uniform vec2 uDir;
varying vec2 vUv;
void main() {
  vec3 c = texture2D(uTex, vUv).rgb * 0.227;
  c += (texture2D(uTex, vUv + uDir * 1.38).rgb + texture2D(uTex, vUv - uDir * 1.38).rgb) * 0.316;
  c += (texture2D(uTex, vUv + uDir * 3.23).rgb + texture2D(uTex, vUv - uDir * 3.23).rgb) * 0.070;
  gl_FragColor = vec4(c, 1.0);
}"#;
const COMP_FS: &str = r#"
uniform sampler2D uTex; uniform sampler2D uBloom;
uniform vec2 uRes; uniform vec4 uTilt; uniform vec2 uGrade;
varying vec2 vUv;
void main() {
  float b = clamp((abs(vUv.y - uTilt.x) - uTilt.y) / 0.35, 0.0, 1.0);
  b = b * b * uTilt.z;
  vec3 sum = vec3(0.0); float tot = 0.0;
  for (int i = -3; i <= 3; i++) for (int j = -3; j <= 3; j++) {
    vec2 o = vec2(float(i), float(j));
    float w = exp(-dot(o, o) / 8.0);
    sum += texture2D(uTex, vUv + o * b / uRes).rgb * w; tot += w;
  }
  vec3 c = sum / tot + texture2D(uBloom, vUv).rgb * uTilt.w;
  float l = dot(c, vec3(0.299, 0.587, 0.114));
  c = mix(vec3(l), c, 1.12);
  c *= mix(vec3(1.0), vec3(1.05, 0.98, 0.9), uGrade.x);
  c *= mix(0.7, 1.0, smoothstep(0.95, 0.35, length(vUv - 0.5)));
  gl_FragColor = vec4(c, 1.0);
}"#;

#[repr(C)] pub struct LitU { pub vp: [f32; 16], pub light_vp: [f32; 16], pub time: f32, pub sun_dir: [f32; 3], pub sun_col: [f32; 3], pub sky: [f32; 3], pub ground: [f32; 3],
    pub fog_col: [f32; 3], pub cam: [f32; 3], pub lamp_col: [f32; 3], pub misc: [f32; 4], pub lamps: [[f32; 4]; MAX_LAMPS] }
#[repr(C)] struct ShadowU { light_vp: [f32; 16], time: f32 }
#[repr(C)] struct WaterU { vp: [f32; 16], time: f32, size: [f32; 2], light: [f32; 3], fog: [f32; 3], fog_nf: [f32; 2], night: f32, cam: [f32; 3] }
#[repr(C)] struct FallU { vp: [f32; 16], time: f32, light: [f32; 3] }
#[repr(C)] struct VpU { vp: [f32; 16] }
#[repr(C)] struct Vec2U { v: [f32; 2] }
#[repr(C)] struct CompU { res: [f32; 2], tilt: [f32; 4], grade: [f32; 2] }

pub struct Frame {
    pub lit: LitU,
    pub water_light: [f32; 3],
    pub fog_srgb: [f32; 3],
    pub sky_srgb: [f32; 3],
    pub bloom: f32,
    pub warm: f32,
}

struct Target { color: TextureId, pass: RenderPass, w: u32, h: u32 }
struct Mesh { vb: BufferId, ib: BufferId, n: i32, cap_v: usize, cap_i: usize }

pub struct Gfx {
    lit: Pipeline, shadow: Pipeline, water: Pipeline, fall: Pipeline, part_alpha: Pipeline, part_add: Pipeline,
    bright: Pipeline, blur: Pipeline, comp: Pipeline,
    atlas: TextureId, info: TextureId,
    world: Mesh, falls: Mesh, water_mesh: Mesh, dynamic: Mesh, parts_a: Mesh, parts_b: Mesh, quad: Mesh,
    shadow_rt: Target, scene: Option<Target>, bloom_a: Option<Target>, bloom_b: Option<Target>,
    pub info_size: [f32; 2],
}

fn uniforms(list: &[(&str, UniformType, usize)]) -> UniformBlockLayout {
    UniformBlockLayout { uniforms: list.iter().map(|(n, t, c)| UniformDesc::new(n, *t).array(*c)).collect() }
}

fn shader(ctx: &mut dyn RenderingBackend, vs: &str, fs: &str, images: &[&str], u: &[(&str, UniformType, usize)]) -> ShaderId {
    let (vs, fs) = (format!("{COMMON}{vs}"), format!("{COMMON}{fs}"));
    match ctx.new_shader(ShaderSource::Glsl { vertex: &vs, fragment: &fs }, ShaderMeta { images: images.iter().map(|s| s.to_string()).collect(), uniforms: uniforms(u) }) {
        Ok(s) => s,
        Err(e) => panic!("shader error: {e:?}"),
    }
}

fn lit_attrs() -> Vec<VertexAttribute> {
    vec![VertexAttribute::new("position", VertexFormat::Float3), VertexAttribute::new("texcoord", VertexFormat::Float2),
         VertexAttribute::new("normal", VertexFormat::Float3), VertexAttribute::new("color0", VertexFormat::Byte4), VertexAttribute::new("kind", VertexFormat::Float1)]
}
fn part_attrs() -> Vec<VertexAttribute> {
    vec![VertexAttribute::new("position", VertexFormat::Float3), VertexAttribute::new("texcoord", VertexFormat::Float2),
         VertexAttribute::new("color0", VertexFormat::Byte4), VertexAttribute::new("kind", VertexFormat::Float1)]
}

fn blend_alpha() -> Option<BlendState> { Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))) }
fn blend_add() -> Option<BlendState> { Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::One)) }

fn static_mesh<T>(ctx: &mut dyn RenderingBackend, v: &[T], i: &[u32]) -> Mesh {
    let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(v));
    let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(i));
    Mesh { vb, ib, n: i.len() as i32, cap_v: v.len(), cap_i: i.len() }
}
fn stream_mesh<T>(ctx: &mut dyn RenderingBackend, cap_v: usize, cap_i: usize) -> Mesh {
    let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Stream, BufferSource::empty::<T>(cap_v));
    let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Stream, BufferSource::empty::<u32>(cap_i));
    Mesh { vb, ib, n: 0, cap_v, cap_i }
}
fn upload<T>(ctx: &mut dyn RenderingBackend, m: &mut Mesh, v: &[T], i: &[u32]) {
    let nv = v.len().min(m.cap_v);
    let ni = if nv < v.len() { 0 } else { i.len().min(m.cap_i) };
    if ni > 0 {
        ctx.buffer_update(m.vb, BufferSource::slice(&v[..nv]));
        ctx.buffer_update(m.ib, BufferSource::slice(&i[..ni]));
    }
    m.n = ni as i32;
}

fn target(ctx: &mut dyn RenderingBackend, w: u32, h: u32, depth: bool, filter: FilterMode) -> Target {
    let color = ctx.new_render_texture(TextureParams { width: w, height: h, format: TextureFormat::RGBA8, min_filter: filter, mag_filter: filter, wrap: TextureWrap::Clamp, ..Default::default() });
    let depth = depth.then(|| ctx.new_render_texture(TextureParams { width: w, height: h, format: TextureFormat::Depth, ..Default::default() }));
    let pass = ctx.new_render_pass(color, depth);
    Target { color, pass, w, h }
}
/// `delete_render_pass` also frees the pass's color and depth textures.
fn drop_target(ctx: &mut dyn RenderingBackend, t: Target) { ctx.delete_render_pass(t.pass); }

impl Gfx {
    pub fn new(ctx: &mut dyn RenderingBackend, atlas_rgba: &[u8], atlas_size: u16, world: &Geo, falls: &Geo, info: &[u8], info_w: u16, info_h: u16) -> Gfx {
        use UniformType::*;
        let lit_s = shader(ctx, LIT_VS, LIT_FS, &["uAtlas", "uShadow"], &[("uVP", Mat4, 1), ("uLightVP", Mat4, 1), ("uTime", Float1, 1), ("uSunDir", Float3, 1), ("uSunCol", Float3, 1),
            ("uSky", Float3, 1), ("uGround", Float3, 1), ("uFogCol", Float3, 1), ("uCam", Float3, 1), ("uLampCol", Float3, 1), ("uMisc", Float4, 1), ("uLamps", Float4, MAX_LAMPS)]);
        let shadow_s = shader(ctx, SHADOW_VS, SHADOW_FS, &["uAtlas"], &[("uLightVP", Mat4, 1), ("uTime", Float1, 1)]);
        let water_s = shader(ctx, WATER_VS, WATER_FS, &["uInfo"], &[("uVP", Mat4, 1), ("uTime", Float1, 1), ("uSize", Float2, 1), ("uLight", Float3, 1), ("uFog", Float3, 1), ("uFogNF", Float2, 1), ("uNight", Float1, 1), ("uCam", Float3, 1)]);
        let fall_s = shader(ctx, FALL_VS, FALL_FS, &[], &[("uVP", Mat4, 1), ("uTime", Float1, 1), ("uLight", Float3, 1)]);
        let part_s = shader(ctx, PART_VS, PART_FS, &[], &[("uVP", Mat4, 1)]);
        let bright_s = shader(ctx, POST_VS, BRIGHT_FS, &["uTex"], &[("uThresh", Float2, 1)]);
        let blur_s = shader(ctx, POST_VS, BLUR_FS, &["uTex"], &[("uDir", Float2, 1)]);
        let comp_s = shader(ctx, POST_VS, COMP_FS, &["uTex", "uBloom"], &[("uRes", Float2, 1), ("uTilt", Float4, 1), ("uGrade", Float2, 1)]);

        let opaque = PipelineParams { depth_test: Comparison::LessOrEqual, depth_write: true, ..Default::default() };
        // miniquad ties the depth test to `depth_write`, so blended passes keep it on (and are drawn back-to-front).
        let transparent = PipelineParams { depth_test: Comparison::LessOrEqual, depth_write: true, color_blend: blend_alpha(), ..Default::default() };
        let additive = PipelineParams { depth_test: Comparison::LessOrEqual, depth_write: true, color_blend: blend_add(), ..Default::default() };
        let lay = [BufferLayout::default()];
        let lit = ctx.new_pipeline(&lay, &lit_attrs(), lit_s, opaque.clone());
        let shadow = ctx.new_pipeline(&lay, &lit_attrs(), shadow_s, opaque.clone());
        let water = ctx.new_pipeline(&lay, &lit_attrs(), water_s, transparent.clone());
        let fall = ctx.new_pipeline(&lay, &lit_attrs(), fall_s, transparent.clone());
        let part_alpha = ctx.new_pipeline(&lay, &part_attrs(), part_s, transparent);
        let part_add = ctx.new_pipeline(&lay, &part_attrs(), part_s, additive);
        let post_attr = [VertexAttribute::new("position", VertexFormat::Float2)];
        let bright = ctx.new_pipeline(&lay, &post_attr, bright_s, PipelineParams::default());
        let blur = ctx.new_pipeline(&lay, &post_attr, blur_s, PipelineParams::default());
        let comp = ctx.new_pipeline(&lay, &post_attr, comp_s, PipelineParams::default());

        let atlas = ctx.new_texture_from_rgba8(atlas_size, atlas_size, atlas_rgba);
        ctx.texture_set_filter(atlas, FilterMode::Nearest, MipmapFilterMode::None);
        ctx.texture_set_wrap(atlas, TextureWrap::Clamp, TextureWrap::Clamp);
        let info_tex = ctx.new_texture_from_rgba8(info_w, info_h, info);
        ctx.texture_set_filter(info_tex, FilterMode::Linear, MipmapFilterMode::None);
        ctx.texture_set_wrap(info_tex, TextureWrap::Clamp, TextureWrap::Clamp);

        let wv = |x: f32, z: f32| V { p: [x, -0.22, z], uv: [0., 0.], n: [0., 1., 0.], c: [255; 4], k: 0. };
        let (cx, cz) = (info_w as f32 / 2.0, info_h as f32 / 2.0);
        let water_v = [wv(cx - 200., cz + 200.), wv(cx + 200., cz + 200.), wv(cx + 200., cz - 200.), wv(cx - 200., cz - 200.)];
        let quad_v: [[f32; 2]; 4] = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]];
        let qi = [0u32, 1, 2, 0, 2, 3];

        let world_mesh = static_mesh(ctx, &world.v, &world.i);
        let falls_mesh = static_mesh(ctx, &falls.v, &falls.i);
        let water_mesh = static_mesh(ctx, &water_v, &qi);
        let quad = static_mesh(ctx, &quad_v, &qi);
        let dynamic = stream_mesh::<V>(ctx, 40000, 60000);
        let parts_a = stream_mesh::<PV>(ctx, 8000, 12000);
        let parts_b = stream_mesh::<PV>(ctx, 8000, 12000);
        let shadow_rt = target(ctx, SHADOW_SIZE, SHADOW_SIZE, true, FilterMode::Nearest);
        Gfx { lit, shadow, water, fall, part_alpha, part_add, bright, blur, comp, atlas, info: info_tex,
            world: world_mesh, falls: falls_mesh, water_mesh, dynamic, parts_a, parts_b, quad,
            shadow_rt, scene: None, bloom_a: None, bloom_b: None, info_size: [info_w as f32, info_h as f32] }
    }

    fn ensure_targets(&mut self, ctx: &mut dyn RenderingBackend, w: u32, h: u32) {
        if self.scene.as_ref().map_or(false, |t| t.w == w && t.h == h) { return; }
        // Point the texture units at the atlas first: miniquad's binding cache would otherwise re-bind deleted textures.
        ctx.apply_pipeline(&self.comp);
        ctx.apply_bindings(&Bindings { vertex_buffers: vec![self.quad.vb], index_buffer: self.quad.ib, images: vec![self.atlas, self.atlas] });
        for t in [self.scene.take(), self.bloom_a.take(), self.bloom_b.take()].into_iter().flatten() { drop_target(ctx, t); }
        self.scene = Some(target(ctx, w, h, true, FilterMode::Linear));
        let (bw, bh) = ((w / 4).max(1), (h / 4).max(1));
        self.bloom_a = Some(target(ctx, bw, bh, false, FilterMode::Linear));
        self.bloom_b = Some(target(ctx, bw, bh, false, FilterMode::Linear));
    }

    fn draw_mesh(ctx: &mut dyn RenderingBackend, m: &Mesh, images: Vec<TextureId>) {
        if m.n == 0 { return; }
        ctx.apply_bindings(&Bindings { vertex_buffers: vec![m.vb], index_buffer: m.ib, images });
        ctx.draw(0, m.n, 1);
    }

    /// Renders one frame of the 3D scene into the default framebuffer.
    pub fn render(&mut self, ctx: &mut dyn RenderingBackend, f: &Frame, time: f32, dynamic: &Geo, alpha: &PGeo, add: &PGeo, w: u32, h: u32) {
        self.ensure_targets(ctx, w, h);
        upload(ctx, &mut self.dynamic, &dynamic.v, &dynamic.i);
        upload(ctx, &mut self.parts_a, &alpha.v, &alpha.i);
        upload(ctx, &mut self.parts_b, &add.v, &add.i);

        // 1. shadow map (packed depth)
        ctx.begin_pass(Some(self.shadow_rt.pass), PassAction::clear_color(1.0, 1.0, 1.0, 1.0));
        ctx.apply_pipeline(&self.shadow);
        ctx.apply_uniforms(UniformsSource::table(&ShadowU { light_vp: f.lit.light_vp, time }));
        Self::draw_mesh(ctx, &self.world, vec![self.atlas]);
        Self::draw_mesh(ctx, &self.dynamic, vec![self.atlas]);
        ctx.end_render_pass();

        // 2. lit scene
        let scene = self.scene.as_ref().unwrap();
        let s = f.sky_srgb;
        ctx.begin_pass(Some(scene.pass), PassAction::clear_color(s[0], s[1], s[2], 1.0));
        ctx.apply_pipeline(&self.lit);
        ctx.apply_uniforms(UniformsSource::table(&f.lit));
        Self::draw_mesh(ctx, &self.world, vec![self.atlas, self.shadow_rt.color]);
        Self::draw_mesh(ctx, &self.dynamic, vec![self.atlas, self.shadow_rt.color]);
        ctx.apply_pipeline(&self.water);
        let cam = f.lit.cam;
        ctx.apply_uniforms(UniformsSource::table(&WaterU { vp: f.lit.vp, time, size: self.info_size, light: f.water_light, fog: f.fog_srgb,
            fog_nf: [f.lit.misc[0], f.lit.misc[1]], night: f.lit.misc[2], cam }));
        Self::draw_mesh(ctx, &self.water_mesh, vec![self.info]);
        ctx.apply_pipeline(&self.fall);
        ctx.apply_uniforms(UniformsSource::table(&FallU { vp: f.lit.vp, time, light: f.water_light }));
        Self::draw_mesh(ctx, &self.falls, vec![]);
        ctx.apply_pipeline(&self.part_alpha);
        ctx.apply_uniforms(UniformsSource::table(&VpU { vp: f.lit.vp }));
        Self::draw_mesh(ctx, &self.parts_a, vec![]);
        ctx.apply_pipeline(&self.part_add);
        ctx.apply_uniforms(UniformsSource::table(&VpU { vp: f.lit.vp }));
        Self::draw_mesh(ctx, &self.parts_b, vec![]);
        ctx.end_render_pass();

        // 3. bloom: bright pass then two separable blurs at quarter resolution
        let (a, b) = (self.bloom_a.as_ref().unwrap(), self.bloom_b.as_ref().unwrap());
        let quad = &self.quad;
        let full = |ctx: &mut dyn RenderingBackend, tex: TextureId| { ctx.apply_bindings(&Bindings { vertex_buffers: vec![quad.vb], index_buffer: quad.ib, images: vec![tex] }); ctx.draw(0, 6, 1); };
        ctx.begin_pass(Some(a.pass), PassAction::Nothing);
        ctx.apply_pipeline(&self.bright);
        ctx.apply_uniforms(UniformsSource::table(&Vec2U { v: [0.62, 0.95] }));
        full(ctx, scene.color);
        ctx.end_render_pass();
        for _ in 0..2 {
            ctx.begin_pass(Some(b.pass), PassAction::Nothing);
            ctx.apply_pipeline(&self.blur);
            ctx.apply_uniforms(UniformsSource::table(&Vec2U { v: [1.0 / a.w as f32, 0.0] }));
            full(ctx, a.color);
            ctx.end_render_pass();
            ctx.begin_pass(Some(a.pass), PassAction::Nothing);
            ctx.apply_pipeline(&self.blur);
            ctx.apply_uniforms(UniformsSource::table(&Vec2U { v: [0.0, 1.0 / a.h as f32] }));
            full(ctx, b.color);
            ctx.end_render_pass();
        }

        // 4. tilt-shift + bloom + grade to the screen
        ctx.begin_default_pass(PassAction::Nothing);
        ctx.apply_pipeline(&self.comp);
        ctx.apply_uniforms(UniformsSource::table(&CompU { res: [w as f32, h as f32], tilt: [0.52, 0.16, 3.2, f.bloom], grade: [f.warm, 0.0] }));
        ctx.apply_bindings(&Bindings { vertex_buffers: vec![quad.vb], index_buffer: quad.ib, images: vec![scene.color, a.color] });
        ctx.draw(0, 6, 1);
        ctx.end_render_pass();
    }
}
