//! Simulação do jogo, independente do motor gráfico. Coordenadas do Flash: x para a direita, y para baixo;
//! a posição do lutador é o ponto entre os pés. Passo fixo de 60 Hz.
//!
//! Unidades do motor original: a posição avança `vel * dt` por tick (dt = 0,384) e acelerações, atritos e
//! gravidade também são multiplicados por dt. O movimento segue o tick de física do jogo (`_-F11._-I30`):
//! atrito sempre aplicado, depois aceleração; gravidade 3,75 mais um termo extra perto do ápice; o corpo é uma
//! linha vertical de 120 px (pés e cabeça) varrida contra superfícies de um lado só; no chão o lutador anda
//! ao longo da superfície; paredes permitem agarrar, deslizar e pular.
//! Ver `docs/bytecode-notes.md` para a origem de cada constante.

pub mod bot;
pub mod combat;
pub mod geometry;
pub mod world;

use bh_assets::stats::SpeedStat;

pub use geometry::{Geometry, Kind, Surface};

/// Altura da linha do corpo: a cabeça fica 120 px acima dos pés.
pub const BODY_HEIGHT: f32 = 120.0;
/// Recuo aplicado ao bater numa superfície (o ponto fica ligeiramente fora dela).
const BACKOFF: f32 = 1.01;
/// Sonda para baixo que mantém o lutador no chão.
const GROUND_PROBE: f32 = 2.02;

// ------------------------------------------------------------------ parâmetros

/// Constantes do motor (do bytecode).
#[derive(Debug, Clone, Copy)]
pub struct Tuning {
    /// Passo de tempo do motor: posição += vel * dt por tick.
    pub dt: f32,
    pub gravity: f32,
    /// Gravidade extra `extra_gravity - extra_slope * |vy|` enquanto |vy| <= `extra_range`.
    pub extra_gravity: f32,
    pub extra_range: f32,
    pub max_fall: f32,
    pub fast_fall: f32,
    pub fast_fall_gravity: f32,
    /// A queda rápida só começa com `vy` acima deste valor.
    pub fast_fall_min_vy: f32,
    /// Quanto a queda acima de `max_fall` decai por tick (x dt) fora da queda rápida.
    pub fall_decay: f32,
    pub jump_impulse: f32,
    pub wall_jump_impulse: f32,
    pub wall_jump_side: f32,
    /// Impulso para longe da parede ao soltá-la.
    pub wall_release: f32,
    pub air_jumps: u8,
    /// Fração da gravidade cancelada ao deslizar numa parede.
    pub wall_slide: f32,
    /// Ticks sem gravidade ao agarrar uma parede.
    pub wall_cling_ticks: u32,
    /// Elasticidade do quique em hitstun e velocidade mínima para quicar.
    pub bounce: f32,
    pub bounce_min_speed: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            dt: 0.384,
            gravity: 3.75,
            extra_gravity: 3.25,
            extra_range: 48.0,
            max_fall: 70.0,
            fast_fall: 85.0,
            fast_fall_gravity: 12.0,
            fast_fall_min_vy: -38.0,
            fall_decay: 6.0,
            jump_impulse: 57.0,
            wall_jump_impulse: 53.0,
            wall_jump_side: 48.0,
            wall_release: 30.0,
            air_jumps: 2,
            wall_slide: 0.85,
            wall_cling_ticks: 7,
            bounce: 0.8,
            bounce_min_speed: 25.0,
        }
    }
}

impl Tuning {
    /// Termo extra de gravidade em função da velocidade vertical (`_-F11._-T4w`).
    pub fn extra(&self, vy: f32) -> f32 {
        if vy.abs() > self.extra_range {
            0.0
        } else {
            self.extra_gravity - (self.extra_gravity / self.extra_range) * vy.abs()
        }
    }
}

/// Atributos do herói, nas unidades do motor.
#[derive(Debug, Clone, Copy)]
pub struct Params {
    pub run_speed: f32,
    pub accel: f32,
    pub friction: f32,
    pub air_speed: f32,
    pub air_accel: f32,
    pub air_friction: f32,
    pub jump_x_impulse: f32,
    pub anim_speed: f32,
    pub t: Tuning,
}

impl Params {
    pub fn new(s: &SpeedStat, t: Tuning) -> Self {
        Params {
            run_speed: s.run_speed,
            accel: s.acceleration,
            friction: s.friction,
            air_speed: s.air_run_speed,
            air_accel: s.air_acceleration,
            air_friction: s.air_friction,
            jump_x_impulse: s.jump_x_impulse,
            anim_speed: s.anim_speed,
            t,
        }
    }
}

// ------------------------------------------------------------------ lutador

#[derive(Debug, Clone, Copy, Default)]
pub struct Input {
    /// -1 (esquerda) .. 1 (direita)
    pub move_x: f32,
    /// Borda de subida: o botão de pulo acabou de ser apertado.
    pub jump: bool,
    /// Botão para baixo segurado (queda rápida / atravessar plataforma).
    pub down: bool,
}

/// O que o estado atual (golpe, esquiva...) permite ou altera no movimento deste tick.
#[derive(Debug, Clone, Copy)]
pub struct StepMods {
    /// O lutador pode acelerar com a direção.
    pub allow_move: bool,
    pub accel_mult: f32,
    pub allow_jump: bool,
    /// Suspende a gravidade (`CancelGravity`/`AntigravTime` do poder, esquiva).
    pub cancel_gravity: bool,
    /// Usa o atrito do chão mesmo no ar (`KeepGroundFriction`).
    pub keep_ground_friction: bool,
    /// Sem atrito neste tick (esquiva/dash controlam a velocidade diretamente).
    pub no_friction: bool,
    /// Há um poder em execução (trava a queda rápida e o atravessar plataformas).
    pub in_power: bool,
}

impl Default for StepMods {
    fn default() -> Self {
        StepMods { allow_move: true, accel_mult: 1.0, allow_jump: true, cancel_gravity: false, keep_ground_friction: false, no_friction: false, in_power: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Run,
    Rising,
    Falling,
    WallCling,
}

/// O que aconteceu no tick (para animação/som).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StepEvents {
    pub jumped: bool,
    pub wall_jumped: bool,
    pub landed: bool,
    pub bounced: bool,
}

#[derive(Debug, Clone)]
pub struct Fighter {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// 1 = olhando para a direita, -1 = esquerda.
    pub facing: i8,
    /// Superfície em que está em pé.
    pub surface: Option<usize>,
    /// Parede em contato: 0 nenhuma, 1 à esquerda, 2 à direita.
    pub wall: u8,
    pub air_jumps_used: u8,
    /// Estado de queda rápida.
    pub fast_fall: bool,
    /// Em hitstun: sem controle, atrito 4.0 e quica nas superfícies.
    pub stunned: bool,
    /// Ticks restantes sem gravidade.
    pub antigrav: u32,
}

fn len(v: (f32, f32)) -> f32 {
    (v.0 * v.0 + v.1 * v.1).sqrt()
}

fn toward(v: f32, target: f32, delta: f32) -> f32 {
    if v < target { (v + delta).min(target) } else { (v - delta).max(target) }
}

impl Fighter {
    pub fn new(x: f32, y: f32, _p: &Params) -> Self {
        Fighter { x, y, vx: 0.0, vy: 0.0, facing: 1, surface: None, wall: 0, air_jumps_used: 0, fast_fall: false, stunned: false, antigrav: 0 }
    }

    pub fn respawn(&mut self, x: f32, y: f32, p: &Params) {
        *self = Fighter::new(x, y, p);
    }

    pub fn grounded(&self) -> bool {
        self.surface.is_some()
    }

    pub fn air_jumps_left(&self, p: &Params) -> u8 {
        p.t.air_jumps.saturating_sub(self.air_jumps_used)
    }

    pub fn motion(&self) -> Motion {
        if self.surface.is_some() {
            if self.vx.abs() > 2.0 { Motion::Run } else { Motion::Idle }
        } else if self.wall != 0 {
            Motion::WallCling
        } else if self.vy < 0.0 {
            Motion::Rising
        } else {
            Motion::Falling
        }
    }

    pub fn step(&mut self, input: &Input, p: &Params, geo: &Geometry) -> StepEvents {
        self.step_with(input, p, geo, &StepMods::default())
    }

    pub fn step_with(&mut self, input: &Input, p: &Params, geo: &Geometry, mods: &StepMods) -> StepEvents {
        let t = &p.t;
        let dt = t.dt;
        let mut ev = StepEvents::default();
        let controllable = !self.stunned;

        // --- no chão a velocidade vertical é zerada
        if self.surface.is_some() && !self.stunned {
            self.vy = 0.0;
        }

        // --- soltar a parede: direção oposta a ela (sem pular) dá um impulso de 30 para longe
        let (left, right) = (input.move_x < -0.1, input.move_x > 0.1);
        if self.wall != 0 && controllable && !mods.in_power && !input.jump && ((self.wall == 2 && left) || (self.wall == 1 && right)) {
            self.vx = if self.wall == 2 { -t.wall_release } else { t.wall_release };
            self.facing = if self.wall == 2 { -1 } else { 1 };
            self.wall = 0;
        }

        // --- atrito (sempre; a aceleração vem depois e o supera)
        if !mods.no_friction {
            let fric = if self.stunned {
                combat::HITSTUN_FRICTION
            } else if self.surface.is_some() || mods.keep_ground_friction {
                p.friction
            } else {
                p.air_friction
            };
            self.vx = toward(self.vx, 0.0, fric * dt);
        }

        // --- ↓ segurado: plataformas finas deixam de segurar
        let drop = input.down && controllable && !mods.in_power;
        if drop {
            if let Some(si) = self.surface {
                if geo.surfaces[si].kind == Kind::Soft {
                    self.surface = None;
                }
            }
        }

        // --- aceleração
        if (left || right) && controllable && mods.allow_move {
            let airborne = self.surface.is_none();
            let top = if airborne { p.air_speed } else { p.run_speed };
            let mut acc = (if airborne { p.air_accel } else { p.accel }) * mods.accel_mult;
            if !mods.in_power {
                self.facing = if left { -1 } else { 1 };
            }
            // no ar, virar contra a própria velocidade é amortecido
            if airborne && self.vx != 0.0 && left != (self.vx < 0.0) {
                acc *= (1.0 - self.vx.abs() / 50.0).max(0.0);
            }
            if left {
                if self.vx > -top {
                    self.vx = (self.vx - acc * dt).max(-top);
                }
            } else if self.vx < top {
                self.vx = (self.vx + acc * dt).min(top);
            }
        }

        // --- pulo
        if input.jump && controllable && mods.allow_jump {
            if self.surface.is_some() {
                self.vy = -t.jump_impulse;
                self.surface = None;
                ev.jumped = true;
                // com a direção apertada: cancela velocidade contrária, ou ganha um empurrão se já está no máximo
                if left {
                    if self.vx > 0.0 {
                        self.vx = 0.0;
                    } else if self.vx <= -p.run_speed {
                        self.vx -= p.jump_x_impulse * 0.5;
                    }
                } else if right {
                    if self.vx < 0.0 {
                        self.vx = 0.0;
                    } else if self.vx >= p.run_speed {
                        self.vx += p.jump_x_impulse * 0.5;
                    }
                }
            } else if self.wall != 0 {
                // pulo de parede: sobe e é empurrado para longe da parede; não gasta pulos no ar
                let facing_left = self.facing < 0;
                self.vy = -t.wall_jump_impulse;
                self.vx = if self.wall == 1 {
                    t.wall_jump_side * if facing_left { 0.9 } else { 1.0 }
                } else {
                    -t.wall_jump_side * if facing_left { 1.0 } else { 0.9 }
                };
                self.facing = if self.wall == 1 { 1 } else { -1 };
                self.wall = 0;
                self.antigrav = 0;
                ev.jumped = true;
                ev.wall_jumped = true;
            } else if self.air_jumps_used < t.air_jumps {
                self.vy = -t.jump_impulse;
                self.air_jumps_used += 1;
                self.antigrav = 0;
                ev.jumped = true;
                if left && self.vx > p.air_speed {
                    self.vx = p.air_speed;
                } else if right && self.vx < -p.air_speed {
                    self.vx = -p.air_speed;
                }
            }
        }

        // --- movimento ao longo da superfície
        let mut rem_x = self.vx * dt;
        if self.surface.is_some() {
            rem_x = self.walk(rem_x, drop, geo);
        }

        // --- queda rápida (estado) e gravidade
        let airborne = self.surface.is_none();
        let down_held = input.down && controllable;
        if self.fast_fall {
            if !down_held || !airborne || mods.in_power || self.wall != 0 {
                self.fast_fall = false;
            }
        } else if down_held && airborne && !mods.in_power && self.wall == 0 && self.vy >= t.fast_fall_min_vy {
            self.fast_fall = true;
        }
        let holding_down = !self.fast_fall && down_held && !mods.in_power;
        if self.antigrav > 0 {
            self.antigrav -= 1;
        } else if airborne && !mods.cancel_gravity {
            let cap = if self.fast_fall || holding_down { t.fast_fall } else { t.max_fall };
            if self.vy < cap {
                self.vy += t.gravity * dt;
                if !self.stunned {
                    self.vy += t.extra(self.vy) * dt;
                }
            }
            if self.fast_fall {
                self.vy += t.fast_fall_gravity * dt;
                if self.vy < 0.0 {
                    self.vy = 0.0;
                }
            }
            if !self.stunned {
                if self.vy > t.fast_fall {
                    self.vy = t.fast_fall;
                }
                if self.vy > t.max_fall {
                    if mods.in_power {
                        self.vy = t.max_fall;
                    } else if !self.fast_fall && !holding_down {
                        self.vy = (self.vy - t.fall_decay * dt).max(t.max_fall);
                    }
                }
            }
        }

        // --- varredura geral: pés e cabeça
        let grounded = self.surface.is_some();
        let vec = (if grounded { rem_x } else { self.vx * dt }, self.vy * dt + if grounded { GROUND_PROBE } else { 0.0 });
        self.sweep_move(vec, drop, p, geo, &mut ev);
        ev
    }

    /// Anda `dist` (com sinal) ao longo da superfície atual, passando para a seguinte ao encontrá-la.
    /// Devolve a distância horizontal que sobrou (0 quando tudo foi consumido).
    fn walk(&mut self, dist: f32, drop: bool, geo: &Geometry) -> f32 {
        let mut rem = dist;
        for _ in 0..4 {
            let Some(si) = self.surface else { break };
            if rem == 0.0 {
                break;
            }
            let d = geo.surfaces[si].dir();
            let vec = (d.0 * rem, d.1 * rem);
            match geo.sweep((self.x, self.y), vec, drop, Some(si)) {
                Some(h) => {
                    let moved = (h.point.0 - self.x, h.point.1 - self.y);
                    let l = len(moved);
                    let back = if l > 0.0 { (moved.0 / l * BACKOFF, moved.1 / l * BACKOFF) } else { (0.0, 0.0) };
                    self.x = h.point.0 - back.0;
                    self.y = h.point.1 - back.1;
                    rem -= rem.signum() * l;
                    let s = &geo.surfaces[h.surf];
                    if s.is_floor() {
                        self.surface = Some(h.surf);
                    } else {
                        // parede ou teto no caminho: para
                        self.vx = 0.0;
                        return 0.0;
                    }
                }
                None => {
                    self.x += vec.0;
                    self.y += vec.1;
                    return 0.0;
                }
            }
        }
        0.0
    }

    fn sweep_move(&mut self, vec: (f32, f32), drop: bool, p: &Params, geo: &Geometry, ev: &mut StepEvents) {
        let t = &p.t;
        let was_airborne = self.surface.is_none();
        let (mv, hit, head_hit) = self.cast(vec, drop, geo);
        let (mut mv, mut hit, mut head_hit) = (mv, hit, head_hit);

        // a linha do corpo não pode ficar atravessada numa superfície dura: refaz só com a componente vertical
        let (tx, ty) = (self.x + mv.0, self.y + mv.1);
        if vec.0 != 0.0 && geo.sweep((tx, ty), (0.0, -BODY_HEIGHT), true, None).is_some() {
            let r = self.cast((0.0, vec.1), drop, geo);
            mv = r.0;
            hit = r.1;
            head_hit = r.2;
        }
        self.x += mv.0;
        self.y += mv.1;

        let prev_wall = self.wall;
        self.wall = 0;
        let Some(h) = hit else {
            self.surface = None;
            // ao soltar a parede, é empurrado para longe dela
            if prev_wall == 1 && self.vx < 20.0 && !self.stunned {
                self.facing = 1;
                self.vx = 20.0;
            } else if prev_wall == 2 && self.vx > -20.0 && !self.stunned {
                self.facing = -1;
                self.vx = -20.0;
            }
            return;
        };
        let s = geo.surfaces[h.surf];

        // recuo: fica ligeiramente fora da superfície
        let l = len(mv);
        let back = if l <= 1.02 * 1.5 || l == 0.0 { s.n } else { (-mv.0 / l, -mv.1 / l) };
        self.x += back.0 * BACKOFF;
        self.y += back.1 * BACKOFF;

        if self.stunned {
            // quica em hitstun
            let speed = len((self.vx, self.vy));
            if s.is_vertical() {
                self.vx = if speed >= t.bounce_min_speed { -self.vx * t.bounce } else { 0.0 };
            } else if s.is_horizontal() {
                self.vy = if speed >= t.bounce_min_speed { -self.vy * t.bounce } else { 0.0 };
            } else {
                let vn = self.vx * s.n.0 + self.vy * s.n.1;
                let (nx, ny) = (vn * s.n.0, vn * s.n.1);
                let (mut tx, mut ty) = (self.vx - nx, self.vy - ny);
                let tl = len((tx, ty));
                let keep = (tl - 2.5).max(0.0);
                if tl > 0.0 {
                    tx *= keep / tl;
                    ty *= keep / tl;
                }
                let (rx, ry) = (tx - nx * t.bounce, ty - ny * t.bounce);
                if len((rx, ry)) >= t.bounce_min_speed {
                    self.vx = rx;
                    self.vy = ry;
                }
            }
            ev.bounced = true;
            self.surface = if s.is_floor() && len((self.vx, self.vy)) < 1.0 { Some(h.surf) } else { None };
            return;
        }

        if head_hit {
            // cabeça no teto
            if self.vy < 0.0 {
                self.vy = 0.0;
            }
            self.surface = None;
        } else if s.is_vertical() {
            self.surface = None;
            if was_airborne {
                self.touch_wall(&s, prev_wall, p, geo);
            } else {
                self.vx = 0.0;
            }
        } else if s.is_floor() {
            if was_airborne {
                ev.landed = true;
            }
            self.surface = Some(h.surf);
            self.vy = 0.0;
            self.air_jumps_used = 0;
            self.fast_fall = false;
        } else {
            // por baixo de uma superfície inclinada
            if self.vy < 0.0 {
                self.vy = 0.0;
            }
            self.surface = None;
        }
    }

    /// Varre pés e cabeça com `vec`; devolve o movimento permitido, a superfície atingida e se foi a cabeça.
    fn cast(&self, vec: (f32, f32), drop: bool, geo: &Geometry) -> ((f32, f32), Option<geometry::Hit>, bool) {
        let feet = (self.x, self.y);
        let head = (self.x, self.y - BODY_HEIGHT);
        let mut mv = vec;
        let mut hit = geo.sweep(feet, vec, drop, None);
        let mut head_hit = false;
        if let Some(h) = hit {
            mv = (h.point.0 - feet.0, h.point.1 - feet.1);
        }
        if let Some(hh) = geo.sweep(head, vec, true, None) {
            let hv = (hh.point.0 - head.0, hh.point.1 - head.1);
            if len(hv) < len(mv) {
                mv = hv;
                hit = Some(hh);
                // só conta como "cabeça" quando a superfície olha para baixo (teto); paredes seguem a regra geral
                head_hit = geo.surfaces[hh.surf].n.1 > 0.0;
            }
        }
        (mv, hit, head_hit)
    }

    /// Contato com uma parede no ar: encaixa na borda se a cabeça já passou do topo, senão agarra e desliza.
    fn touch_wall(&mut self, s: &Surface, prev_wall: u8, p: &Params, geo: &Geometry) {
        let t = &p.t;
        let toward_wall = if s.a.0 > self.x { 1.0 } else { -1.0 };
        let head = (self.x, self.y - BODY_HEIGHT);
        let head_on_wall = geo.sweep(head, (toward_wall * (GROUND_PROBE + BACKOFF), 0.0), true, None).is_some_and(|h| geo.surfaces[h.surf].is_vertical());

        // encaixe de borda: perto do topo e não subindo rápido, sobe para a plataforma
        if !head_on_wall && self.y - 60.0 < s.top() && self.vy > -35.0 {
            let from = (self.x, s.top() - 5.0);
            if let Some(h) = geo.sweep(from, (toward_wall * (GROUND_PROBE + BACKOFF), 6.0), true, None) {
                if geo.surfaces[h.surf].is_floor() {
                    self.x = h.point.0;
                    self.y = h.point.1 - BACKOFF;
                    self.vy = t.gravity * t.dt;
                    return;
                }
            }
        }
        if !(head_on_wall || prev_wall != 0) {
            self.vx = 0.0;
            return;
        }
        self.wall = if toward_wall > 0.0 { 2 } else { 1 };
        if prev_wall == 0 {
            // primeiro contato: para de cair e fica um instante sem gravidade
            if self.vy >= 0.0 {
                self.vy = 0.0;
                self.antigrav = t.wall_cling_ticks;
            }
        } else if self.vy > t.gravity * t.wall_slide * t.dt {
            // deslizando: a maior parte da gravidade deste tick é devolvida
            self.vy -= (t.gravity + t.extra(self.vy)) * t.wall_slide * t.dt;
        }
        // mantém uma pequena velocidade contra a parede para o contato persistir
        self.vx = toward_wall * 10.0;
        self.air_jumps_used = 0;
        self.fast_fall = false;
    }
}

#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod world_tests;
