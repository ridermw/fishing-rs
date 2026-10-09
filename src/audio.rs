//! Every sound is synthesized into in-memory WAV buffers at startup. No audio files.
use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, PlaySoundParams, Sound};

const SR: f32 = 22050.0;

struct Buf(Vec<f32>);
impl Buf {
    fn new(sec: f32) -> Self { Buf(vec![0.0; (sec * SR) as usize]) }
    fn tone(&mut self, at: f32, f0: f32, dur: f32, wave: u8, vol: f32, slide: f32) {
        let (s0, n) = ((at * SR) as usize, (dur * SR) as usize);
        let f1 = (f0 + slide).max(30.0);
        let mut ph = 0.0f32;
        for i in 0..n {
            let t = i as f32 / n as f32;
            let f = if slide != 0.0 { f0 * (f1 / f0).powf(t) } else { f0 };
            ph = (ph + f / SR).fract();
            let v = match wave {
                0 => (ph * std::f32::consts::TAU).sin(),
                1 => if ph < 0.5 { 1.0 } else { -1.0 },
                2 => 1.0 - 4.0 * (ph - 0.5).abs(),
                _ => ph * 2.0 - 1.0,
            };
            let g = vol * (0.0008f32 / vol).powf(t);
            if let Some(o) = self.0.get_mut(s0 + i) { *o += v * g; }
        }
    }
    /// Band-passed noise burst with exponential decay.
    fn burst(&mut self, at: f32, dur: f32, freq: f32, vol: f32, q: f32, seed: &mut u32) {
        let (s0, n) = ((at * SR) as usize, (dur * SR) as usize);
        let w0 = std::f32::consts::TAU * (freq / SR).min(0.45);
        let alpha = w0.sin() / (2.0 * q);
        let (b0, b2, a0, a1, a2) = (alpha, -alpha, 1.0 + alpha, -2.0 * w0.cos(), 1.0 - alpha);
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        for i in 0..n {
            *seed ^= *seed << 13; *seed ^= *seed >> 17; *seed ^= *seed << 5;
            let x = *seed as f32 / 2147483648.0 - 1.0;
            let y = (b0 * x + b2 * x2 - a1 * y1 - a2 * y2) / a0;
            x2 = x1; x1 = x; y2 = y1; y1 = y;
            let g = vol * (0.001f32 / vol).powf(i as f32 / n as f32);
            if let Some(o) = self.0.get_mut(s0 + i) { *o += y * g * 2.0; }
        }
    }
    fn wav(&self) -> Vec<u8> {
        let n = self.0.len() as u32;
        let mut b = Vec::with_capacity(44 + n as usize * 2);
        let w32 = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
        b.extend_from_slice(b"RIFF"); w32(&mut b, 36 + n * 2); b.extend_from_slice(b"WAVEfmt ");
        w32(&mut b, 16); b.extend_from_slice(&1u16.to_le_bytes()); b.extend_from_slice(&1u16.to_le_bytes());
        w32(&mut b, SR as u32); w32(&mut b, SR as u32 * 2); b.extend_from_slice(&2u16.to_le_bytes()); b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data"); w32(&mut b, n * 2);
        for s in &self.0 { b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes()); }
        b
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sfx { Splash, Plop, Cast, Bite, Nibble, Tick, Strain, Snap, Lose, Catch, Coin, Blip, Bird, Cricket }

pub struct Audio { sounds: Vec<(Sfx, Sound)>, ambience: Option<Sound>, pub muted: bool, started: bool, bird_t: f32, tick: usize }

async fn make(sfx: Sfx, seed: &mut u32, variant: usize) -> Option<(Sfx, Sound)> {
    use Sfx::*;
    let mut b = Buf::new(match sfx { Catch => 0.9, Snap | Lose | Splash => 0.6, Bird => 0.6, _ => 0.4 });
    match sfx {
        Splash => { b.burst(0.0, 0.45, 900.0, 0.35, 0.8, seed); b.burst(0.0, 0.2, 2400.0, 0.12, 1.0, seed); }
        Plop => { b.tone(0.0, 420.0, 0.12, 0, 0.15, -250.0); b.burst(0.0, 0.12, 1600.0, 0.08, 1.0, seed); }
        Cast => { b.burst(0.0, 0.25, 3000.0, 0.08, 3.0, seed); b.tone(0.0, 900.0, 0.2, 0, 0.05, -500.0); }
        Bite => { b.tone(0.0, 880.0, 0.08, 1, 0.1, 0.0); b.tone(0.08, 1320.0, 0.12, 1, 0.1, 0.0); }
        Nibble => b.tone(0.0, 520.0, 0.05, 0, 0.06, 0.0),
        Tick => b.tone(0.0, 1800.0 + variant as f32 * 120.0, 0.025, 1, 0.03, 0.0),
        Strain => b.tone(0.0, 140.0, 0.12, 3, 0.03, 40.0),
        Snap => { b.tone(0.0, 700.0, 0.35, 3, 0.1, -600.0); b.burst(0.0, 0.1, 4000.0, 0.15, 1.0, seed); }
        Lose => { b.tone(0.0, 392.0, 0.15, 2, 0.12, 0.0); b.tone(0.15, 311.0, 0.3, 2, 0.12, 0.0); }
        Catch => { for (k, f) in [523.0, 659.0, 784.0, 1046.0].iter().enumerate() { b.tone(k as f32 * 0.09, *f, 0.18, 2, 0.13, 0.0); } b.tone(0.36, 1318.0, 0.4, 2, 0.1, 0.0); }
        Coin => { b.tone(0.0, 988.0, 0.08, 1, 0.08, 0.0); b.tone(0.08, 1318.0, 0.25, 1, 0.08, 0.0); }
        Blip => b.tone(0.0, 660.0, 0.05, 1, 0.05, 0.0),
        Bird => {
            let base = 2200.0 + variant as f32 * 350.0;
            for k in 0..(2 + variant % 3) { b.tone(k as f32 * 0.11, base, 0.07, 0, 0.025, if (variant + k) % 2 == 0 { 600.0 } else { -500.0 }); }
        }
        Cricket => { b.tone(0.0, 4200.0, 0.04, 0, 0.012, 0.0); b.tone(0.09, 4200.0, 0.04, 0, 0.012, 0.0); }
    }
    load_sound_from_bytes(&b.wav()).await.ok().map(|s| (sfx, s))
}

impl Audio {
    pub async fn new() -> Audio {
        use Sfx::*;
        let mut seed = 0x9e3779b9u32;
        let mut sounds = vec![];
        for (s, n) in [(Splash, 1), (Plop, 1), (Cast, 1), (Bite, 1), (Nibble, 1), (Tick, 3), (Strain, 1), (Snap, 1), (Lose, 1), (Catch, 1), (Coin, 1), (Blip, 1), (Bird, 4), (Cricket, 1)] {
            for v in 0..n { if let Some(x) = make(s, &mut seed, v).await { sounds.push(x); } }
        }
        // ocean: low-passed noise, cross-faded so it loops seamlessly
        let mut amb = Buf::new(5.0);
        let mut lp = 0.0f32;
        for o in amb.0.iter_mut() {
            seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
            lp += 0.14 * (seed as f32 / 2147483648.0 - 1.0 - lp);
            *o = lp * 2.4;
        }
        let fade = SR as usize;
        let n = amb.0.len() - fade;
        for i in 0..fade { let t = i as f32 / fade as f32; amb.0[i] = amb.0[i] * t + amb.0[n + i] * (1.0 - t); }
        amb.0.truncate(n);
        let ambience = load_sound_from_bytes(&amb.wav()).await.ok();
        Audio { sounds, ambience, muted: false, started: false, bird_t: 3.0, tick: 0 }
    }

    pub fn play(&mut self, s: Sfx) {
        if self.muted { return; }
        let variants: Vec<&Sound> = self.sounds.iter().filter(|(k, _)| *k == s).map(|(_, v)| v).collect();
        if variants.is_empty() { return; }
        self.tick = self.tick.wrapping_add(1);
        play_sound(variants[self.tick % variants.len()], PlaySoundParams { looped: false, volume: 0.55 });
    }

    pub fn start(&mut self) {
        if self.started { return; }
        self.started = true;
        if let Some(a) = &self.ambience { play_sound(a, PlaySoundParams { looped: true, volume: if self.muted { 0.0 } else { 0.1 } }); }
    }

    pub fn toggle(&mut self) -> bool { self.muted = !self.muted; self.muted }

    /// Swells the ocean and chirps birds by day, crickets by night.
    pub fn update(&mut self, dt: f32, time: f32, day: f32) {
        if let Some(a) = &self.ambience { set_sound_volume(a, if self.muted { 0.0 } else { 0.55 * (0.05 + 0.035 * (time * 0.69).sin()) * 2.0 }); }
        if self.muted || !self.started { return; }
        self.bird_t -= dt;
        if self.bird_t <= 0.0 {
            self.bird_t = 2.0 + macroquad::rand::gen_range(0.0, 6.0);
            let r = macroquad::rand::gen_range(0.0, 1.0);
            if day > 0.5 && r < 0.7 { self.play(Sfx::Bird) } else if day < 0.3 && r < 0.5 { self.play(Sfx::Cricket) }
        }
    }
}
