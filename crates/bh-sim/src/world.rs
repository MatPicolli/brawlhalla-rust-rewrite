//! Partida: vários lutadores sobre um mapa, com golpes leves e pesados (poderes de `powerTypes`), esquiva,
//! acertos, dano e hitstun. Passo fixo de 60 Hz.
//!
//! Regras de execução dos poderes (do motor original, `_-6z._-e58`):
//! - cada evento do `CastTime` ocorre num tick; no evento o atacante recebe o `FireImpulse` e o hitbox é testado;
//! - um alvo é atingido no máximo uma vez por poder;
//! - o encadeamento é automático: ao acertar, entra o `ComboOverrideIfHit` (se houver); ao fim dos eventos entra
//!   o `ComboName`; sem próximo poder, vale a recuperação (`RecoverTime` dividido pela Destreza);
//! - golpes pesados carregam enquanto o botão está segurado (entre o mínimo e o máximo) e então soltam o combo.

use bh_assets::{
    dodge::{self, DodgeDir, DodgeType},
    powers::{Ellipse, Power, PowerTable, TimeMods},
    stats::WeaponKit,
};

use crate::{
    combat::{self, CombatStats, CENTER_TO_FEET, HURTBOX_H, HURTBOX_W},
    Fighter, Geometry, Input, Params, StepEvents, StepMods,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct ActorInput {
    pub input: Input,
    /// Botão de ataque leve acabou de ser apertado.
    pub attack: bool,
    /// Botão de ataque pesado acabou de ser apertado / está segurado.
    pub heavy: bool,
    pub heavy_held: bool,
    /// Botão de esquiva acabou de ser apertado.
    pub dodge: bool,
    /// Direção para cima segurada.
    pub up: bool,
}

#[derive(Debug, Clone)]
pub struct Attack {
    pub power: String,
    /// Ticks desde o início do poder.
    pub tick: u32,
    pub hit: bool,
    /// Alvos já atingidos por este poder (um acerto por alvo por poder).
    pub victims: Vec<usize>,
    /// Animação em curso e quadro atual (1-based, fracionário).
    pub anim: String,
    pub anim_frame: f32,
    anim_start: f32,
    /// Ticks de carga (golpes pesados) e a fração do máximo com que o golpe foi solto.
    pub charge: u32,
    pub charge_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct DodgeState {
    pub ty: usize,
    pub tick: u32,
    pub vx: f32,
    pub vy: f32,
    pub from_air: bool,
}

#[derive(Debug, Clone)]
pub struct Actor {
    pub fighter: Fighter,
    pub params: Params,
    pub stats: CombatStats,
    pub time_mods: TimeMods,
    pub kit: WeaponKit,
    pub damage: f32,
    pub stun: u32,
    pub attack: Option<Attack>,
    pub dodge: Option<DodgeState>,
    /// Ticks até poder esquivar / atacar de novo.
    pub dodge_cooldown: u32,
    pub attack_cooldown: u32,
    pub kos: u32,
    pub spawn: (f32, f32),
    /// Magnitude do último knockback sofrido (para escolher a animação de reação).
    pub last_knockback: f32,
    /// Eventos de movimento do último tick.
    pub moved: StepEvents,
}

/// O que ocorreu num tick, para a camada gráfica.
#[derive(Debug, Clone, Default)]
pub struct Events {
    /// (atacante, alvo, dano, knockback)
    pub hits: Vec<(usize, usize, f32, f32)>,
    pub kos: Vec<usize>,
}

pub struct World {
    pub geo: Geometry,
    pub powers: PowerTable,
    pub dodges: Vec<DodgeType>,
    pub actors: Vec<Actor>,
    pub tick: u32,
    /// Limites do mapa (x1, y1, x2, y2); sair deles é KO.
    pub bounds: (f32, f32, f32, f32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dir {
    Neutral,
    Side,
    Down,
    Up,
}

impl Actor {
    pub fn new(spawn: (f32, f32), params: Params, stats: CombatStats, weapon: &str) -> Self {
        Actor::with_kit(spawn, params, stats, TimeMods::default(), WeaponKit { weapon: weapon.to_string(), ..Default::default() })
    }

    pub fn with_kit(spawn: (f32, f32), params: Params, stats: CombatStats, time_mods: TimeMods, kit: WeaponKit) -> Self {
        Actor {
            fighter: Fighter::new(spawn.0, spawn.1, &params),
            params,
            stats,
            time_mods,
            kit,
            damage: 0.0,
            stun: 0,
            attack: None,
            dodge: None,
            dodge_cooldown: 0,
            attack_cooldown: 0,
            kos: 0,
            spawn,
            last_knockback: 0.0,
            moved: StepEvents::default(),
        }
    }

    /// Centro do corpo (referência dos golpes).
    pub fn center(&self) -> (f32, f32) {
        (self.fighter.x, self.fighter.y - CENTER_TO_FEET)
    }

    pub fn respawn(&mut self) {
        let (sx, sy) = self.spawn;
        let p = self.params;
        self.fighter.respawn(sx, sy, &p);
        self.damage = 0.0;
        self.stun = 0;
        self.attack = None;
        self.dodge = None;
        self.dodge_cooldown = 0;
        self.attack_cooldown = 0;
    }
}

fn dir_of(inp: &ActorInput) -> Dir {
    if inp.input.down {
        Dir::Down
    } else if inp.input.move_x.abs() > 0.1 {
        Dir::Side
    } else if inp.up {
        Dir::Up
    } else {
        Dir::Neutral
    }
}

/// Nome do poder para a arma, direção, tipo (leve/pesado) e se está no ar.
fn power_name(kit: &WeaponKit, airborne: bool, dir: Dir, heavy: bool) -> String {
    let w = &kit.weapon;
    match (heavy, airborne, dir) {
        (false, false, Dir::Side) => format!("{w}Side"),
        (false, false, Dir::Down) => format!("{w}Down"),
        (false, false, _) => format!("{w}Neutral"),
        (false, true, Dir::Side) => format!("{w}AirSide"),
        (false, true, Dir::Down) => format!("{w}AirDown"),
        (false, true, _) => format!("{w}Air"),
        (true, false, Dir::Side) => kit.sig_side.clone(),
        (true, false, Dir::Down) => kit.sig_down.clone(),
        (true, false, _) => kit.sig_neutral.clone(),
        (true, true, Dir::Down) => format!("{w}GroundPound"),
        (true, true, _) => format!("{w}AirUpHeavy"),
    }
}

/// Fator da fórmula `StandardHorizontal` (0,6..1,0): quanto a velocidade atual já aponta para a esquiva.
fn standard_factor(vx: f32, top: f32, sign: f32) -> f32 {
    let target = top * sign;
    let span = (top * 2.0).abs().max(1e-6);
    let diff = (target - vx).abs().min(span);
    let k = 1.0 - diff / span;
    if k >= 0.5 { 0.6 + k * k * 0.4 } else { 0.7 + (k - 0.5).powi(3) * 0.8 }
}

/// Velocidade (vx, vy) de uma esquiva, em unidades do motor.
pub fn dodge_velocity(d: &DodgeType, f: &Fighter, p: &Params, side: f32, has_side: bool) -> (f32, f32) {
    let airborne = !f.grounded();
    let top = if airborne { p.air_speed } else { p.run_speed };
    let factor = |formula: &str| match formula {
        "StandardHorizontal" => standard_factor(f.vx, top, if side == 0.0 { f.facing as f32 } else { side }),
        "MomentumHorizontal" => (1.0 + f.vx.abs() / 60.0) / 2.0,
        _ => 1.0,
    };
    // x: entre Mult e MaxMult conforme |vx| / velocidade no ar (0,33..0,57)
    let mx = if has_side && d.speed_x.1 != 0.0 {
        let k = ((f.vx.abs() / p.air_speed).clamp(0.33, 0.57) - 0.33) / 0.24;
        d.speed_x.0 + k * (d.speed_x.1 - d.speed_x.0)
    } else {
        d.speed_x.0
    };
    // y: entre Mult e MaxMult conforme a velocidade vertical atual
    let vertical = matches!(d.dir, DodgeDir::Up | DodgeDir::Down | DodgeDir::SideUp | DodgeDir::SideDown);
    let my = if vertical && d.speed_y.1 != 0.0 {
        let going_up = d.speed_y.0 < 0.0;
        let raw = if going_up { f.vy.min(0.0) / -40.0 } else { f.vy.max(0.0) / 70.0 };
        let k = (raw.clamp(0.5, 0.75) - 0.5) / 0.25;
        d.speed_y.0 + k * (d.speed_y.1 - d.speed_y.0)
    } else {
        d.speed_y.0
    };
    (60.0 * factor(&d.speed_x_formula) * mx * if side == 0.0 { f.facing as f32 } else { side }, 60.0 * factor(&d.speed_y_formula) * my)
}

impl World {
    pub fn new(geo: Geometry, powers: PowerTable, bounds: (f32, f32, f32, f32)) -> Self {
        World { geo, powers, dodges: Vec::new(), actors: Vec::new(), tick: 0, bounds }
    }

    /// O lutador está invulnerável (esquiva depois dos ticks iniciais)?
    pub fn invulnerable(&self, i: usize) -> bool {
        self.actors[i].dodge.as_ref().is_some_and(|d| self.dodges.get(d.ty).is_some_and(|t| d.tick >= t.start_invuln))
    }

    /// Elipses ativos de um ataque neste tick, no mundo (já espelhados e relativos ao centro do atacante).
    pub fn active_boxes(&self, actor: usize) -> Vec<Ellipse> {
        let a = &self.actors[actor];
        let Some(atk) = &a.attack else { return Vec::new() };
        let Some(power) = self.powers.get(&atk.power) else { return Vec::new() };
        let (cx, cy) = a.center();
        let sign = a.fighter.facing as f32;
        let mut out = Vec::new();
        for (h, k, t) in power.schedule() {
            if t == atk.tick {
                for e in &power.hits[h].boxes[k as usize] {
                    if e.rx > 0.0 && e.ry > 0.0 {
                        out.push(Ellipse { cx: cx + e.cx * sign, cy: cy + e.cy, rx: e.rx, ry: e.ry });
                    }
                }
            }
        }
        out
    }

    pub fn step(&mut self, inputs: &[ActorInput]) -> Events {
        let mut ev = Events::default();
        let n = self.actors.len();
        for i in 0..n {
            self.step_actor(i, inputs.get(i).copied().unwrap_or_default());
        }
        for i in 0..n {
            self.resolve_hits(i, &mut ev);
        }
        for i in 0..n {
            self.advance_attack(i, inputs.get(i).copied().unwrap_or_default());
            let (x1, y1, x2, y2) = self.bounds;
            let a = &mut self.actors[i];
            let (x, y) = (a.fighter.x, a.fighter.y);
            if x < x1 || x > x2 || y < y1 || y > y2 {
                a.kos += 1;
                a.respawn();
                ev.kos.push(i);
            }
        }
        self.tick += 1;
        ev
    }

    fn start_attack(&mut self, i: usize, name: String, charge_ratio: f32, victims: Vec<usize>) -> bool {
        let Some(p) = self.powers.get(&name) else { return false };
        let a = &mut self.actors[i];
        // animação: nova se o poder tem a sua; senão continua a do poder anterior
        let (anim, start) = match (&a.attack, p.cast_anim.is_empty()) {
            (Some(prev), true) => (prev.anim.clone(), prev.anim_frame),
            (Some(prev), false) if prev.anim == p.cast_anim && prev.power != name => (prev.anim.clone(), prev.anim_frame),
            _ => (p.cast_anim.clone(), 1.0),
        };
        // velocidade ao começar o poder: impulso definido ou limites
        let f = &mut a.fighter;
        let sign = f.facing as f32;
        match p.cast_impulse.0 {
            Some(x) => f.vx = x * sign,
            None => {
                let backward = f.vx != 0.0 && (f.vx < 0.0) != (sign < 0.0);
                if p.speed_limit_backward != 0.0 && backward && f.vx.abs() > p.speed_limit_backward {
                    f.vx = -sign * p.speed_limit_backward;
                } else if p.speed_limit != 0.0 && f.vx.abs() > p.speed_limit {
                    f.vx = sign * p.speed_limit;
                }
            }
        }
        match p.cast_impulse.1 {
            Some(y) => f.vy = y,
            None => {
                if (p.speed_limit_y < 0.0 && f.vy < p.speed_limit_y) || (p.speed_limit_y > 0.0 && f.vy > p.speed_limit_y) {
                    f.vy = p.speed_limit_y;
                }
            }
        }
        let frame = p.anim_frame(0, start, &a.time_mods);
        a.attack = Some(Attack { power: name, tick: 0, hit: false, victims, anim, anim_frame: frame, anim_start: start, charge: 0, charge_ratio });
        true
    }

    fn step_actor(&mut self, i: usize, mut inp: ActorInput) {
        {
            let a = &mut self.actors[i];
            let stunned = a.stun > 0;
            if stunned {
                a.stun -= 1;
                a.attack = None;
                a.dodge = None;
                inp = ActorInput::default();
            }
            a.fighter.stunned = stunned;
            a.dodge_cooldown = a.dodge_cooldown.saturating_sub(1);
            a.attack_cooldown = a.attack_cooldown.saturating_sub(1);
        }
        let free = {
            let a = &self.actors[i];
            a.stun == 0 && !a.fighter.stunned && a.attack.is_none() && a.dodge.is_none()
        };

        // --- início de esquiva
        if free && inp.dodge && self.actors[i].dodge_cooldown == 0 {
            let a = &self.actors[i];
            let airborne = !a.fighter.grounded();
            let side = if inp.input.move_x > 0.1 { 1.0 } else if inp.input.move_x < -0.1 { -1.0 } else { 0.0 };
            let dir = match (side != 0.0, inp.up, inp.input.down, airborne) {
                (true, true, _, _) => DodgeDir::SideUp,
                (true, _, true, true) => DodgeDir::SideDown,
                (true, _, _, _) => DodgeDir::Side,
                (false, true, _, _) => DodgeDir::Up,
                (false, _, true, true) => DodgeDir::Down,
                _ => DodgeDir::Neutral,
            };
            if let Some(t) = dodge::pick(&self.dodges, dir, airborne) {
                let ty = self.dodges.iter().position(|d| d.name == t.name).unwrap();
                let (vx, vy) = dodge_velocity(t, &a.fighter, &a.params, side, side != 0.0);
                let a = &mut self.actors[i];
                if side != 0.0 {
                    a.fighter.facing = side as i8;
                }
                a.fighter.surface = if vy < 0.0 { None } else { a.fighter.surface };
                a.dodge = Some(DodgeState { ty, tick: 0, vx, vy, from_air: airborne });
            }
        }

        // --- início de ataque
        let free = {
            let a = &self.actors[i];
            a.stun == 0 && !a.fighter.stunned && a.attack.is_none() && a.dodge.is_none() && a.attack_cooldown == 0
        };
        if free && (inp.attack || inp.heavy) {
            let a = &self.actors[i];
            let dir = dir_of(&inp);
            let name = power_name(&a.kit, !a.fighter.grounded(), dir, inp.heavy && !inp.attack);
            if self.powers.get(&name).is_some() {
                if dir == Dir::Side {
                    self.actors[i].fighter.facing = if inp.input.move_x > 0.0 { 1 } else { -1 };
                }
                self.start_attack(i, name, 0.0, Vec::new());
            }
        }

        // --- movimento deste tick conforme o estado
        let mut input = inp.input;
        let mut mods = StepMods::default();
        let a = &mut self.actors[i];
        if let Some(d) = &a.dodge {
            // esquiva: velocidade constante, sem gravidade nem atrito
            a.fighter.vx = d.vx;
            a.fighter.vy = if a.fighter.grounded() && d.vy > 0.0 { 0.0 } else { d.vy };
            input = Input::default();
            mods = StepMods { allow_move: false, allow_jump: false, cancel_gravity: true, no_friction: true, in_power: true, ..Default::default() };
        } else if let Some(atk) = &a.attack {
            if let Some(p) = self.powers.get(&atk.power) {
                mods = StepMods {
                    allow_move: p.allow_move,
                    accel_mult: p.accel_mult,
                    allow_jump: false,
                    cancel_gravity: p.cancel_gravity || atk.tick < p.antigrav_time,
                    keep_ground_friction: p.keep_ground_friction,
                    no_friction: false,
                    in_power: true,
                };
                // FireImpulse dos eventos deste tick: define a velocidade do atacante
                let sign = a.fighter.facing as f32;
                for (h, k, t) in p.schedule() {
                    if t == atk.tick {
                        let (fx, fy) = p.hits[h].fire_at(k);
                        if let Some(x) = fx {
                            a.fighter.vx = x * sign;
                        }
                        if let Some(y) = fy {
                            a.fighter.vy = y;
                            if y < 0.0 {
                                a.fighter.surface = None;
                            }
                        }
                    }
                }
            }
            input.jump = false;
        }
        a.moved = a.fighter.step_with(&input, &a.params, &self.geo, &mods);

        // --- fim da esquiva
        let mut end_dodge = None;
        if let Some(d) = a.dodge.as_mut() {
            d.tick += 1;
            if let Some(t) = self.dodges.get(d.ty) {
                if d.tick >= t.duration {
                    end_dodge = Some((t.clone(), d.from_air, d.vx));
                }
            }
        }
        if let Some((t, from_air, dvx)) = end_dodge {
            let back = inp.input.move_x != 0.0 && dvx != 0.0 && (inp.input.move_x > 0.0) != (dvx > 0.0);
            let exit = if back { t.exit_back } else { t.exit };
            a.fighter.vx *= exit.0;
            a.fighter.vy *= exit.1;
            a.dodge = None;
            a.dodge_cooldown = if from_air { t.cooldown } else { t.cooldown_ground };
        }
        // pousar depois de uma esquiva aérea encurta a recarga
        if a.moved.landed && a.dodge.is_none() {
            if let Some(t) = self.dodges.first() {
                a.dodge_cooldown = a.dodge_cooldown.min(t.cooldown_landed);
            }
        }
    }

    fn resolve_hits(&mut self, i: usize, ev: &mut Events) {
        let boxes = self.active_boxes(i);
        if boxes.is_empty() {
            return;
        }
        let (power_name, tick) = {
            let atk = self.actors[i].attack.as_ref().unwrap();
            (atk.power.clone(), atk.tick)
        };
        let Some(power) = self.powers.get(&power_name).cloned() else { return };
        if power.target_method == "Nobody" || power.target_method == "Smash" {
            return;
        }
        let Some((h, k, _)) = power.schedule().into_iter().find(|&(h, k, t)| t == tick && power.hits[h].boxes[k as usize].iter().any(|e| e.rx > 0.0)) else {
            return;
        };
        let base = power.hits[h].damage_at(k);
        if base == 0.0 && !power.allow_zero_damage_hit {
            return;
        }
        for j in 0..self.actors.len() {
            if j == i || self.actors[i].attack.as_ref().is_some_and(|a| a.victims.contains(&j)) || self.invulnerable(j) {
                continue;
            }
            let (vx, vy) = self.actors[j].center();
            if !boxes.iter().any(|e| combat::ellipse_hits_rect(*e, vx, vy, HURTBOX_W, HURTBOX_H)) {
                continue;
            }
            let (dmg, kb) = self.apply_hit(i, j, &power, h, k);
            let atk = self.actors[i].attack.as_mut().unwrap();
            atk.hit = true;
            atk.victims.push(j);
            ev.hits.push((i, j, dmg, kb));
        }
    }

    /// Aplica o golpe ao alvo e devolve `(dano causado, magnitude do knockback)`.
    fn apply_hit(&mut self, i: usize, j: usize, power: &Power, h: usize, k: u32) -> (f32, f32) {
        let hit = &power.hits[h];
        let (atk_stats, mirrored, ac, ratio) = {
            let a = &self.actors[i];
            (a.stats, a.fighter.facing < 0, a.center(), a.attack.as_ref().map(|x| x.charge_ratio).unwrap_or(0.0))
        };
        let (vic_stats, vc, vic_damage) = {
            let v = &self.actors[j];
            (v.stats, v.center(), v.damage)
        };
        // golpes soltos de uma carga: dano e impulso variável crescem até 25% com a carga
        let charge = if power.target_method.starts_with("SmashRelease") { 1.0 + 0.25 * ratio } else { 1.0 };
        let dmg = combat::damage_dealt(hit.damage_at(k), &atk_stats, &vic_stats) * charge;
        let offset = hit.offset_at(k);
        let t = self.actors[i].params.t;

        let mut fixed = hit.fixed_at(k);
        let dir = if power.impulse_to_point > 0 {
            let target = (ac.0 + if mirrored { -offset.0 } else { offset.0 }, ac.1 + offset.1);
            let v = combat::impulse_to_point(vc, target, power.impulse_to_point, t.dt, t.gravity);
            let len = (v.0 * v.0 + v.1 * v.1).sqrt().max(1e-6);
            fixed = fixed.min(len);
            (v.0 / len, v.1 / len)
        } else {
            combat::direction(ac, vc, offset, mirrored, power.lock_45)
        };
        let kb = combat::knockback(fixed, hit.variable_at(k) * charge, vic_damage, dmg, atk_stats.impulse_mult, power.ignore_strength);

        let v = &mut self.actors[j];
        v.damage += dmg;
        v.stun = power.stun;
        v.attack = None;
        v.dodge = None;
        v.last_knockback = kb;
        v.fighter.vx = dir.0 * kb;
        v.fighter.vy = dir.1 * kb;
        v.fighter.surface = None;
        v.fighter.wall = 0;
        v.fighter.fast_fall = false;
        (dmg, kb)
    }

    fn advance_attack(&mut self, i: usize, inp: ActorInput) {
        let Some(atk) = self.actors[i].attack.clone() else { return };
        let Some(power) = self.powers.get(&atk.power).cloned() else {
            self.actors[i].attack = None;
            return;
        };
        let mods = self.actors[i].time_mods;

        // carga dos golpes pesados: solta ao largar o botão (depois do mínimo) ou no máximo
        if power.target_method == "Smash" {
            let (min, max) = power.charge_window(&mods);
            let charge = atk.charge + 1;
            if (charge >= min && !inp.heavy_held) || charge >= max {
                let ratio = if max > 0 { (charge as f32 / max as f32).min(1.0) } else { 0.0 };
                if power.combo.is_empty() || !self.start_attack(i, power.combo.clone(), ratio, Vec::new()) {
                    self.end_attack(i, &power, false);
                }
            } else if let Some(a) = self.actors[i].attack.as_mut() {
                a.charge = charge;
                a.tick += 1;
                // a animação de carga avança um quadro por tick e para no fim
                a.anim_frame += 1.0;
            }
            return;
        }

        // acertou e há poder específico para o acerto: troca na hora
        if atk.hit && !power.combo_if_hit.is_empty() && power.combo_if_hit != atk.power {
            if self.start_attack(i, power.combo_if_hit.clone(), atk.charge_ratio, Vec::new()) {
                return;
            }
        }
        let last = power.last_event_tick();
        let tick = atk.tick + 1;
        if tick > last && !power.combo.is_empty() && power.combo != atk.power {
            // fim dos eventos: entra o próximo poder da cadeia, sem recuperação
            if self.start_attack(i, power.combo.clone(), atk.charge_ratio, Vec::new()) {
                if let Some(a) = self.actors[i].attack.as_mut() {
                    a.hit = false;
                }
                return;
            }
        }
        if tick >= power.total_ticks(&mods) {
            self.end_attack(i, &power, atk.hit);
            return;
        }
        if let Some(a) = self.actors[i].attack.as_mut() {
            a.tick = tick;
            a.anim_frame = power.anim_frame(tick, a.anim_start, &mods);
        }
    }

    fn end_attack(&mut self, i: usize, power: &Power, hit: bool) {
        let a = &mut self.actors[i];
        a.attack = None;
        a.attack_cooldown = if hit && power.on_hit_cooldown > 0 { power.on_hit_cooldown } else { power.cooldown };
    }
}
