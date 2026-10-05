//! Testes de integração do `World` com poderes do martelo escritos como no `powerTypes` do jogo.

use std::collections::HashMap;

use bh_assets::{
    dodge::{DodgeDir, DodgeType},
    powers::{parse_power, PowerTable, TimeMods},
    stats::WeaponKit,
};

use crate::{
    bot::Bot,
    combat::CombatStats,
    tests::{params, solid},
    world::{Actor, ActorInput, World},
    Geometry, Input,
};

fn row(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn hammer_powers() -> PowerTable {
    let rows: Vec<Vec<(&str, &str)>> = vec![
        vec![
            ("PowerName", "HammerNeutral"),
            ("TargetMethod", "PBAoE"),
            ("AoERadiusX", "49&25"),
            ("AoERadiusY", "24&15"),
            ("CenterOffsetX", "83&106"),
            ("CenterOffsetY", "-4&-18"),
            ("CastTime", "6:5@2-3"),
            ("RecoverTime", "7@9"),
            ("CooldownTime", "9"),
            ("BaseDamage", "5"),
            ("FixedImpulse", "50"),
            ("VariableImpulse", "0"),
            ("ImpulseOffsetX", "203~161"),
            ("ImpulseOffsetY", "12~7"),
            ("FixedStunTime", "20"),
            ("FireImpulseX", "50~5"),
            ("ImpulseToPoint", "21"),
            ("CastAnim", "AttackCombo1"),
            ("ComboName", "HammerNeutralCombo"),
        ],
        vec![
            ("PowerName", "HammerNeutralCombo"),
            ("TargetMethod", "PBAoE"),
            ("AoERadiusX", "49&15"),
            ("AoERadiusY", "25&15"),
            ("CenterOffsetX", "131&163"),
            ("CenterOffsetY", "8&-12"),
            ("CastTime", "4:5@3-4"),
            ("BaseDamage", "5"),
            ("FixedImpulse", "50"),
            ("ImpulseOffsetX", "267~237"),
            ("ImpulseOffsetY", "-10~-15"),
            ("FixedStunTime", "27"),
            ("CastAnim", "AttackCombo2"),
            ("ComboName", "HammerNeutralMiss"),
            ("ComboOverrideIfHit", "HammerNeutralHit"),
        ],
        vec![("PowerName", "HammerNeutralHit"), ("TargetMethod", "PBAoE"), ("CastTime", "0@5"), ("RecoverTime", "11@10"), ("CastAnim", "AttackCombo2")],
        vec![
            ("PowerName", "HammerNeutralMiss"),
            ("TargetMethod", "Nobody"),
            ("CastTime", "0@5"),
            ("RecoverTime", "13@10"),
            ("FixedRecoverTime", "3"),
            ("CooldownTime", "9"),
            ("CastAnim", "AttackCombo2"),
        ],
        vec![
            ("PowerName", "HammerSide"),
            ("TargetMethod", "PBAoE"),
            ("AoERadiusX", "43.5&48,45&33"),
            ("AoERadiusY", "56&66,38&46"),
            ("CenterOffsetX", "179&226,177&211"),
            ("CenterOffsetY", "-58&-45,-56&-49"),
            ("CastTime", "9:2@4,0:3@5"),
            ("BaseDamage", "0,16"),
            ("FixedImpulse", "0,56"),
            ("VariableImpulse", "0,8"),
            ("ImpulseOffsetX", "1000"),
            ("ImpulseOffsetY", "200"),
            ("FixedStunTime", "25"),
            ("CastAnim", "AttackDash"),
            ("ComboName", "HammerSideMiss"),
            ("ComboOverrideIfHit", "HammerSideHit"),
        ],
        vec![("PowerName", "HammerSideHit"), ("TargetMethod", "PBAoE"), ("RecoverTime", "19@17"), ("CooldownTime", "11")],
        vec![("PowerName", "HammerSideMiss"), ("TargetMethod", "PBAoE"), ("CastTime", "0@9"), ("RecoverTime", "23@17"), ("FixedRecoverTime", "1")],
        vec![
            ("PowerName", "SigCharge"),
            ("TargetMethod", "Smash"),
            ("IsSignature", "TRUE"),
            ("CastAnim", "L:AttackSpecialBodvar"),
            ("CastTime", "0,72"),
            ("FixedMinChargeTime", "13"),
            ("SpeedLimit", "65"),
            ("ComboName", "SigRelease"),
        ],
        vec![
            ("PowerName", "SigRelease"),
            ("TargetMethod", "SmashRelease"),
            ("IsSignature", "TRUE"),
            ("AoERadiusX", "62&47"),
            ("AoERadiusY", "110&66"),
            ("CenterOffsetX", "44&218"),
            ("CenterOffsetY", "-69&-77"),
            ("CastTime", "10:2@32"),
            ("FixedRecoverTime", "25@43"),
            ("CancelGravity", "TRUE"),
            ("BaseDamage", "24"),
            ("VariableImpulse", "66"),
            ("FixedImpulse", "65"),
            ("ImpulseOffsetX", "750"),
            ("ImpulseOffsetY", "-1250"),
            ("FixedStunTime", "28"),
        ],
    ];
    PowerTable::from_powers(rows.iter().map(|r| parse_power(&row(r))).collect())
}

fn dodges() -> Vec<DodgeType> {
    let base = DodgeType {
        name: "StandardSide".into(),
        dir: DodgeDir::Side,
        air_type: "Both".into(),
        fast: false,
        duration: 14,
        start_invuln: 2,
        speed_x_formula: "StandardHorizontal".into(),
        speed_x: (0.8818, 0.96),
        speed_y_formula: "NONE".into(),
        speed_y: (0.0, 0.0),
        cooldown: 163,
        cooldown_ground: 60,
        cooldown_landed: 75,
        exit: (1.0, 0.0),
        exit_back: (0.25, 0.0),
    };
    vec![
        base.clone(),
        DodgeType { name: "StandardSpot".into(), dir: DodgeDir::Neutral, air_type: "Ground".into(), duration: 18, speed_x_formula: "NONE".into(), speed_x: (0.0, 0.0), exit: (0.0, 0.0), ..base.clone() },
        DodgeType { name: "AerialSpot".into(), dir: DodgeDir::Neutral, air_type: "Air".into(), duration: 22, speed_x_formula: "NONE".into(), speed_x: (0.0, 0.0), exit: (0.0, 0.0), ..base.clone() },
        DodgeType { name: "StandardUp".into(), dir: DodgeDir::Up, speed_x_formula: "NONE".into(), speed_x: (0.0, 0.0), speed_y: (-0.5859, -0.6415), exit: (0.0, 1.0), ..base },
    ]
}

fn stats() -> CombatStats {
    CombatStats { strength: 6, defense: 5, impulse_mult: 1.074 }
}

fn kit() -> WeaponKit {
    WeaponKit { weapon: "Hammer".into(), sig_neutral: "SigCharge".into(), sig_side: "SigCharge".into(), sig_down: "SigCharge".into() }
}

fn flat_world() -> World {
    let geo = Geometry::new(&solid(-5000.0, 0.0, 5000.0, 500.0));
    let mut w = World::new(geo, hammer_powers(), (-5000.0, -5000.0, 5000.0, 5000.0));
    w.dodges = dodges();
    w
}

fn actor(x: f32) -> Actor {
    Actor::with_kit((x, -1.0), params(), stats(), TimeMods::default(), kit())
}

/// Dois lutadores sobre um piso largo; o alvo à direita a `gap` pixels do atacante.
fn duel(gap: f32) -> World {
    let mut w = flat_world();
    w.actors.push(actor(0.0));
    w.actors.push(actor(gap));
    for _ in 0..3 {
        w.step(&idle());
    }
    w
}

fn press_attack() -> [ActorInput; 2] {
    [ActorInput { attack: true, ..Default::default() }, ActorInput::default()]
}

fn idle() -> [ActorInput; 2] {
    [ActorInput::default(); 2]
}

fn run_right() -> ActorInput {
    ActorInput { input: Input { move_x: 1.0, ..Default::default() }, ..Default::default() }
}

/// Avança até o primeiro acerto e devolve (ticks desde o início, atacante, alvo, dano, knockback).
fn first_hit(w: &mut World, max: u32) -> Option<(u32, usize, usize, f32, f32)> {
    for t in 1..=max {
        if let Some(h) = w.step(&idle()).hits.first() {
            return Some((t, h.0, h.1, h.2, h.3));
        }
    }
    None
}

#[test]
fn neutral_light_hits_at_first_active_tick_with_exact_numbers() {
    let mut w = duel(120.0);
    w.step(&press_attack());
    let (tick, a, v, dmg, kb) = first_hit(&mut w, 30).expect("o golpe deveria acertar");
    // o poder começa no step que recebeu o botão; o primeiro evento é o tick 6 do poder
    assert_eq!(tick, 6);
    assert_eq!((a, v), (0, 1));
    assert!((dmg - 5.155).abs() < 1e-3, "{dmg}");
    // ImpulseToPoint: o impulso fixo (50) é limitado à velocidade que leva o alvo ao ponto em 21 ticks
    assert!(kb > 20.0 && kb < 50.0, "{kb}");
    assert_eq!(w.actors[1].stun, 20);
    assert!((w.actors[1].damage - 5.155).abs() < 1e-3);
    // o ponto fica 203 px à frente do atacante (no momento do acerto) e 12 px abaixo do centro
    let target_x = w.actors[0].fighter.x + 203.0;
    // interrompe a cadeia para medir só este golpe (o segundo golpe empurraria o alvo de novo)
    w.actors[0].attack = None;
    for _ in 0..21 {
        w.step(&idle());
    }
    let x = w.actors[1].fighter.x;
    assert!((x - target_x).abs() < 25.0, "alvo em x = {x}, ponto = {target_x}");
}

#[test]
fn target_out_of_reach_is_missed() {
    let mut w = duel(900.0);
    w.step(&press_attack());
    assert!(first_hit(&mut w, 60).is_none());
    assert_eq!(w.actors[1].damage, 0.0);
}

#[test]
fn fire_impulse_moves_the_attacker_forward() {
    let mut w = duel(900.0);
    let x0 = w.actors[0].fighter.x;
    w.step(&press_attack());
    for _ in 0..6 {
        w.step(&idle());
    }
    // no primeiro evento a velocidade vira o FireImpulseX (50, já com o atrito do tick descontado depois)
    assert!(w.actors[0].fighter.vx > 40.0, "vx = {}", w.actors[0].fighter.vx);
    for _ in 0..20 {
        w.step(&idle());
    }
    let dx = w.actors[0].fighter.x - x0;
    assert!(dx > 40.0 && dx < 200.0, "avanço = {dx}");
}

#[test]
fn attacker_cannot_run_or_jump_during_attack() {
    let mut w = duel(900.0);
    let run_jump = ActorInput { input: Input { move_x: -1.0, jump: true, down: false }, ..Default::default() };
    w.step(&[ActorInput { attack: true, ..Default::default() }, ActorInput::default()]);
    for _ in 0..5 {
        w.step(&[run_jump, ActorInput::default()]);
    }
    let f = &w.actors[0].fighter;
    assert!(f.grounded() && f.x.abs() < 1.0, "x = {}, no chão = {}", f.x, f.grounded());
}

#[test]
fn hit_target_ignores_input_during_hitstun() {
    let mut w = duel(120.0);
    w.step(&press_attack());
    first_hit(&mut w, 30).expect("acerto");
    assert!(w.actors[1].stun > 0);
    let act = ActorInput { input: Input { move_x: -1.0, ..Default::default() }, attack: true, heavy: true, dodge: true, ..Default::default() };
    w.step(&[ActorInput::default(), act]);
    assert!(w.actors[1].attack.is_none() && w.actors[1].dodge.is_none());
}

#[test]
fn combo_chains_automatically_and_switches_on_hit() {
    let mut w = duel(120.0);
    w.step(&press_attack());
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..80 {
        w.step(&idle());
        if let Some(a) = &w.actors[0].attack {
            if seen.last() != Some(&a.power) {
                seen.push(a.power.clone());
            }
        }
    }
    // primeiro golpe -> segundo golpe (sem apertar de novo) -> variante de acerto, e termina
    assert_eq!(seen, vec!["HammerNeutral", "HammerNeutralCombo", "HammerNeutralHit"], "{seen:?}");
    assert!(w.actors[0].attack.is_none());
    assert!(w.actors[1].damage > 10.0, "os dois golpes deveriam acertar: {}", w.actors[1].damage);
}

#[test]
fn combo_takes_the_miss_branch_when_nothing_is_hit() {
    let mut w = duel(900.0);
    w.step(&press_attack());
    let mut seen: Vec<String> = Vec::new();
    let mut ticks = 0;
    for t in 0..120 {
        w.step(&idle());
        match &w.actors[0].attack {
            Some(a) => {
                if seen.last() != Some(&a.power) {
                    seen.push(a.power.clone());
                }
            }
            None => {
                ticks = t;
                break;
            }
        }
    }
    assert_eq!(seen, vec!["HammerNeutral", "HammerNeutralCombo", "HammerNeutralMiss"], "{seen:?}");
    // 12 ticks do primeiro golpe + 10 do segundo + 1 + (13 + 3) da recuperação do erro
    assert!((36..=42).contains(&ticks), "a cadeia durou {ticks} ticks");
    // recarga: não pode atacar imediatamente
    assert!(w.actors[0].attack_cooldown > 0);
    w.step(&press_attack());
    assert!(w.actors[0].attack.is_none());
}

#[test]
fn a_target_is_hit_once_per_power() {
    let mut w = duel(120.0);
    w.step(&press_attack());
    let mut by_power: HashMap<String, u32> = HashMap::new();
    for _ in 0..80 {
        let power = w.actors[0].attack.as_ref().map(|a| a.power.clone());
        let ev = w.step(&idle());
        if !ev.hits.is_empty() {
            *by_power.entry(power.unwrap_or_default()).or_default() += ev.hits.len() as u32;
        }
    }
    assert!(by_power.values().all(|&n| n == 1), "{by_power:?}");
}

#[test]
fn side_attack_scales_knockback_with_damage_and_skips_the_zero_damage_group() {
    let side = [ActorInput { attack: true, input: Input { move_x: 1.0, ..Default::default() }, ..Default::default() }, ActorInput::default()];
    let hit_at = |pre_damage: f32| {
        let mut w = duel(150.0);
        w.actors[1].damage = pre_damage;
        w.step(&side);
        let h = first_hit(&mut w, 40).expect("o golpe lateral deveria acertar");
        (h.0, h.3, h.4, w.actors[0].attack.as_ref().map(|a| a.power.clone()))
    };
    let (tick, d0, k0, _) = hit_at(0.0);
    let (_, _, k100, _) = hit_at(100.0);
    let (_, _, k200, _) = hit_at(200.0);
    assert_eq!(tick, 12, "o primeiro grupo (dano 0) não conta; o segundo começa no tick 12");
    assert!((d0 - 16.0 * 1.031).abs() < 1e-3, "dano = {d0}");
    assert!(k0 > 56.0 && k100 > k0 && k200 > k100, "{k0} {k100} {k200}");
}

#[test]
fn hitting_switches_immediately_to_the_hit_variant() {
    let side = [ActorInput { attack: true, input: Input { move_x: 1.0, ..Default::default() }, ..Default::default() }, ActorInput::default()];
    let mut w = duel(150.0);
    w.step(&side);
    first_hit(&mut w, 40).expect("acerto");
    assert_eq!(w.actors[0].attack.as_ref().map(|a| a.power.as_str()), Some("HammerSideHit"));
}

#[test]
fn mirrored_attack_hits_a_target_on_the_left() {
    let mut w = flat_world();
    w.actors.push(actor(0.0));
    w.actors.push(actor(-120.0));
    for _ in 0..3 {
        w.step(&idle());
    }
    w.actors[0].fighter.facing = -1;
    w.step(&press_attack());
    first_hit(&mut w, 30).expect("acerto");
    assert!(w.actors[1].fighter.vx < 0.0);
    assert!(w.actors[0].fighter.x <= 0.0, "o avanço também é espelhado");
}

#[test]
fn heavy_attack_charges_while_held_and_scales_with_charge() {
    let heavy = |held_ticks: u32| {
        let mut w = duel(150.0);
        let press = ActorInput { heavy: true, heavy_held: true, ..Default::default() };
        let hold = ActorInput { heavy_held: true, ..Default::default() };
        w.step(&[press, ActorInput::default()]);
        assert_eq!(w.actors[0].attack.as_ref().map(|a| a.power.as_str()), Some("SigCharge"));
        let (mut released_at, mut hit) = (None, None);
        for t in 1..=200u32 {
            let p1 = if t <= held_ticks { hold } else { ActorInput::default() };
            let ev = w.step(&[p1, ActorInput::default()]);
            if released_at.is_none() && w.actors[0].attack.as_ref().is_some_and(|a| a.power == "SigRelease") {
                released_at = Some(t);
            }
            if let Some(x) = ev.hits.first() {
                hit = Some((x.2, x.3));
                break;
            }
        }
        let (dmg, kb) = hit.expect("o golpe pesado deveria acertar");
        (released_at.expect("a carga deveria ser solta"), dmg, kb)
    };
    // soltando cedo: a carga mínima (13 ticks) é respeitada
    let (early, dmg_min, kb_min) = heavy(2);
    assert_eq!(early, 12, "largando cedo, só solta ao completar os 13 ticks mínimos de carga");
    // segurando até o máximo (72 ticks): solta sozinho e bate mais forte
    let (auto, dmg_max, kb_max) = heavy(150);
    assert_eq!(auto, 71, "deveria soltar sozinho no máximo de carga");
    assert!((dmg_max / dmg_min - 1.25 / (1.0 + 0.25 * 13.0 / 72.0)).abs() < 0.01, "{dmg_min} -> {dmg_max}");
    assert!(kb_max > kb_min);
}

#[test]
fn heavy_release_sends_the_target_up_and_forward() {
    let mut w = duel(150.0);
    w.step(&[ActorInput { heavy: true, heavy_held: true, ..Default::default() }, ActorInput::default()]);
    first_hit(&mut w, 80).expect("acerto");
    let f = &w.actors[1].fighter;
    assert!(f.vy < -30.0 && f.vx > 10.0, "v = ({}, {})", f.vx, f.vy);
    assert_eq!(w.actors[1].stun, 28);
}

#[test]
fn recovery_is_shorter_with_more_dexterity() {
    let chain_len = |mods: TimeMods| {
        let mut w = flat_world();
        w.actors.push(Actor::with_kit((0.0, -1.0), params(), stats(), mods, kit()));
        w.actors.push(actor(900.0));
        for _ in 0..3 {
            w.step(&idle());
        }
        w.step(&press_attack());
        (0..200).find(|_| {
            w.step(&idle());
            w.actors[0].attack.is_none()
        })
    };
    let slow = chain_len(TimeMods::default()).unwrap();
    let fast = chain_len(TimeMods { recover: 1.476, air_recover: 1.476, sig_recover: 1.26, min_charge: 1.32 }).unwrap();
    assert!(fast < slow, "{fast} vs {slow}");
}

#[test]
fn side_dodge_moves_and_makes_invulnerable_then_goes_on_cooldown() {
    let mut w = duel(120.0);
    // o alvo (P2) esquiva para a direita enquanto P1 ataca
    let dodge = ActorInput { dodge: true, input: Input { move_x: 1.0, ..Default::default() }, ..Default::default() };
    w.step(&press_attack());
    for _ in 0..4 {
        w.step(&idle());
    }
    let x0 = w.actors[1].fighter.x;
    w.step(&[ActorInput::default(), dodge]);
    assert!(w.actors[1].dodge.is_some());
    assert!(!w.invulnerable(1), "os primeiros ticks ainda são vulneráveis");
    w.step(&idle());
    w.step(&idle());
    assert!(w.invulnerable(1));
    let mut hits = 0;
    for _ in 0..40 {
        hits += w.step(&idle()).hits.len();
    }
    assert_eq!(hits, 0, "a esquiva deveria evitar o golpe");
    assert!(w.actors[1].dodge.is_none());
    let dx = w.actors[1].fighter.x - x0;
    assert!(dx > 150.0 && dx < 400.0, "distância da esquiva = {dx}");
    // recarga de esquiva no chão
    assert!(w.actors[1].dodge_cooldown > 0);
    w.step(&[ActorInput::default(), dodge]);
    assert!(w.actors[1].dodge.is_none(), "não pode esquivar de novo durante a recarga");
}

#[test]
fn spot_dodge_stays_in_place_and_air_dodge_freezes_gravity() {
    let mut w = duel(400.0);
    let spot = ActorInput { dodge: true, ..Default::default() };
    let x0 = w.actors[0].fighter.x;
    w.step(&[spot, ActorInput::default()]);
    assert!(w.actors[0].dodge.is_some());
    for _ in 0..25 {
        w.step(&idle());
    }
    assert!(w.actors[0].dodge.is_none() && (w.actors[0].fighter.x - x0).abs() < 1.0);

    // no ar: a esquiva neutra segura o lutador no lugar
    let mut w = flat_world();
    w.actors.push(actor(0.0));
    w.actors[0].fighter.y = -600.0;
    w.step(&[ActorInput::default()]);
    w.step(&[spot]);
    let y0 = w.actors[0].fighter.y;
    for _ in 0..10 {
        w.step(&[ActorInput::default()]);
    }
    assert!(w.actors[0].dodge.is_some() && (w.actors[0].fighter.y - y0).abs() < 0.5, "y: {} -> {}", y0, w.actors[0].fighter.y);
    // a recarga da esquiva aérea é bem maior que a do chão
    for _ in 0..15 {
        w.step(&[ActorInput::default()]);
    }
    assert!(w.actors[0].dodge_cooldown > 100, "recarga = {}", w.actors[0].dodge_cooldown);
}

#[test]
fn up_dodge_lifts_off_the_ground() {
    let mut w = duel(400.0);
    let up = ActorInput { dodge: true, up: true, ..Default::default() };
    let y0 = w.actors[0].fighter.y;
    w.step(&[up, ActorInput::default()]);
    for _ in 0..10 {
        w.step(&idle());
    }
    assert!(w.actors[0].fighter.y < y0 - 80.0, "subiu {} px", y0 - w.actors[0].fighter.y);
}

#[test]
fn leaving_the_bounds_is_a_ko_and_resets_damage() {
    let mut w = duel(120.0);
    w.bounds = (-200.0, -200.0, 200.0, 200.0);
    w.actors[1].damage = 80.0;
    w.actors[1].fighter.x = 900.0;
    let ev = w.step(&idle());
    assert_eq!(ev.kos, vec![1]);
    assert_eq!(w.actors[1].kos, 1);
    assert_eq!(w.actors[1].damage, 0.0);
}

#[test]
fn stunned_target_can_be_launched_into_a_wall_and_bounce() {
    let mut segs = solid(-5000.0, 0.0, 5000.0, 500.0);
    segs.extend(solid(600.0, -2000.0, 800.0, 0.0));
    let mut w = World::new(Geometry::new(&segs), hammer_powers(), (-5000.0, -5000.0, 5000.0, 5000.0));
    w.actors.push(actor(0.0));
    w.actors.push(actor(150.0));
    for _ in 0..3 {
        w.step(&idle());
    }
    w.actors[1].damage = 150.0;
    w.step(&[ActorInput { attack: true, input: Input { move_x: 1.0, ..Default::default() }, ..Default::default() }, ActorInput::default()]);
    first_hit(&mut w, 40).expect("acerto");
    let mut bounced = false;
    for _ in 0..40 {
        w.step(&idle());
        bounced |= w.actors[1].moved.bounced;
    }
    assert!(bounced, "o alvo em hitstun deveria quicar na parede");
    assert!(w.actors[1].fighter.x < 600.0);
}

#[test]
fn idle_actor_only_runs_when_told() {
    let mut w = duel(400.0);
    for _ in 0..30 {
        w.step(&[run_right(), ActorInput::default()]);
    }
    assert!(w.actors[0].fighter.x > 200.0);
    assert!((w.actors[1].fighter.x - 400.0).abs() < 1.0);
}

#[test]
fn bot_walks_up_to_the_target_and_hits_it() {
    let mut w = duel(900.0);
    let mut bot = Bot::new(7);
    let mut hits = 0;
    for _ in 0..900 {
        let p1 = bot.think(&w, 0, 1);
        hits += w.step(&[p1, ActorInput::default()]).hits.iter().filter(|h| h.0 == 0).count();
        if hits > 0 {
            break;
        }
    }
    assert!(hits > 0, "o bot deveria alcançar e acertar o alvo; terminou em x = {}", w.actors[0].fighter.x);
}

#[test]
fn bot_recovers_to_the_stage_when_knocked_off() {
    // palco estreito; o bot começa no ar, fora dele e abaixo do topo
    let geo = Geometry::new(&solid(-300.0, 0.0, 300.0, 400.0));
    let mut w = World::new(geo, hammer_powers(), (-5000.0, -5000.0, 5000.0, 5000.0));
    w.dodges = dodges();
    w.actors.push(Actor::with_kit((0.0, -1.0), params(), stats(), TimeMods::default(), kit()));
    w.actors.push(actor(100.0));
    w.actors[0].fighter.x = 520.0;
    w.actors[0].fighter.y = -40.0;
    let mut bot = Bot::new(3);
    let mut back = false;
    for _ in 0..600 {
        let p1 = bot.think(&w, 0, 1);
        w.step(&[p1, ActorInput::default()]);
        if w.actors[0].fighter.grounded() {
            back = true;
            break;
        }
    }
    assert!(back && w.actors[0].kos == 0, "o bot não voltou: ({}, {})", w.actors[0].fighter.x, w.actors[0].fighter.y);
}

#[test]
fn two_bots_fight_without_getting_stuck() {
    let mut w = duel(600.0);
    let (mut b0, mut b1) = (Bot::new(11), Bot::new(29));
    let (mut hits0, mut hits1) = (0, 0);
    for _ in 0..3600 {
        let (i0, i1) = (b0.think(&w, 0, 1), b1.think(&w, 1, 0));
        for h in w.step(&[i0, i1]).hits {
            if h.0 == 0 {
                hits0 += 1;
            } else {
                hits1 += 1;
            }
        }
    }
    assert!(hits0 > 0 && hits1 > 0, "em um minuto os dois deveriam se acertar: {hits0} x {hits1}");
    for a in &w.actors {
        assert!(a.fighter.x.is_finite() && a.fighter.y.is_finite());
    }
}
