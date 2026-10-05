//! Testes do movimento e da colisão do lutador.

use bh_assets::stats::SpeedStat;

use crate::{geometry::P, Fighter, Geometry, Input, Kind, Motion, Params, StepMods, Tuning, BODY_HEIGHT};

pub(crate) fn params() -> Params {
    let s = SpeedStat {
        run_speed: 45.94,
        jump_x_impulse: 13.8,
        anim_speed: 1.0,
        acceleration: 10.675,
        friction: 5.276,
        air_run_speed: 39.95,
        air_acceleration: 8.228,
        air_friction: 3.298,
    };
    Params::new(&s, Tuning::default())
}

type Seg = (P, P, Kind, Option<P>);

/// Bloco sólido retangular (topo, lados e fundo).
pub(crate) fn solid(x1: f32, y1: f32, x2: f32, y2: f32) -> Vec<Seg> {
    vec![
        ((x1, y1), (x2, y1), Kind::Hard, None),
        ((x2, y1), (x2, y2), Kind::Hard, None),
        ((x1, y2), (x2, y2), Kind::Hard, None),
        ((x1, y2), (x1, y1), Kind::Hard, None),
    ]
}

/// Chão largo em y = 100 (bloco de -5000..5000 x 100..600) e uma plataforma fina em y = -150.
fn stage() -> Geometry {
    let mut segs = solid(-5000.0, 100.0, 5000.0, 600.0);
    segs.push(((100.0, -150.0), (300.0, -150.0), Kind::Soft, None));
    Geometry::new(&segs)
}

fn run(f: &mut Fighter, p: &Params, g: &Geometry, i: Input, ticks: u32) {
    for _ in 0..ticks {
        f.step(&i, p, g);
    }
}

const NONE: Input = Input { move_x: 0.0, jump: false, down: false };
const RIGHT: Input = Input { move_x: 1.0, jump: false, down: false };
const LEFT: Input = Input { move_x: -1.0, jump: false, down: false };
const JUMP: Input = Input { move_x: 0.0, jump: true, down: false };
const DOWN: Input = Input { move_x: 0.0, jump: false, down: true };

fn on_floor(f: &Fighter, y: f32) -> bool {
    f.grounded() && (f.y - y).abs() < 1.5
}

#[test]
fn falls_and_lands_just_above_the_floor() {
    let (p, g) = (params(), stage());
    let mut f = Fighter::new(50.0, 0.0, &p);
    run(&mut f, &p, &g, NONE, 90);
    assert!(on_floor(&f, 100.0), "y = {}", f.y);
    assert_eq!(f.vy, 0.0);
    // fica parado: não treme nem afunda
    let y = f.y;
    run(&mut f, &p, &g, NONE, 30);
    assert!((f.y - y).abs() < 0.05, "{} -> {}", y, f.y);
}

#[test]
fn friction_is_always_applied_so_net_acceleration_is_accel_minus_friction() {
    let (p, g) = (params(), stage());
    let mut f = Fighter::new(0.0, 99.0, &p);
    run(&mut f, &p, &g, NONE, 3);
    // 1º tick: parte de zero, o atrito não tem o que tirar
    f.step(&RIGHT, &p, &g);
    assert!((f.vx - p.accel * p.t.dt).abs() < 1e-3, "vx = {}", f.vx);
    // do 2º em diante: atrito e depois aceleração
    let before = f.vx;
    f.step(&RIGHT, &p, &g);
    let expected = (p.accel - p.friction) * p.t.dt;
    assert!((f.vx - before - expected).abs() < 1e-3, "ganho = {} (esperado {expected})", f.vx - before);
    let mut ticks = 2;
    while f.vx < p.run_speed - 1e-3 && ticks < 100 {
        f.step(&RIGHT, &p, &g);
        ticks += 1;
    }
    assert!((20..=24).contains(&ticks), "chegou ao máximo em {ticks} ticks");
    // soltando a direção, para com o atrito do chão
    let mut stop = 0;
    while f.vx > 0.0 && stop < 100 {
        f.step(&NONE, &p, &g);
        stop += 1;
    }
    assert_eq!(stop, (p.run_speed / (p.friction * p.t.dt)).ceil() as u32);
}

#[test]
fn jump_budget_is_ground_plus_two_air_jumps() {
    let (p, g) = (params(), stage());
    let mut f = Fighter::new(50.0, 99.0, &p);
    run(&mut f, &p, &g, NONE, 3);
    f.step(&JUMP, &p, &g);
    assert!(!f.grounded() && f.vy < -50.0);
    run(&mut f, &p, &g, NONE, 10);
    f.step(&JUMP, &p, &g);
    assert_eq!(f.air_jumps_left(&p), 1);
    run(&mut f, &p, &g, NONE, 10);
    f.step(&JUMP, &p, &g);
    assert_eq!(f.air_jumps_left(&p), 0);
    run(&mut f, &p, &g, NONE, 10);
    let vy = f.vy;
    f.step(&JUMP, &p, &g);
    assert!(f.vy > vy, "o terceiro pulo no ar não deve acontecer");
    // pousar devolve os pulos
    run(&mut f, &p, &g, NONE, 200);
    assert!(f.grounded());
    assert_eq!(f.air_jumps_left(&p), 2);
}

#[test]
fn extra_gravity_near_the_apex_lowers_the_jump() {
    let (p, g) = (params(), stage());
    let mut f = Fighter::new(50.0, 99.0, &p);
    run(&mut f, &p, &g, NONE, 3);
    let y0 = f.y;
    f.step(&JUMP, &p, &g);
    let mut top = 0.0f32;
    for _ in 0..200 {
        f.step(&NONE, &p, &g);
        top = top.max(y0 - f.y);
        if f.grounded() {
            break;
        }
    }
    // só com gravidade 3,75 o pulo teria ~422 px; com o termo extra fica bem mais baixo
    assert!(top > 250.0 && top < 380.0, "altura do pulo = {top}");
}

#[test]
fn walks_off_a_ledge_and_falls() {
    let p = params();
    let g = Geometry::new(&solid(0.0, 100.0, 400.0, 300.0));
    let mut f = Fighter::new(350.0, 99.0, &p);
    run(&mut f, &p, &g, NONE, 3);
    assert!(f.grounded());
    run(&mut f, &p, &g, RIGHT, 30);
    assert!(!f.grounded() && f.x > 400.0 && f.y > 100.0, "({}, {})", f.x, f.y);
}

#[test]
fn can_stand_with_half_the_body_past_the_edge() {
    // o corpo é uma linha: basta o ponto dos pés estar sobre a plataforma
    let p = params();
    let g = Geometry::new(&solid(0.0, 100.0, 400.0, 300.0));
    let mut f = Fighter::new(399.0, 50.0, &p);
    run(&mut f, &p, &g, NONE, 60);
    assert!(on_floor(&f, 100.0));
    let mut f = Fighter::new(401.0, 50.0, &p);
    run(&mut f, &p, &g, NONE, 30);
    assert!(!f.grounded());
}

#[test]
fn soft_platform_passes_from_below_and_holds_from_above() {
    let (p, g) = (params(), stage());
    let mut f = Fighter::new(200.0, 99.0, &p);
    run(&mut f, &p, &g, NONE, 3);
    f.step(&JUMP, &p, &g);
    run(&mut f, &p, &g, NONE, 8);
    f.step(&JUMP, &p, &g);
    let mut above = false;
    for _ in 0..300 {
        f.step(&NONE, &p, &g);
        above |= f.y < -160.0;
        if f.grounded() {
            break;
        }
    }
    assert!(above, "deveria ter subido através da plataforma fina");
    assert!(on_floor(&f, -150.0), "deveria pousar na plataforma fina: y = {}", f.y);
}

#[test]
fn holding_down_drops_through_a_soft_platform() {
    let (p, g) = (params(), stage());
    let mut f = Fighter::new(200.0, -200.0, &p);
    run(&mut f, &p, &g, NONE, 60);
    assert!(on_floor(&f, -150.0));
    run(&mut f, &p, &g, DOWN, 80);
    assert!(on_floor(&f, 100.0), "y = {}", f.y);
    // em chão duro, ↓ não faz atravessar
    run(&mut f, &p, &g, DOWN, 30);
    assert!(on_floor(&f, 100.0));
}

#[test]
fn head_bonks_on_a_ceiling() {
    let p = params();
    let mut segs = solid(-500.0, 300.0, 500.0, 500.0);
    segs.extend(solid(-500.0, -200.0, 500.0, 100.0)); // teto com o fundo em y = 100
    let g = Geometry::new(&segs);
    let mut f = Fighter::new(0.0, 299.0, &p);
    run(&mut f, &p, &g, NONE, 3);
    f.step(&JUMP, &p, &g);
    let mut min_head = f32::MAX;
    for _ in 0..60 {
        f.step(&NONE, &p, &g);
        min_head = min_head.min(f.y - BODY_HEIGHT);
    }
    assert!(min_head >= 100.0, "a cabeça atravessou o teto: {min_head}");
    assert!(min_head < 110.0, "deveria ter encostado no teto: {min_head}");
}

#[test]
fn a_wall_blocks_running_on_the_ground() {
    let p = params();
    let mut segs = solid(-500.0, 100.0, 500.0, 300.0);
    segs.extend(solid(300.0, -300.0, 400.0, 100.0)); // muro em cima do chão
    let g = Geometry::new(&segs);
    let mut f = Fighter::new(0.0, 99.0, &p);
    run(&mut f, &p, &g, NONE, 3);
    run(&mut f, &p, &g, RIGHT, 120);
    assert!(f.x < 300.0 && f.x > 295.0, "x = {}", f.x);
    assert!(f.grounded());
}

#[test]
fn clings_slides_and_wall_jumps() {
    let p = params();
    // muro alto à direita, sem chão por perto
    let g = Geometry::new(&solid(300.0, -2000.0, 500.0, 4000.0));
    let mut f = Fighter::new(200.0, 0.0, &p);
    let mut touched = false;
    for _ in 0..60 {
        f.step(&RIGHT, &p, &g);
        if f.wall != 0 {
            touched = true;
            break;
        }
    }
    assert!(touched, "deveria agarrar a parede");
    assert_eq!(f.wall, 2, "parede à direita");
    assert_eq!(f.motion(), Motion::WallCling);
    // deslizando, a queda é muito mais lenta que a queda livre
    run(&mut f, &p, &g, RIGHT, 40);
    assert!(f.wall != 0 && f.vy < 20.0, "vy deslizando = {}", f.vy);
    assert_eq!(f.air_jumps_left(&p), 2);
    // pulo de parede: sobe e sai para a esquerda, sem gastar pulos no ar
    let ev = f.step(&Input { jump: true, ..RIGHT }, &p, &g);
    assert!(ev.wall_jumped);
    assert!(f.vy < -40.0 && f.vx < -30.0, "v = ({}, {})", f.vx, f.vy);
    assert_eq!(f.air_jumps_left(&p), 2);
}

#[test]
fn letting_go_of_the_wall_pushes_away() {
    let p = params();
    let g = Geometry::new(&solid(300.0, -2000.0, 500.0, 4000.0));
    let mut f = Fighter::new(200.0, 0.0, &p);
    for _ in 0..60 {
        f.step(&RIGHT, &p, &g);
        if f.wall != 0 {
            break;
        }
    }
    assert!(f.wall != 0);
    f.step(&LEFT, &p, &g);
    assert_eq!(f.wall, 0);
    assert!(f.vx < -20.0 && f.facing == -1, "vx = {}", f.vx);
    run(&mut f, &p, &g, LEFT, 5);
    assert!(f.wall == 0 && f.x < 290.0, "x = {}", f.x);
}

#[test]
fn snaps_onto_a_ledge_when_the_head_is_past_the_corner() {
    let p = params();
    let g = Geometry::new(&solid(300.0, 100.0, 800.0, 600.0));
    // caindo junto ao lado esquerdo do bloco, com os pés pouco abaixo do topo
    let mut f = Fighter::new(280.0, 80.0, &p);
    let mut ok = false;
    for _ in 0..40 {
        f.step(&RIGHT, &p, &g);
        if f.grounded() {
            ok = true;
            break;
        }
    }
    assert!(ok && on_floor(&f, 100.0) && f.x >= 300.0, "({}, {}) grounded={}", f.x, f.y, f.grounded());
}

#[test]
fn fast_fall_is_a_latched_state_with_a_higher_cap() {
    let p = params();
    let g = Geometry::default();
    let mut f = Fighter::new(0.0, 0.0, &p);
    run(&mut f, &p, &g, NONE, 120);
    assert!((f.vy - p.t.max_fall).abs() < 1e-3, "queda normal = {}", f.vy);
    run(&mut f, &p, &g, DOWN, 30);
    assert!(f.fast_fall);
    assert!((f.vy - p.t.fast_fall).abs() < 1e-3, "queda rápida = {}", f.vy);
    // soltando ↓ a velocidade volta aos poucos para o limite normal
    f.step(&NONE, &p, &g);
    assert!(!f.fast_fall && f.vy < p.t.fast_fall && f.vy > p.t.max_fall);
    run(&mut f, &p, &g, NONE, 60);
    assert!((f.vy - p.t.max_fall).abs() < 1e-3);
}

#[test]
fn air_turning_is_damped_at_speed() {
    let p = params();
    let g = Geometry::default();
    let mut f = Fighter::new(0.0, 0.0, &p);
    f.vx = p.air_speed;
    f.step(&LEFT, &p, &g);
    // atrito do ar + aceleração amortecida por (1 - |vx|/50)
    let after_friction = p.air_speed - p.air_friction * p.t.dt;
    let damp = 1.0 - after_friction / 50.0;
    let expected = after_friction - p.air_accel * damp * p.t.dt;
    assert!((f.vx - expected).abs() < 1e-3, "vx = {} (esperado {expected})", f.vx);
}

#[test]
fn stunned_fighter_bounces_off_the_floor_and_cannot_act() {
    let (p, g) = (params(), stage());
    let mut f = Fighter::new(0.0, 0.0, &p);
    f.stunned = true;
    f.vy = 60.0;
    let mut bounced = false;
    for _ in 0..20 {
        let ev = f.step(&Input { move_x: 1.0, jump: true, down: false }, &p, &g);
        if ev.bounced {
            bounced = true;
            break;
        }
    }
    assert!(bounced && f.vy < 0.0, "vy após o quique = {}", f.vy);
    assert_eq!(f.air_jumps_left(&p), 2, "o pulo é ignorado em hitstun");
    assert!(f.vx <= 0.0, "a direção é ignorada em hitstun: vx = {}", f.vx);
}

#[test]
fn power_mods_can_freeze_gravity_and_movement() {
    let p = params();
    let g = Geometry::default();
    let mut f = Fighter::new(0.0, 0.0, &p);
    let mods = StepMods { allow_move: false, cancel_gravity: true, allow_jump: false, in_power: true, ..Default::default() };
    for _ in 0..20 {
        f.step_with(&Input { move_x: 1.0, jump: true, down: true }, &p, &g, &mods);
    }
    assert_eq!((f.vx, f.vy, f.y), (0.0, 0.0, 0.0));
    assert!(!f.fast_fall);
}

/// Regressão: cair rente ao canto pontudo de um chanfro (geometria do Brawlhaven) não pode travar o lutador.
#[test]
fn does_not_get_stuck_on_slanted_corners() {
    let p = params();
    let g = Geometry::new(&[
        ((780.0, 1170.0), (1420.0, 1170.0), Kind::Hard, None),
        ((740.0, 1220.0), (860.0, 1340.0), Kind::Hard, None),
        ((860.0, 1340.0), (1330.0, 1340.0), Kind::Hard, None),
        ((1330.0, 1340.0), (1420.0, 1170.0), Kind::Hard, None),
        ((740.0, 1220.0), (780.0, 1170.0), Kind::Hard, None),
    ]);
    for start_x in [650.0, 690.0, 705.0, 720.0, 735.0, 745.0, 760.0] {
        let mut f = Fighter::new(start_x, 1000.0, &p);
        let mut last = (f.x, f.y);
        for tick in 0..400 {
            f.step(&RIGHT, &p, &g);
            if tick == 300 {
                last = (f.x, f.y);
            }
        }
        let moved = (f.x - last.0).abs() + (f.y - last.1).abs();
        assert!(moved > 1.0, "preso em ({}, {}) partindo de x={start_x}", f.x, f.y);
    }
}
