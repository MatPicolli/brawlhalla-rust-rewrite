//! Poderes (golpes) de `powerTypes` em `Game.swz`, lidos como o `PowerType` do jogo faz (bytecode
//! `PowerType.static._-h2H` e `_-u1w`). Só os campos usados pela simulação.
//!
//! Microssintaxe dos campos numéricos (uma lista separada por vírgulas, um elemento por golpe/"hit"):
//! - `a&b&c`  hitboxes/valores **simultâneos** (constantes ao longo do golpe)
//! - `a~b~c`  interpolação linear por partes sobre os eventos do golpe
//! - `a|b|c`  alterna por evento
//! - `tN`     total N dividido igualmente entre os eventos
//! - `!`      não definido (impulsos)
//! - `n`      constante
//!
//! `CastTime` é uma lista de segmentos `A:B@C-D` (ou `A@C`): `A` ticks de preparação, depois `B + 1` eventos
//! (um por tick, no mínimo 1 tick entre eventos consecutivos) em que o hitbox está ativo. `@C` e `-D` são
//! marcadores da animação: o quadro mostrado é interpolado linearmente entre eles ao longo dos ticks.

use std::collections::HashMap;

use crate::character::{csv_section, read_rows};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipse {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
}

/// Um golpe (segmento de `CastTime`): ticks de preparação e os hitboxes de cada evento.
#[derive(Debug, Clone, Default)]
pub struct HitDef {
    /// Ticks entre o evento anterior e o primeiro evento deste golpe.
    pub pre: u32,
    /// Ticks em que o golpe está ativo menos 1 (`B`); há `events() = B + 1` eventos.
    pub active: u32,
    /// Hitboxes simultâneos de cada evento (relativos ao centro do corpo, `cx` espelha com o lado).
    pub boxes: Vec<Vec<Ellipse>>,
    /// Por evento (o jogo indexa esses vetores pelo evento em que o acerto ocorre).
    pub damage: Vec<f32>,
    pub fixed_impulse: Vec<f32>,
    pub variable_impulse: Vec<f32>,
    /// Deslocamento somado ao vetor atacante→alvo antes de normalizar (`x` espelha com o lado).
    pub impulse_offset: Vec<(f32, f32)>,
    /// Velocidade imposta ao atacante em cada evento (`FireImpulseX/Y`; `None` = não definido).
    pub fire_x: Vec<Option<f32>>,
    pub fire_y: Vec<Option<f32>>,
}

impl HitDef {
    pub fn events(&self) -> u32 {
        self.active + 1
    }

    fn at<T: Copy + Default>(v: &[T], k: u32) -> T {
        v.get(k as usize).or(v.last()).copied().unwrap_or_default()
    }

    pub fn damage_at(&self, k: u32) -> f32 {
        Self::at(&self.damage, k)
    }
    pub fn fixed_at(&self, k: u32) -> f32 {
        Self::at(&self.fixed_impulse, k)
    }
    pub fn variable_at(&self, k: u32) -> f32 {
        Self::at(&self.variable_impulse, k)
    }
    pub fn offset_at(&self, k: u32) -> (f32, f32) {
        Self::at(&self.impulse_offset, k)
    }
    pub fn fire_at(&self, k: u32) -> (Option<f32>, Option<f32>) {
        (Self::at(&self.fire_x, k), Self::at(&self.fire_y, k))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Power {
    pub name: String,
    pub parent_item: String,
    pub origin_power: String,
    /// `PBAoE` (área em volta do lutador), `Smash` (carga), `SmashRelease`, `Nobody`...
    pub target_method: String,
    pub is_air: bool,
    pub is_signature: bool,
    pub cast_anim: String,
    /// `L:` na animação: fica em laço enquanto o poder espera (carga).
    pub loop_anim: bool,
    pub hits: Vec<HitDef>,
    /// Recuperação depois do último evento: `recover` é dividido pelo modificador de Destreza, `fixed_recover` não.
    pub recover: u32,
    pub fixed_recover: u32,
    /// Marcadores de animação: (índice global do evento, quadro 1-based).
    pub anim_markers: Vec<(usize, f32)>,
    /// Quadro da animação ao fim da recuperação.
    pub recover_frame: Option<f32>,
    pub cooldown: u32,
    pub on_hit_cooldown: u32,
    /// Duração do stun (ticks) aplicada à vítima (`FixedStunTime`, primeiro elemento).
    pub stun: u32,
    pub combo: String,
    pub combo_if_hit: String,
    /// `ImpulseToPoint`: número de ticks para levar a vítima a um ponto (0 = desligado).
    pub impulse_to_point: u32,
    pub lock_45: bool,
    pub ignore_strength: bool,
    pub allow_zero_damage_hit: bool,
    /// `AllowMove`: o lutador pode andar durante o golpe.
    pub allow_move: bool,
    pub accel_mult: f32,
    pub multihit: bool,
    pub min_time_between_hits: u32,
    /// Velocidade imposta ao começar o poder (`CastImpulseX/Y`).
    pub cast_impulse: (Option<f32>, Option<f32>),
    /// Limites aplicados à velocidade ao começar o poder (0 = sem limite).
    pub speed_limit: f32,
    pub speed_limit_backward: f32,
    pub speed_limit_y: f32,
    pub cancel_gravity: bool,
    /// Ticks sem gravidade a partir do início do poder.
    pub antigrav_time: u32,
    pub keep_ground_friction: bool,
    /// Tempo mínimo de carga (ticks) dos golpes pesados; dividido pelo modificador de carga da Destreza.
    pub min_charge: u32,
}

/// Modificadores de tempo do atacante (Destreza). Os tempos são divididos por eles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeMods {
    pub recover: f32,
    pub air_recover: f32,
    pub sig_recover: f32,
    pub min_charge: f32,
}

impl Default for TimeMods {
    fn default() -> Self {
        TimeMods { recover: 1.0, air_recover: 1.0, sig_recover: 1.0, min_charge: 1.0 }
    }
}

impl Power {
    pub fn total_events(&self) -> usize {
        self.hits.iter().map(|h| h.events() as usize).sum()
    }

    /// Cronograma: `(hit, evento no hit, tick)` de cada evento, em ordem. O primeiro evento ocorre em
    /// `pre` ticks; cada evento seguinte avança `max(1, atraso)` ticks.
    pub fn schedule(&self) -> Vec<(usize, u32, u32)> {
        let mut out = Vec::with_capacity(self.total_events());
        let mut tick = 0u32;
        let mut first = true;
        for (h, hit) in self.hits.iter().enumerate() {
            for k in 0..hit.events() {
                let delay = if k == 0 { hit.pre } else { 0 };
                tick = if first { delay } else { tick + delay.max(1) };
                first = false;
                out.push((h, k, tick));
            }
        }
        out
    }

    /// Tick do último evento.
    pub fn last_event_tick(&self) -> u32 {
        self.schedule().last().map(|e| e.2).unwrap_or(0)
    }

    fn recover_mod(&self, m: &TimeMods) -> f32 {
        if self.is_signature {
            m.sig_recover
        } else if self.is_air {
            m.air_recover
        } else {
            m.recover
        }
    }

    /// Ticks de recuperação depois do último evento.
    pub fn recover_ticks(&self, m: &TimeMods) -> u32 {
        (self.recover as f32 / self.recover_mod(m).max(0.01)).floor() as u32 + self.fixed_recover
    }

    /// Duração total do poder em ticks (do início até poder agir de novo).
    pub fn total_ticks(&self, m: &TimeMods) -> u32 {
        self.last_event_tick() + self.recover_ticks(m) + 1
    }

    /// Tempo de carga: (mínimo, máximo) em ticks, para poderes `Smash`.
    pub fn charge_window(&self, m: &TimeMods) -> (u32, u32) {
        let max = self.hits.get(1).map(|h| h.pre).unwrap_or(0);
        let min = (self.min_charge as f32 / m.min_charge.max(0.01)).floor() as u32;
        (min.min(max), max)
    }

    /// Quadro da animação (1-based, fracionário) no tick `tick` do poder, partindo de `start_frame`.
    pub fn anim_frame(&self, tick: u32, start_frame: f32, m: &TimeMods) -> f32 {
        let sched = self.schedule();
        let mut keys: Vec<(u32, f32)> = vec![(0, start_frame)];
        for &(idx, frame) in &self.anim_markers {
            if let Some(e) = sched.get(idx) {
                keys.push((e.2, frame));
            }
        }
        if let Some(f) = self.recover_frame {
            keys.push((self.last_event_tick() + self.recover_ticks(m), f));
        }
        let mut prev = keys[0];
        for &k in &keys[1..] {
            if tick <= k.0 {
                if k.0 == prev.0 {
                    return k.1;
                }
                let t = (tick - prev.0) as f32 / (k.0 - prev.0) as f32;
                return prev.1 + (k.1 - prev.1) * t;
            }
            prev = k;
        }
        prev.1
    }
}

/// `parseFloat` do AS3: usa o maior prefixo numérico; 0 se não houver.
pub fn parse_f(s: &str) -> f32 {
    let s = s.trim();
    let b = s.as_bytes();
    let mut end = 0;
    let mut seen_digit = false;
    while end < b.len() {
        let c = b[end] as char;
        let ok = c.is_ascii_digit() || (end == 0 && (c == '-' || c == '+')) || c == '.' || ((c == 'e' || c == 'E') && seen_digit);
        if !ok {
            break;
        }
        seen_digit |= c.is_ascii_digit();
        end += 1;
    }
    s[..end].parse().unwrap_or(0.0)
}

pub fn parse_i(s: &str) -> i32 {
    parse_f(s) as i32
}

/// Valores por evento (cada um com a lista de valores simultâneos) para um elemento da lista.
pub fn series(elem: &str, events: usize) -> Vec<Vec<f32>> {
    let e = elem.trim();
    let events = events.max(1);
    if e.contains('&') {
        let vals: Vec<f32> = e.split('&').map(parse_f).collect();
        return vec![vals; events];
    }
    if let Some(rest) = e.strip_prefix('t') {
        return vec![vec![parse_f(rest) / events as f32]; events];
    }
    if e.contains('|') {
        let parts: Vec<f32> = e.split('|').map(parse_f).collect();
        return (0..events).map(|k| vec![parts[k % parts.len()]]).collect();
    }
    if e.contains('~') {
        let a: Vec<f32> = e.split('~').map(parse_f).collect();
        if a.len() < 2 || events == 1 {
            return vec![vec![a[0]]; events];
        }
        let s = (a.len() - 1) as f32;
        let span = (events as f32 - 1.0) / s;
        return (0..events)
            .map(|k| {
                let sec = (((k as f32 / events as f32) * s) as usize).min(a.len() - 2);
                let (start, end) = (sec as f32 * span, (sec + 1) as f32 * span);
                vec![a[sec] + (k as f32 - start) * ((a[sec + 1] - a[sec]) / (end - start))]
            })
            .collect();
    }
    vec![vec![parse_f(e)]; events]
}

/// Como `series`, mas `!` ou vazio significam "não definido".
fn series_opt(elem: &str, events: usize) -> Vec<Option<f32>> {
    let e = elem.trim();
    if e.is_empty() || e.starts_with('!') {
        return vec![None; events.max(1)];
    }
    series(e, events).into_iter().map(|v| Some(v[0])).collect()
}

fn elem<'a>(list: &'a [&'a str], hit: usize, default: &'a str) -> &'a str {
    let v = if list.len() > 1 { list.get(hit).copied() } else { list.first().copied() };
    v.filter(|s| !s.trim().is_empty()).unwrap_or(default)
}

fn flag(s: Option<&String>) -> bool {
    s.is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
}

fn uint(s: &str) -> u32 {
    parse_i(s).max(0) as u32
}

fn opt_f(s: &str) -> Option<f32> {
    let t = s.trim();
    (!t.is_empty() && !t.starts_with('!')).then(|| parse_f(t))
}

/// `N@F` -> (N, Some(F)); `N` -> (N, None). `end` antes do quadro é aceito e ignorado.
fn time_and_frame(s: &str) -> (u32, Option<f32>) {
    match s.split_once('@') {
        Some((n, f)) => (uint(n), Some(parse_f(f.trim().trim_start_matches("end")))),
        None => (uint(s), None),
    }
}

pub fn parse_power(row: &HashMap<String, String>) -> Power {
    let get = |k: &str| row.get(k).map(String::as_str).unwrap_or("");
    let raw_anim = get("CastAnim");
    let (anim_flags, cast_anim) = match raw_anim.rsplit_once(':') {
        Some((f, a)) => (f, a),
        None => ("", raw_anim),
    };
    let (recover, rf1) = time_and_frame(get("RecoverTime"));
    let (fixed_recover, rf2) = time_and_frame(get("FixedRecoverTime"));
    let mut p = Power {
        name: get("PowerName").to_string(),
        parent_item: get("ParentItem").to_string(),
        origin_power: get("OriginPower").to_string(),
        target_method: get("TargetMethod").to_string(),
        is_air: flag(row.get("IsAirPower")),
        is_signature: flag(row.get("IsSignature")),
        cast_anim: cast_anim.to_string(),
        loop_anim: anim_flags.contains('L'),
        recover,
        fixed_recover,
        recover_frame: rf2.or(rf1),
        cooldown: uint(get("CooldownTime")),
        on_hit_cooldown: uint(get("OnHitCooldownTime")),
        stun: uint(get("FixedStunTime").split(',').next().unwrap_or("0")),
        combo: get("ComboName").split('[').next().unwrap_or("").to_string(),
        combo_if_hit: get("ComboOverrideIfHit").to_string(),
        impulse_to_point: {
            let v = get("ImpulseToPoint").trim();
            if v.eq_ignore_ascii_case("true") { 1 } else { uint(v) }
        },
        lock_45: flag(row.get("LockTo45Degrees")),
        ignore_strength: flag(row.get("IgnoreStrength")),
        allow_zero_damage_hit: flag(row.get("AllowHitOnZeroDamage")),
        allow_move: flag(row.get("AllowMove")),
        accel_mult: opt_f(get("AccelMult").split(',').next().unwrap_or("")).unwrap_or(1.0),
        multihit: flag(row.get("IsMultihit")),
        min_time_between_hits: uint(get("MinTimeBetweenHits")),
        cast_impulse: (opt_f(get("CastImpulseX")), opt_f(get("CastImpulseY"))),
        speed_limit: parse_f(get("SpeedLimit")),
        speed_limit_backward: parse_f(get("SpeedLimitBackward")),
        speed_limit_y: parse_f(get("SpeedLimitY")),
        cancel_gravity: flag(row.get("CancelGravity")),
        antigrav_time: uint(get("AntigravTime")),
        keep_ground_friction: flag(row.get("KeepGroundFriction")),
        min_charge: uint(get("FixedMinChargeTime")),
        ..Default::default()
    };

    // segmentos de CastTime e marcadores de animação
    let cast = get("CastTime");
    let segs: Vec<&str> = if cast.trim().is_empty() { vec!["0"] } else { cast.split(',').collect() };
    let mut event_base = 0usize;
    for seg in &segs {
        let (pre, active, marks) = match seg.split_once(':') {
            Some((a, rest)) => {
                let (b, m) = match rest.split_once('@') {
                    Some((b, m)) => (uint(b), Some(m)),
                    None => (uint(rest), None),
                };
                (uint(a), b, m)
            }
            None => match seg.split_once('@') {
                Some((a, m)) => (uint(a), 0, Some(m)),
                None => (uint(seg), 0, None),
            },
        };
        if let Some(m) = marks {
            let mut it = m.split('-');
            if let Some(first) = it.next() {
                p.anim_markers.push((event_base, parse_f(first)));
            }
            if let Some(last) = it.next() {
                p.anim_markers.push((event_base + active as usize, parse_f(last)));
            }
        }
        event_base += active as usize + 1;
        p.hits.push(HitDef { pre, active, ..Default::default() });
    }

    let list = |k: &str| -> Vec<&str> { get(k).split(',').collect() };
    let (rx, ry, cx, cy) = (list("AoERadiusX"), list("AoERadiusY"), list("CenterOffsetX"), list("CenterOffsetY"));
    let (dmg, fix, var) = (list("BaseDamage"), list("FixedImpulse"), list("VariableImpulse"));
    let (ox, oy) = (list("ImpulseOffsetX"), list("ImpulseOffsetY"));
    let (fx, fy) = (list("FireImpulseX"), list("FireImpulseY"));
    for (h, hit) in p.hits.iter_mut().enumerate() {
        let ev = hit.events() as usize;
        let (srx, sry, scx, scy) =
            (series(elem(&rx, h, "0"), ev), series(elem(&ry, h, "0"), ev), series(elem(&cx, h, "0"), ev), series(elem(&cy, h, "0"), ev));
        for k in 0..ev {
            let n = srx[k].len().max(sry[k].len()).max(scx[k].len()).max(scy[k].len());
            let at = |v: &Vec<f32>, i: usize| v.get(i).or(v.first()).copied().unwrap_or(0.0);
            hit.boxes.push((0..n).map(|i| Ellipse { cx: at(&scx[k], i), cy: at(&scy[k], i), rx: at(&srx[k], i), ry: at(&sry[k], i) }).collect());
        }
        let per_event = |l: &[&str]| -> Vec<f32> { series(elem(l, h, "0"), ev).into_iter().map(|v| v[0]).collect() };
        hit.damage = per_event(&dmg);
        hit.fixed_impulse = per_event(&fix);
        hit.variable_impulse = per_event(&var);
        hit.impulse_offset = per_event(&ox).into_iter().zip(per_event(&oy)).collect();
        hit.fire_x = series_opt(elem(&fx, h, "!"), ev);
        hit.fire_y = series_opt(elem(&fy, h, "!"), ev);
    }
    p
}

#[derive(Debug, Default)]
pub struct PowerTable {
    map: HashMap<String, Power>,
}

impl PowerTable {
    pub fn load(game_sections: &[Vec<u8>]) -> PowerTable {
        let mut map = HashMap::new();
        if let Some(data) = csv_section(game_sections, "powerTypes") {
            for row in read_rows(data) {
                let p = parse_power(&row);
                if !p.name.is_empty() {
                    map.insert(p.name.clone(), p);
                }
            }
        }
        PowerTable { map }
    }

    pub fn from_powers(powers: Vec<Power>) -> PowerTable {
        PowerTable { map: powers.into_iter().map(|p| (p.name.clone(), p)).collect() }
    }

    pub fn get(&self, name: &str) -> Option<&Power> {
        self.map.get(name)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simultaneous_values_repeat_per_event() {
        let s = series("49&25", 6);
        assert_eq!(s.len(), 6);
        assert!(s.iter().all(|v| v == &vec![49.0, 25.0]));
    }

    #[test]
    fn range_interpolates_from_first_to_last_event() {
        let v: Vec<f32> = series("50~5", 6).iter().map(|x| x[0]).collect();
        assert_eq!(v[0], 50.0);
        assert!((v[5] - 5.0).abs() < 1e-4, "{v:?}");
        assert!(v.windows(2).all(|w| w[1] < w[0]));
    }

    #[test]
    fn total_and_alternating() {
        assert_eq!(series("t12", 4).iter().map(|x| x[0]).collect::<Vec<_>>(), vec![3.0; 4]);
        assert_eq!(series("1|2", 4).iter().map(|x| x[0]).collect::<Vec<_>>(), vec![1.0, 2.0, 1.0, 2.0]);
    }

    #[test]
    fn parse_f_uses_numeric_prefix() {
        assert_eq!(parse_f("25~22"), 25.0);
        assert_eq!(parse_f("-4&-18"), -4.0);
        assert_eq!(parse_f("abc"), 0.0);
        assert_eq!(parse_f("7@9"), 7.0);
    }

    fn row(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn neutral() -> Power {
        parse_power(&row(&[
            ("PowerName", "HammerNeutral"),
            ("AoERadiusX", "49&25"),
            ("AoERadiusY", "24&15"),
            ("CenterOffsetX", "83&106"),
            ("CenterOffsetY", "-4&-18"),
            ("CastTime", "6:5@2-3"),
            ("RecoverTime", "7@9"),
            ("BaseDamage", "5"),
            ("FixedImpulse", "50"),
            ("VariableImpulse", "0"),
            ("ImpulseOffsetX", "203~161"),
            ("ImpulseOffsetY", "12~7"),
            ("FixedStunTime", "20"),
            ("FireImpulseX", "50~5"),
            ("FireImpulseY", "0"),
            ("CastAnim", "AttackCombo1"),
        ]))
    }

    #[test]
    fn hammer_neutral_row() {
        let p = neutral();
        assert_eq!(p.hits.len(), 1);
        assert_eq!((p.hits[0].pre, p.hits[0].active, p.hits[0].events()), (6, 5, 6));
        assert_eq!(p.hits[0].boxes.len(), 6);
        assert_eq!(p.hits[0].boxes[0], vec![Ellipse { cx: 83.0, cy: -4.0, rx: 49.0, ry: 24.0 }, Ellipse { cx: 106.0, cy: -18.0, rx: 25.0, ry: 15.0 }]);
        assert_eq!((p.recover, p.stun, p.hits[0].damage_at(0), p.hits[0].fixed_at(0)), (7, 20, 5.0, 50.0));
        assert_eq!(p.hits[0].offset_at(0), (203.0, 12.0));
        let last = p.hits[0].offset_at(5);
        assert!((last.0 - 161.0).abs() < 1e-3 && (last.1 - 7.0).abs() < 1e-3, "{last:?}");
        // FireImpulseX 50~5: o atacante avança desacelerando; FireImpulseY 0 zera a vertical
        assert_eq!(p.hits[0].fire_at(0), (Some(50.0), Some(0.0)));
        assert!((p.hits[0].fire_at(5).0.unwrap() - 5.0).abs() < 1e-3);
        assert_eq!(p.last_event_tick(), 11);
    }

    #[test]
    fn recovery_is_divided_by_the_dexterity_modifier() {
        let p = neutral();
        let none = TimeMods::default();
        assert_eq!(p.recover_ticks(&none), 7);
        let bodvar = TimeMods { recover: 1.476, air_recover: 1.476, sig_recover: 1.26, min_charge: 1.32 };
        assert_eq!(p.recover_ticks(&bodvar), 4);
        assert_eq!(p.total_ticks(&bodvar), 11 + 4 + 1);
    }

    #[test]
    fn animation_is_interpolated_between_markers() {
        let p = neutral();
        let m = TimeMods::default();
        assert_eq!(p.anim_markers, vec![(0, 2.0), (5, 3.0)]);
        assert_eq!(p.recover_frame, Some(9.0));
        assert_eq!(p.anim_frame(0, 1.0, &m), 1.0);
        assert!((p.anim_frame(3, 1.0, &m) - 1.5).abs() < 1e-4);
        assert_eq!(p.anim_frame(6, 1.0, &m), 2.0);
        assert_eq!(p.anim_frame(11, 1.0, &m), 3.0);
        assert_eq!(p.anim_frame(18, 1.0, &m), 9.0);
        assert_eq!(p.anim_frame(99, 1.0, &m), 9.0);
    }

    #[test]
    fn multi_hit_schedule_advances_at_least_one_tick_per_event() {
        let p = parse_power(&row(&[
            ("PowerName", "HammerSide"),
            ("CastTime", "9:2@4,0:3@5,0:4@6-6"),
            ("BaseDamage", "0,16,16"),
            ("CenterOffsetX", "179&226,177&211,181&212"),
            ("CenterOffsetY", "-58&-45,-56&-49,-54&-48"),
            ("AoERadiusX", "43.5&48,45&33,23&34"),
            ("AoERadiusY", "56&66,38&46,34&43"),
            ("FireImpulseX", "45~56,56,56~22"),
            ("FireImpulseY", "0,-50~5~5~5,0"),
        ]));
        assert_eq!(p.hits.len(), 3);
        assert_eq!(p.hits[0].damage_at(0), 0.0);
        assert_eq!(p.hits[1].damage_at(2), 16.0);
        let ticks: Vec<u32> = p.schedule().iter().map(|e| e.2).collect();
        // 9,10,11 | 12,13,14,15 | 16..20: o primeiro evento de um golpe com atraso 0 vem um tick depois
        assert_eq!(ticks, vec![9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);
        assert_eq!(p.schedule()[3], (1, 0, 12));
        assert_eq!(p.hits[1].fire_at(0), (Some(56.0), Some(-50.0)));
        assert_eq!(p.hits[2].fire_at(4).0, Some(22.0));
    }

    #[test]
    fn unset_impulses_and_charge_window() {
        let p = parse_power(&row(&[
            ("PowerName", "HammerSmashNeutralViking"),
            ("TargetMethod", "Smash"),
            ("IsSignature", "TRUE"),
            ("CastAnim", "L:AttackSpecialBodvar"),
            ("CastTime", "0,72"),
            ("FixedMinChargeTime", "13"),
            ("SpeedLimit", "65"),
            ("FireImpulseX", "!"),
            ("ComboName", "HammerSmashNeutralComboViking"),
        ]));
        assert!(p.loop_anim && p.is_signature);
        assert_eq!(p.cast_anim, "AttackSpecialBodvar");
        assert_eq!(p.hits[0].fire_at(0), (None, None));
        assert_eq!(p.cast_impulse, (None, None));
        assert_eq!(p.speed_limit, 65.0);
        assert_eq!(p.charge_window(&TimeMods::default()), (13, 72));
        assert_eq!(p.charge_window(&TimeMods { min_charge: 1.32, ..Default::default() }), (9, 72));
    }
}
