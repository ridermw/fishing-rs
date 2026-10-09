//! Screen-space UI drawn with macroquad on top of the 3D frame.
use crate::{fishing::RODS, FState, Game};
use macroquad::prelude::*;

const PANEL: Color = Color::new(0.11, 0.086, 0.133, 0.88);
const EDGE: Color = Color::new(0.91, 0.82, 0.63, 1.0);
const DARK: Color = Color::new(0.10, 0.07, 0.125, 1.0);
const PAPER: Color = Color::new(0.965, 0.925, 0.824, 1.0);
const INK: Color = Color::new(0.165, 0.114, 0.078, 1.0);
const GOLD: Color = Color::new(0.95, 0.79, 0.30, 1.0);
const MUTED: Color = Color::new(0.80, 0.74, 0.61, 1.0);

fn sc() -> f32 { (screen_height() / 720.0).clamp(0.8, 1.6) }
fn fs(px: f32) -> u16 { (px * sc()).round() as u16 }
fn tw(t: &str, size: u16) -> f32 { measure_text(t, None, size, 1.0).width }

fn panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x + 3.0, y + 6.0, w, h, Color::new(0., 0., 0., 0.35));
    draw_rectangle(x - 3.0, y - 3.0, w + 6.0, h + 6.0, DARK);
    draw_rectangle(x, y, w, h, PANEL);
    draw_rectangle_lines(x, y, w, h, 3.0, EDGE);
}
fn text(t: &str, x: f32, y: f32, size: u16, c: Color) { draw_text(t, x.round(), y.round(), size as f32, c); }
fn text_c(t: &str, cx: f32, y: f32, size: u16, c: Color) { text(t, cx - tw(t, size) / 2.0, y, size, c); }
fn outlined(t: &str, cx: f32, y: f32, size: u16, c: Color, o: Color, w: f32) {
    let x = cx - tw(t, size) / 2.0;
    for (dx, dy) in [(-w, 0.), (w, 0.), (0., -w), (0., w), (-w, -w), (w, w), (-w, w), (w, -w)] { text(t, x + dx, y + dy, size, o); }
    text(t, x, y, size, c);
}
fn wrap(t: &str, max_w: f32, size: u16) -> Vec<String> {
    let mut lines = vec![]; let mut cur = String::new();
    for word in t.split_whitespace() {
        let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if tw(&trial, size) > max_w && !cur.is_empty() { lines.push(std::mem::replace(&mut cur, word.to_string())); } else { cur = trial; }
    }
    if !cur.is_empty() { lines.push(cur); }
    lines
}
fn bar(x: f32, y: f32, w: f32, h: f32, v: f32, c: Color) {
    draw_rectangle(x - 2.0, y - 2.0, w + 4.0, h + 4.0, BLACK);
    draw_rectangle(x, y, w, h, Color::new(0.07, 0.05, 0.09, 1.0));
    draw_rectangle(x, y, w * v.clamp(0.0, 1.0), h, c);
}
fn clicked() -> Option<Vec2> {
    if is_mouse_button_pressed(MouseButton::Left) { return Some(mouse_position().into()); }
    touches().into_iter().find(|t| t.phase == TouchPhase::Started).map(|t| t.position)
}
fn inside(p: Vec2, r: Rect) -> bool { r.contains(p) }

impl Game {
    fn buttons(&self) -> [(Rect, &'static str); 3] {
        let s = sc(); let size = fs(16.0);
        let labels = ["Journal", if self.audio.muted { "Sound: off" } else { "Sound: on" }, "?"];
        let mut x = screen_width() - 14.0 * s; let y = 14.0 * s; let h = 30.0 * s;
        let mut out = [(Rect::default(), ""); 3];
        for k in (0..3).rev() { let w = tw(labels[k], size) + 22.0 * s; x -= w; out[k] = (Rect::new(x, y, w, h), labels[k]); x -= 10.0 * s; }
        out
    }

    /// True when a screen point lands on an interactive UI element (keeps it from starting the touch stick).
    pub fn ui_hit(&self, p: Vec2) -> bool {
        self.buttons().iter().any(|(r, _)| inside(p, *r)) || self.dialog.is_some() || self.card.is_some() || self.any_panel()
    }

    pub fn draw_ui(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        let s = sc();
        let click = clicked();
        let mut consumed = false;

        // HUD
        let h = self.save.hour; let (hh, mm) = (h as u32, ((h.fract() * 60.0) as u32 / 10) * 10);
        let clock = format!("Day {}  {:02}:{:02}", self.save.day, hh, mm);
        let row = format!("{}   Bag {}   {}", self.save.coins, self.save.bag.len(), RODS[self.save.rod].name);
        let (bx, by) = (14.0 * s, 14.0 * s);
        let bw = tw(&clock, fs(20.0)).max(tw(&row, fs(16.0)) + 18.0 * s) + 52.0 * s;
        panel(bx, by, bw, 58.0 * s);
        let icon = vec2(bx + 18.0 * s, by + 19.0 * s);
        if self.env_night > 0.5 { draw_circle(icon.x, icon.y, 7.0 * s, Color::new(0.93, 0.92, 0.78, 1.0)); draw_circle(icon.x + 3.5 * s, icon.y - 2.5 * s, 6.0 * s, PANEL); }
        else { draw_circle(icon.x, icon.y, 6.0 * s, GOLD); for k in 0..8 { let a = k as f32 * 0.785; draw_line(icon.x + a.cos() * 8.0 * s, icon.y + a.sin() * 8.0 * s, icon.x + a.cos() * 10.5 * s, icon.y + a.sin() * 10.5 * s, 2.0, GOLD); } }
        text(&clock, bx + 34.0 * s, by + 26.0 * s, fs(20.0), WHITE);
        let c = vec2(bx + 18.0 * s, by + 42.0 * s);
        draw_poly(c.x, c.y, 4, 6.0 * s, 0.0, GOLD);
        text(&row, bx + 32.0 * s, by + 48.0 * s, fs(16.0), MUTED);

        // buttons
        for (k, (r, label)) in self.buttons().iter().enumerate() {
            panel(r.x, r.y, r.w, r.h);
            text_c(label, r.x + r.w / 2.0, r.y + r.h * 0.7, fs(16.0), WHITE);
            if let Some(p) = click { if inside(p, *r) {
                consumed = true;
                match k { 0 => if self.f.state == FState::Idle { self.journal = !self.journal; self.help = false; }, 1 => { self.audio.toggle(); }, _ => { self.help = !self.help; self.journal = false; } }
            } }
        }

        // prompt bubble over the player's head
        let head = self.to_screen(self.actors[0].pos + vec3(0.0, 2.0, 0.0));
        if let Some(p) = &self.prompt {
            if self.started {
                let key = if self.touch.used { "A" } else if p == "Cast" { "Hold Space" } else { "Space" };
                let size = fs(16.0);
                let (kw, pw) = (tw(key, size) + 10.0 * s, tw(p, size));
                let (w, hgt) = (kw + pw + 24.0 * s, 28.0 * s);
                let (x, y) = (head.x - w / 2.0, head.y - hgt);
                draw_rectangle(x + 2.0, y + 5.0, w, hgt, Color::new(0., 0., 0., 0.3));
                draw_rectangle(x, y, w, hgt, PAPER); draw_rectangle_lines(x, y, w, hgt, 3.0, INK);
                draw_rectangle(x + 8.0 * s, y + 5.0 * s, kw, hgt - 10.0 * s, INK);
                text(key, x + 13.0 * s, y + hgt * 0.68, size, PAPER);
                text(p, x + kw + 15.0 * s, y + hgt * 0.68, size, INK);
            }
        }
        if self.f.state == FState::Bite {
            let bounce = ((get_time() * 8.0) as i32 % 2) as f32 * 6.0 * s;
            outlined("!", head.x, head.y - 4.0 * s - bounce, fs(64.0), Color::new(1.0, 0.95, 0.35, 1.0), Color::new(0.66, 0.19, 0.10, 1.0), 3.0 * s);
        }

        // toast
        if self.toast.1 > 0.0 {
            let size = fs(18.0); let w = tw(&self.toast.0, size) + 32.0 * s;
            let a = self.toast.1.min(0.25) / 0.25;
            let y = 84.0 * s - (1.0 - a) * 20.0;
            panel(sw / 2.0 - w / 2.0, y, w, 36.0 * s);
            text_c(&self.toast.0, sw / 2.0, y + 24.0 * s, size, WHITE);
        }

        // power meter
        if self.f.state == FState::Charge {
            let (w, x, y) = (260.0 * s, sw / 2.0 - 130.0 * s, sh - 150.0 * s);
            panel(x - 12.0 * s, y - 26.0 * s, w + 24.0 * s, 54.0 * s);
            text("POWER", x, y - 6.0 * s, fs(14.0), EDGE);
            let p = self.f.power;
            let c = if p < 0.6 { Color::new(0.36, 0.72, 0.36, 1.) } else if p < 0.85 { GOLD } else { Color::new(0.91, 0.35, 0.18, 1.) };
            bar(x, y, w, 16.0 * s, p, c);
        }

        // reel panel
        if self.f.state == FState::Reel {
            let w = (460.0 * s).min(sw * 0.92); let (x, y, hgt) = (sw / 2.0 - w / 2.0, sh - 160.0 * s, 132.0 * s);
            panel(x, y, w, hgt);
            let jitter = if self.f.run { ((get_time() * 16.0) as i32 % 2) as f32 * 4.0 - 2.0 } else { 0.0 };
            let (msg, col) = if self.f.run { ("It's running! Ease off!", Color::new(1.0, 0.48, 0.35, 1.)) } else { ("Calm - reel it in!", Color::new(0.62, 0.94, 0.62, 1.)) };
            text_c(msg, sw / 2.0 + jitter, y + 30.0 * s, fs(20.0), col);
            let t = self.f.tension / self.rod().max_t;
            let tc = if t > 0.8 { if (get_time() * 10.0) as i32 % 2 == 0 { Color::new(0.91, 0.25, 0.18, 1.) } else { Color::new(1.0, 0.5, 0.4, 1.) } } else if t > 0.55 { GOLD } else { Color::new(0.36, 0.72, 0.36, 1.) };
            let (lx, bw) = (x + 90.0 * s, w - 110.0 * s);
            text("Tension", x + 16.0 * s, y + 60.0 * s, fs(14.0), EDGE);
            bar(lx, y + 48.0 * s, bw, 16.0 * s, t, tc);
            draw_rectangle(lx + bw * 0.8, y + 45.0 * s, 2.0, 22.0 * s, Color::new(0.91, 0.25, 0.18, 1.));
            text("Catch", x + 16.0 * s, y + 88.0 * s, fs(14.0), EDGE);
            bar(lx, y + 76.0 * s, bw, 16.0 * s, self.f.progress, Color::new(0.30, 0.66, 0.88, 1.));
            let hint = if self.touch.used { "Hold A to reel - let go when it runs" } else { "Hold Space to reel - let go when it runs" };
            text_c(hint, sw / 2.0, y + 118.0 * s, fs(13.0), MUTED);
        }

        // catch card
        if let Some(card) = &self.card {
            let sp = &self.species[card.fish];
            let (w, hgt) = (300.0 * s, 250.0 * s); let (x, y) = (sw / 2.0 - w / 2.0, sh * 0.46 - hgt / 2.0);
            panel(x, y, w, hgt);
            text_c("You caught a", sw / 2.0, y + 26.0 * s, fs(15.0), EDGE);
            let iw = 192.0 * s;
            draw_texture_ex(&self.icons[card.fish], sw / 2.0 - iw / 2.0, y + 36.0 * s, WHITE, DrawTextureParams { dest_size: Some(vec2(iw, iw / 2.0)), ..Default::default() });
            text_c(sp.name, sw / 2.0, y + 160.0 * s, fs(26.0), WHITE);
            let rc = match sp.rarity() { "Uncommon" => Color::new(0.48, 0.85, 0.48, 1.), "Rare" => Color::new(0.48, 0.72, 1.0, 1.), "Legendary" => GOLD, _ => Color::new(0.72, 0.78, 0.85, 1.) };
            text_c(&sp.rarity().to_uppercase(), sw / 2.0, y + 184.0 * s, fs(14.0), rc);
            text_c(&format!("{} cm  -  {} coins", card.size, card.value), sw / 2.0, y + 210.0 * s, fs(17.0), MUTED);
            text_c(if self.touch.used { "Tap to continue" } else { "Press Space" }, sw / 2.0, y + 236.0 * s, fs(13.0), MUTED);
            if card.new {
                let (nx, ny) = (x + w - 34.0 * s, y - 14.0 * s);
                draw_rectangle(nx - 3.0, ny - 3.0, 62.0 * s, 30.0 * s, WHITE);
                draw_rectangle(nx, ny, 62.0 * s - 6.0, 30.0 * s - 6.0, Color::new(0.91, 0.25, 0.18, 1.));
                text("NEW!", nx + 6.0 * s, ny + 19.0 * s, fs(17.0), WHITE);
            }
            if let Some(p) = click { if !consumed && inside(p, Rect::new(x, y, w, hgt)) { self.act.ui_press = true; consumed = true; } }
        }

        // dialog
        if self.dialog.is_some() {
            let w = (640.0 * s).min(sw * 0.94); let x = sw / 2.0 - w / 2.0;
            let size = fs(19.0);
            let d = self.dialog.as_ref().unwrap();
            let lines = wrap(&d.lines[d.line], w - 36.0 * s, size);
            let last = d.line + 1 == d.lines.len();
            let nchoice = if last { d.choices.len() } else { 0 };
            let hgt = 30.0 * s + lines.len() as f32 * 26.0 * s + nchoice as f32 * 34.0 * s + 22.0 * s;
            let y = sh - hgt - 24.0 * s;
            panel(x, y, w, hgt);
            let nw = tw(&d.name, fs(17.0)) + 20.0 * s;
            draw_rectangle(x + 16.0 * s, y - 18.0 * s, nw, 28.0 * s, PAPER); draw_rectangle_lines(x + 16.0 * s, y - 18.0 * s, nw, 28.0 * s, 3.0, INK);
            text(&d.name, x + 26.0 * s, y + 2.0 * s, fs(17.0), INK);
            for (k, l) in lines.iter().enumerate() { text(l, x + 18.0 * s, y + 38.0 * s + k as f32 * 26.0 * s, size, WHITE); }
            let mut pick = None;
            let cy0 = y + 38.0 * s + lines.len() as f32 * 26.0 * s;
            for k in 0..nchoice {
                let c = &d.choices[k];
                let r = Rect::new(x + 14.0 * s, cy0 + k as f32 * 34.0 * s - 4.0 * s, w - 28.0 * s, 30.0 * s);
                if k == d.sel { draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.91, 0.82, 0.63, 0.12)); draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, EDGE);
                    draw_triangle(vec2(r.x + 10.0 * s, r.y + 8.0 * s), vec2(r.x + 10.0 * s, r.y + 22.0 * s), vec2(r.x + 20.0 * s, r.y + 15.0 * s), GOLD); }
                text(&c.label, r.x + 28.0 * s, r.y + 21.0 * s, fs(17.0), if c.disabled { Color::new(1., 1., 1., 0.45) } else { WHITE });
                if let Some(p) = click { if inside(p, r) { pick = Some(k); } }
            }
            if nchoice == 0 && (get_time() * 2.0) as i32 % 2 == 0 {
                let (tx, ty) = (x + w - 22.0 * s, y + hgt - 16.0 * s);
                draw_triangle(vec2(tx - 7.0 * s, ty - 5.0 * s), vec2(tx + 7.0 * s, ty - 5.0 * s), vec2(tx, ty + 4.0 * s), GOLD);
            }
            if let Some(p) = click {
                if !consumed && inside(p, Rect::new(x, y - 18.0 * s, w, hgt + 18.0 * s)) {
                    consumed = true;
                    if let Some(k) = pick { self.dialog.as_mut().unwrap().sel = k; self.advance_dialog(); }
                    else if nchoice == 0 { self.advance_dialog(); }
                }
            }
        }

        // journal
        if self.journal {
            let w = (860.0 * s).min(sw * 0.94); let hgt = (sh * 0.86).min(640.0 * s);
            let (x, y) = (sw / 2.0 - w / 2.0, sh / 2.0 - hgt / 2.0);
            panel(x, y, w, hgt);
            text("Angler's Journal", x + 22.0 * s, y + 36.0 * s, fs(26.0), GOLD);
            let caught = self.species.iter().filter(|sp| self.save.entry(sp.id()).is_some()).count();
            text(&format!("{} / {} species  |  {} caught  |  {} coins earned", caught, self.species.len(), self.save.total, self.save.earned), x + 22.0 * s, y + 60.0 * s, fs(15.0), MUTED);
            let cols = ((w - 44.0 * s) / (190.0 * s)).floor().max(1.0) as usize;
            let cw = (w - 44.0 * s - (cols - 1) as f32 * 10.0 * s) / cols as f32;
            let ch = ((hgt - 90.0 * s) / ((self.species.len() + cols - 1) / cols) as f32 - 10.0 * s).min(130.0 * s);
            for (k, sp) in self.species.iter().enumerate() {
                let (cx, cy) = (x + 22.0 * s + (k % cols) as f32 * (cw + 10.0 * s), y + 78.0 * s + (k / cols) as f32 * (ch + 10.0 * s));
                draw_rectangle(cx, cy, cw, ch, Color::new(1., 1., 1., 0.05));
                draw_rectangle_lines(cx, cy, cw, ch, 2.0, Color::new(0.91, 0.82, 0.63, 0.35));
                let e = self.save.entry(sp.id());
                let iw = (96.0 * s).min(ch * 0.8);
                let tex = if e.is_some() { &self.icons[k] } else { &self.silhouettes[k] };
                draw_texture_ex(tex, cx + cw / 2.0 - iw / 2.0, cy + 4.0 * s, WHITE, DrawTextureParams { dest_size: Some(vec2(iw, iw / 2.0)), ..Default::default() });
                let ny = cy + iw / 2.0 + 20.0 * s;
                text_c(if e.is_some() { sp.name } else { "???" }, cx + cw / 2.0, ny, fs(16.0), if e.is_some() { WHITE } else { Color::new(0.54, 0.5, 0.44, 1.) });
                for (li, l) in wrap(sp.hint, cw - 12.0 * s, fs(12.0)).iter().take(2).enumerate() { text_c(l, cx + cw / 2.0, ny + 16.0 * s + li as f32 * 13.0 * s, fs(12.0), Color::new(0.72, 0.67, 0.54, 1.)); }
                if let Some(e) = e { text_c(&format!("x{} - best {} cm", e.n, e.best), cx + cw / 2.0, cy + ch - 8.0 * s, fs(13.0), Color::new(0.62, 0.94, 0.62, 1.)); }
            }
            let close = Rect::new(x + w - 36.0 * s, y + 8.0 * s, 28.0 * s, 28.0 * s);
            text("X", close.x + 8.0 * s, close.y + 20.0 * s, fs(20.0), EDGE);
            if let Some(p) = click { if !consumed && (inside(p, close) || !inside(p, Rect::new(x, y, w, hgt))) { self.journal = false; consumed = true; } }
        }

        // help
        if self.help {
            let lines = [
                "WASD / Arrows: walk  -  Shift: run",
                "Face the water and HOLD Space to wind up. Let go to cast.",
                "Wait for the !, then press Space right away to hook the fish.",
                "Hold Space to reel while the fish is calm. When it RUNS, let go so your line doesn't snap.",
                "Fish change with the time of day, the sea or pond, and how deep the water is.",
                "Sell your catch to Marla at the market and buy better rods.",
                "J: journal  -  M: sound  -  H: help",
            ];
            let w = (760.0 * s).min(sw * 0.94);
            let wrapped: Vec<Vec<String>> = lines.iter().map(|l| wrap(l, w - 70.0 * s, fs(17.0))).collect();
            let n: usize = wrapped.iter().map(|l| l.len()).sum();
            let hgt = 110.0 * s + n as f32 * 26.0 * s;
            let (x, y) = (sw / 2.0 - w / 2.0, sh / 2.0 - hgt / 2.0);
            panel(x, y, w, hgt);
            text("How to fish", x + 22.0 * s, y + 36.0 * s, fs(26.0), GOLD);
            let mut ly = y + 70.0 * s;
            for l in &wrapped { draw_rectangle(x + 26.0 * s, ly - 9.0 * s, 6.0 * s, 6.0 * s, EDGE); for seg in l { text(seg, x + 44.0 * s, ly, fs(17.0), WHITE); ly += 26.0 * s; } }
            text("Built in Rust. Every texture, sprite and sound is generated in code.", x + 22.0 * s, y + hgt - 18.0 * s, fs(13.0), Color::new(0.72, 0.67, 0.54, 1.));
            if let Some(p) = click { if !consumed && !self.buttons().iter().any(|(r, _)| inside(p, *r)) { self.help = false; consumed = true; } }
        }

        // touch controls
        if self.touch.used && self.started {
            if let Some((_, o, d)) = self.touch.stick {
                draw_circle(o.x, o.y, 55.0, Color::new(0., 0., 0., 0.2)); draw_circle_lines(o.x, o.y, 55.0, 3.0, Color::new(1., 1., 1., 0.4));
                draw_circle(o.x + d.x * 50.0, o.y + d.y * 50.0, 22.0, Color::new(1., 1., 1., 0.55));
            }
            let (ax, ay) = (sw - 70.0, sh - 80.0);
            let down = self.touch.a_id.is_some();
            draw_circle(ax, ay + 6.0, 42.0, Color::new(0.48, 0.10, 0.06, 1.));
            draw_circle(ax, ay + if down { 4.0 } else { 0.0 }, 42.0, Color::new(0.91, 0.25, 0.18, 1.));
            draw_circle_lines(ax, ay + if down { 4.0 } else { 0.0 }, 42.0, 4.0, PAPER);
            text_c("A", ax, ay + 11.0 + if down { 4.0 } else { 0.0 }, 32, WHITE);
        }

        // title
        if self.title_fade > 0.0 {
            let a = self.title_fade;
            draw_rectangle(0., 0., sw, sh, Color::new(0.04, 0.03, 0.08, 0.4 * a));
            let big = fs(96.0).min((sw / 9.0) as u16);
            outlined("a little HD-2D fishing game", sw / 2.0, sh * 0.40, fs(22.0), Color::new(GOLD.r, GOLD.g, GOLD.b, a), Color::new(0., 0., 0., a), 2.0);
            outlined("Driftmoor Cove", sw / 2.0, sh * 0.40 + big as f32 * 0.95, big, Color::new(1.0, 0.96, 0.85, a), Color::new(0.23, 0.14, 0.08, a), 4.0 * s);
            if (get_time() * 1.6) as i32 % 2 == 0 || self.started {
                outlined("Press any key or tap to begin", sw / 2.0, sh * 0.40 + big as f32 * 1.6, fs(20.0), Color::new(1., 1., 1., a), Color::new(0., 0., 0., a), 2.0);
            }
            text_c("Rust + WebAssembly edition", sw / 2.0, sh - 24.0 * s, fs(15.0), Color::new(0.85, 0.8, 0.7, a));
        }
        let _ = consumed;
    }
}
