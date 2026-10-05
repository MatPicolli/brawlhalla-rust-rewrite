//! Modo `--play [--level NOME] [--char TRAJE] [--weapon ARMA]`: duelo local contra um bot, um alvo parado ou
//! um segundo jogador.
//!
//! P1: A/D andar · W ou Espaço pular (chão + 2 no ar; W também é "cima") · S baixo (queda rápida, atravessar
//!     plataforma fina) · J golpe leve · K golpe pesado (segure para carregar) · L esquiva.
//! P2 (modo humano): ←/→ andar · ↑ pular/cima · ↓ baixo · , golpe leve · . golpe pesado · / esquiva
//!     (ou 1, 2, 3 do teclado numérico).
//! Gerais: P alterna o P2 (bot / parado / humano) · R reinicia · C liga/desliga colisões e hitboxes · Esc sai.
//! Gamepads: o primeiro controla P1 e o segundo P2 (analógico, A pula, X leve, B pesado, gatilhos esquivam).
//!
//! A simulação fica em `bh-sim`; poderes, dano, knockback, stun, esquiva e movimento vêm dos dados e do bytecode.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use bevy::{
    asset::{AssetPlugin, UnapprovedPathMode},
    prelude::*,
    render::view::screenshot::{save_to_disk, Screenshot},
};
use bh_assets::{
    dodge,
    level::{self, CollisionKind, Level},
    powers::PowerTable,
    stats, swz,
};
use bh_sim::{
    bot::Bot,
    combat::{CombatStats, HURTBOX_H, HURTBOX_W},
    world::{Actor, ActorInput, World},
    Geometry, Input, Motion, Params, Tuning, BODY_HEIGHT,
};

use crate::{
    levelart,
    rig::{Animation, CharArt},
};

pub struct Options {
    pub level: String,
    pub costume: String,
    pub weapon: String,
    /// "" (jogo normal), "script" (P1 roteirizado, P2 parado) ou "bots" (dois bots).
    pub demo: String,
    pub screenshot: Option<PathBuf>,
    pub shot_tick: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum P2Mode {
    Bot,
    Still,
    Human,
}

/// Entrada de um jogador amostrada em Update e consumida em FixedUpdate.
#[derive(Default, Clone, Copy)]
struct Pad {
    x: f32,
    down: bool,
    up: bool,
    heavy_held: bool,
    jump: bool,
    attack: bool,
    heavy: bool,
    dodge: bool,
}

impl Pad {
    fn take(&mut self) -> ActorInput {
        let inp = ActorInput {
            input: Input { move_x: self.x, jump: self.jump, down: self.down },
            attack: self.attack,
            heavy: self.heavy,
            heavy_held: self.heavy_held,
            dodge: self.dodge,
            up: self.up,
        };
        self.jump = false;
        self.attack = false;
        self.heavy = false;
        self.dodge = false;
        inp
    }
}

/// Estado de exibição de um lutador.
#[derive(Default, Clone)]
struct View {
    anim: usize,
    frame: usize,
    /// Ticks desde que a animação atual começou.
    anim_ticks: f32,
    shown: Option<(usize, usize, i8)>,
    root: Option<Entity>,
    since_hit: u32,
    since_land: u32,
}

#[derive(Resource)]
struct Game {
    level: Level,
    world: World,
    art: CharArt,
    anims: Vec<Animation>,
    views: Vec<View>,
    pads: [Pad; 2],
    p2: P2Mode,
    bots: [Bot; 2],
    debug: bool,
    last_hit: String,
    /// Tremor da câmera: (amplitude em px de mundo, ticks restantes).
    shake: (f32, u32),
    demo: String,
    screenshot: Option<PathBuf>,
    shot_tick: u32,
    ticks: u32,
}

#[derive(Component)]
struct PartEntity(usize);

#[derive(Component)]
struct Hud(usize);

#[derive(Component)]
struct InfoText;

impl Game {
    fn anim_index(&self, name: &str) -> Option<usize> {
        self.anims.iter().position(|a| a.name == name)
    }

    fn first_anim(&self, names: &[&str]) -> usize {
        names.iter().find_map(|n| self.anim_index(n)).unwrap_or(0)
    }

    /// Animação e quadro (0-based) de um lutador neste tick. `None` no quadro = usar o tempo da animação.
    fn pick_anim(&self, i: usize) -> (usize, Option<usize>, bool) {
        let a = &self.world.actors[i];
        let v = &self.views[i];
        if let Some(atk) = &a.attack {
            if let Some(idx) = self.anim_index(&atk.anim) {
                return (idx, Some((atk.anim_frame.floor().max(1.0) - 1.0) as usize), false);
            }
        }
        if let Some(d) = &a.dodge {
            let airborne = !a.fighter.grounded();
            let moving = d.vx != 0.0 || d.vy != 0.0;
            let idx = if airborne {
                self.first_anim(&["DodgeAir", "DodgeSpot", "Ready"])
            } else if moving {
                self.first_anim(&["DodgeRoll", "DodgeSpot", "Ready"])
            } else {
                self.first_anim(&["DodgeSpot", "Ready"])
            };
            let n = self.anims[idx].frames.len().max(1);
            let dur = self.world.dodges.get(d.ty).map(|t| t.duration).unwrap_or(14).max(1);
            return (idx, Some((d.tick as usize * n / dur as usize).min(n - 1)), false);
        }
        if a.stun > 0 {
            let idx = if a.last_knockback > 35.0 { self.first_anim(&["HitReactLaunch", "HitReact", "Ready"]) } else { self.first_anim(&["HitReact", "Ready"]) };
            return (idx, Some(v.since_hit as usize), false);
        }
        match a.fighter.motion() {
            Motion::WallCling => (self.first_anim(&["WallCling", "Fall", "Ready"]), None, true),
            Motion::Rising => (self.first_anim(&["Jump", "Ready"]), None, false),
            Motion::Falling => {
                let names: &[&str] = if a.fighter.fast_fall {
                    &["FallFast", "Fall", "Ready"]
                } else if a.fighter.air_jumps_used > 0 {
                    &["JumpDown", "Fall", "Ready"]
                } else {
                    &["Fall", "JumpDown", "Ready"]
                };
                (self.first_anim(names), None, true)
            }
            Motion::Run => (self.first_anim(&["Run", "Ready"]), None, true),
            Motion::Idle => {
                let land = self.first_anim(&["JumpLand", "Ready"]);
                if (v.since_land as usize) < self.anims[land].frames.len() / 2 && self.anims[land].name == "JumpLand" {
                    (land, Some(v.since_land as usize * 2), false)
                } else {
                    (self.first_anim(&["Ready"]), None, true)
                }
            }
        }
    }

    fn reset(&mut self) {
        for a in &mut self.world.actors {
            a.respawn();
            a.kos = 0;
        }
        self.world.actors[1].fighter.facing = -1;
        self.last_hit.clear();
    }
}

pub fn run(dir: &Path, o: Options) -> Result<()> {
    let key = swz::find_key(&fs::read(dir.join("BrawlhallaAir.swf"))?, &fs::read(dir.join("Init.swz"))?)?;
    let dynamic = swz::decrypt(&fs::read(dir.join("Dynamic.swz"))?, key)?;
    let lv = level::parse_levels(&dynamic)
        .into_iter()
        .find(|l| l.name.eq_ignore_ascii_case(&o.level))
        .ok_or_else(|| anyhow!("mapa '{}' não encontrado", o.level))?;

    let loaded = CharArt::load(dir, &o.costume, &o.weapon)?;
    let game = &loaded.game_sections;
    let levels = stats::hero_levels(game, &loaded.hero)?;
    let combat = CombatStats { strength: levels.strength, defense: levels.weight, impulse_mult: stats::strength_impulse_mult(game, levels.strength)? };
    let time_mods = dodge::time_mods(game, levels.dexterity)?;
    let params = Params::new(&stats::hero_speed_stat(game, &loaded.hero)?, Tuning::default());
    let kit = [1, 2]
        .iter()
        .filter_map(|&s| stats::hero_weapon_kit(game, &loaded.hero, s).ok())
        .find(|k| k.weapon.eq_ignore_ascii_case(&o.weapon))
        .unwrap_or_else(|| stats::WeaponKit { weapon: o.weapon.clone(), ..Default::default() });
    println!("{}: {levels:?}; {combat:?}; {time_mods:?}", loaded.hero);
    println!("arma {} com golpes pesados {} / {} / {}", kit.weapon, kit.sig_neutral, kit.sig_side, kit.sig_down);

    let geo = Geometry::from_level(&lv);
    let powers = PowerTable::load(game);
    let c = lv.camera;
    let (ml, mt, mr, mb) = level::kill_margins(&loaded.init_sections, &lv.name).unwrap_or((0.0, 0.0, 0.0, 0.0));
    println!("{} poderes; mapa {} com {} superfícies; KO a {ml}/{mt}/{mr}/{mb} px além da câmera", powers.len(), lv.name, geo.surfaces.len());
    let mut world = World::new(geo, powers, (c.x - ml, c.y - mt, c.x + c.w + mr, c.y + c.h + mb));
    world.dodges = dodge::dodge_types(game)?;

    let spawn = lv.respawns.first().copied().unwrap_or((c.x + c.w / 2.0, c.y + 300.0));
    world.actors.push(Actor::with_kit(spawn, params, combat, time_mods, kit.clone()));
    world.actors.push(Actor::with_kit((spawn.0 + 220.0, spawn.1), params, combat, time_mods, kit));
    world.actors[1].fighter.facing = -1;

    println!("P1: A/D, W/Espaço pula, S baixo, J leve, K pesado, L esquiva. P alterna o P2 (bot/parado/humano). R reinicia. C debug.");
    let p2 = if o.demo == "script" { P2Mode::Still } else { P2Mode::Bot };
    let state = Game {
        level: lv,
        world,
        art: loaded.art,
        anims: loaded.anims,
        views: vec![View::default(), View::default()],
        pads: [Pad::default(); 2],
        p2,
        bots: [Bot::new(0x51f1_5eed), Bot::new(0x0bad_cafe)],
        debug: true,
        last_hit: String::new(),
        shake: (0.0, 0),
        demo: o.demo,
        screenshot: o.screenshot,
        shot_tick: o.shot_tick,
        ticks: 0,
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: dir.to_string_lossy().into_owned(),
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window { title: "Brawlhalla recomp".into(), resolution: (1280, 720).into(), ..default() }),
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(0.07, 0.07, 0.09)))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(state)
        .add_systems(Startup, setup)
        .add_systems(Update, (read_input, render_characters, follow_camera, draw_debug, hud, capture).chain())
        .add_systems(FixedUpdate, sim_step)
        .run();
    Ok(())
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut game: ResMut<Game>) {
    let start = game.world.actors[0].spawn;
    commands.spawn((Camera2d, Transform::from_xyz(start.0, -start.1, 0.0)));
    levelart::spawn(&mut commands, &assets, &game.level);
    for i in 0..game.world.actors.len() {
        let e = commands.spawn((Transform::from_xyz(0.0, 0.0, 10.0 + i as f32), Visibility::default())).id();
        game.views[i].root = Some(e);
        commands.spawn((
            Hud(i),
            Text::new("0%"),
            TextFont { font_size: bevy::text::FontSize::Px(40.0), ..default() },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(18.0),
                left: if i == 0 { Val::Px(60.0) } else { Val::Auto },
                right: if i == 1 { Val::Px(60.0) } else { Val::Auto },
                ..default()
            },
        ));
    }
    commands.spawn((
        InfoText,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(16.0), ..default() },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        Node { position_type: PositionType::Absolute, top: Val::Px(8.0), left: Val::Px(12.0), ..default() },
    ));
}

fn read_input(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut game: ResMut<Game>, mut exit: MessageWriter<AppExit>) {
    use KeyCode::*;
    let any = |ks: &[KeyCode]| ks.iter().any(|k| keys.pressed(*k));
    let hit = |ks: &[KeyCode]| ks.iter().any(|k| keys.just_pressed(*k));
    let axis = |l: &[KeyCode], r: &[KeyCode]| (any(r) as i32 - any(l) as i32) as f32;

    let mut p = [Pad::default(); 2];
    // P1: WASD + J/K/L
    p[0].x = axis(&[KeyA], &[KeyD]);
    p[0].down = any(&[KeyS]);
    p[0].up = any(&[KeyW]);
    p[0].jump = hit(&[KeyW, Space]);
    p[0].attack = hit(&[KeyJ]);
    p[0].heavy = hit(&[KeyK]);
    p[0].heavy_held = any(&[KeyK]);
    p[0].dodge = hit(&[KeyL]);
    // P2: setas + , . /  (ou 1 2 3 do teclado numérico)
    p[1].x = axis(&[ArrowLeft], &[ArrowRight]);
    p[1].down = any(&[ArrowDown]);
    p[1].up = any(&[ArrowUp]);
    p[1].jump = hit(&[ArrowUp]);
    p[1].attack = hit(&[Comma, Numpad1]);
    p[1].heavy = hit(&[Period, Numpad2]);
    p[1].heavy_held = any(&[Period, Numpad2]);
    p[1].dodge = hit(&[Slash, Numpad3]);
    // gamepads: o primeiro é P1, o segundo P2
    for (i, pad) in pads.iter().take(2).enumerate() {
        let stick = pad.left_stick();
        if stick.x.abs() > 0.3 {
            p[i].x = stick.x.signum();
        }
        p[i].down |= stick.y < -0.6 || pad.pressed(GamepadButton::DPadDown);
        p[i].up |= stick.y > 0.6 || pad.pressed(GamepadButton::DPadUp);
        p[i].jump |= pad.just_pressed(GamepadButton::South);
        p[i].attack |= pad.just_pressed(GamepadButton::West);
        p[i].heavy |= pad.just_pressed(GamepadButton::East) || pad.just_pressed(GamepadButton::North);
        p[i].heavy_held |= pad.pressed(GamepadButton::East) || pad.pressed(GamepadButton::North);
        p[i].dodge |= pad.just_pressed(GamepadButton::RightTrigger) || pad.just_pressed(GamepadButton::LeftTrigger) || pad.just_pressed(GamepadButton::RightTrigger2);
    }
    for i in 0..2 {
        let g = &mut game.pads[i];
        g.x = p[i].x;
        g.down = p[i].down;
        g.up = p[i].up;
        g.heavy_held = p[i].heavy_held;
        g.jump |= p[i].jump;
        g.attack |= p[i].attack;
        g.heavy |= p[i].heavy;
        g.dodge |= p[i].dodge;
    }
    if keys.just_pressed(KeyC) {
        game.debug = !game.debug;
    }
    if keys.just_pressed(KeyP) {
        game.p2 = match game.p2 {
            P2Mode::Bot => P2Mode::Still,
            P2Mode::Still => P2Mode::Human,
            P2Mode::Human => P2Mode::Bot,
        };
    }
    if keys.just_pressed(KeyR) {
        game.reset();
    }
    if keys.just_pressed(Escape) {
        exit.write(AppExit::Success);
    }
}

/// P1 roteirizado (`--demo script`): golpe neutro, lateral, pesado carregado, esquiva e pulos.
fn script_input(tick: u32) -> ActorInput {
    let none = ActorInput::default();
    let right = Input { move_x: 1.0, ..Default::default() };
    match tick {
        60 => ActorInput { attack: true, ..none },
        150 => ActorInput { attack: true, input: right, ..none },
        230 => ActorInput { heavy: true, heavy_held: true, ..none },
        231..=260 => ActorInput { heavy_held: true, ..none },
        340 => ActorInput { dodge: true, input: right, ..none },
        400 => ActorInput { input: Input { jump: true, ..Default::default() }, ..none },
        420 => ActorInput { input: Input { jump: true, ..Default::default() }, ..none },
        440 => ActorInput { attack: true, ..none },
        _ => none,
    }
}

fn sim_step(mut game: ResMut<Game>) {
    let g = &mut *game;
    let inputs = match g.demo.as_str() {
        "script" => [script_input(g.ticks), ActorInput::default()],
        "bots" => [g.bots[0].think(&g.world, 0, 1), g.bots[1].think(&g.world, 1, 0)],
        _ => {
            let p1 = g.pads[0].take();
            let p2 = match g.p2 {
                P2Mode::Bot => g.bots[1].think(&g.world, 1, 0),
                P2Mode::Still => ActorInput::default(),
                P2Mode::Human => g.pads[1].take(),
            };
            [p1, p2]
        }
    };
    let ev = g.world.step(&inputs);
    g.ticks += 1;
    g.shake.1 = g.shake.1.saturating_sub(1);

    for (i, v) in g.views.iter_mut().enumerate() {
        v.since_hit += 1;
        v.since_land += 1;
        if g.world.actors[i].moved.landed {
            v.since_land = 0;
        }
    }
    for &(a, t, dmg, kb) in &ev.hits {
        g.last_hit = format!("P{} acertou P{}: dano {:.1}, knockback {:.1}", a + 1, t + 1, dmg, kb);
        println!("tick {}: {} (dano do alvo {:.1}%)", g.ticks, g.last_hit, g.world.actors[t].damage);
        g.views[t].since_hit = 0;
        // tremor proporcional ao golpe, nos três degraus que o jogo usa
        let amp = if kb >= 50.0 { 9.0 } else if kb >= 25.0 { 6.0 } else { 3.0 };
        g.shake = (amp, 8);
    }
    for &i in &ev.kos {
        g.last_hit = format!("KO de P{}", i + 1);
        println!("tick {}: {}", g.ticks, g.last_hit);
        g.shake = (14.0, 14);
    }

    // animação mostrada, em ticks de simulação (sincronizada com os poderes)
    for i in 0..g.views.len() {
        let (anim, frame, looping) = g.pick_anim(i);
        let speed = g.world.actors[i].params.anim_speed;
        let n = g.anims[anim].frames.len().max(1);
        let v = &mut g.views[i];
        if v.anim != anim {
            v.anim = anim;
            v.anim_ticks = 0.0;
        } else {
            v.anim_ticks += speed;
        }
        let f = frame.unwrap_or(v.anim_ticks as usize);
        v.frame = if looping { f % n } else { f.min(n - 1) };
    }
}

fn render_characters(
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut roots: Query<&mut Transform>,
    old: Query<(Entity, &PartEntity)>,
) {
    let g = &mut *game;
    for i in 0..g.views.len() {
        let Some(root_e) = g.views[i].root else { continue };
        let a = &g.world.actors[i];
        if let Ok(mut rt) = roots.get_mut(root_e) {
            rt.translation.x = a.fighter.x;
            rt.translation.y = -a.fighter.y;
            rt.scale.x = a.fighter.facing as f32;
        }
        let state = (g.views[i].anim, g.views[i].frame, a.fighter.facing);
        if g.views[i].shown == Some(state) {
            continue;
        }
        g.views[i].shown = Some(state);
        for (e, p) in &old {
            if p.0 == i {
                commands.entity(e).despawn();
            }
        }
        let Some(frame) = g.anims[state.0].frames.get(state.1) else { continue };
        for d in g.art.draw_frame(frame, state.2 < 0, &mut meshes, &mut materials) {
            commands.entity(root_e).with_child((PartEntity(i), Mesh2d(d.mesh), MeshMaterial2d(d.material), d.transform));
        }
    }
}

fn follow_camera(time: Res<Time>, game: Res<Game>, windows: Query<&Window>, mut cam: Query<(&mut Transform, &mut Projection), With<Camera2d>>) {
    let Ok((mut tf, mut proj)) = cam.single_mut() else { return };
    let Projection::Orthographic(ortho) = &mut *proj else { return };
    let Ok(win) = windows.single() else { return };
    let c = game.level.camera;
    let (a, b) = (&game.world.actors[0].fighter, &game.world.actors[1].fighter);
    // enquadra os dois lutadores (limitados aos limites da câmera do mapa)
    let clampx = |x: f32| x.clamp(c.x, c.x + c.w);
    let clampy = |y: f32| y.clamp(c.y, c.y + c.h);
    let (ax, ay, bx, by) = (clampx(a.x), clampy(a.y), clampx(b.x), clampy(b.y));
    let aspect = win.width() / win.height();
    // nunca mostra além dos limites da câmera do mapa (nem em altura nem em largura)
    let max_h = c.h.min(c.w / aspect).max(1000.0);
    let want_h = ((ay - by).abs() + 700.0).max(((ax - bx).abs() + 700.0) / aspect).clamp(1000.0, max_h);
    let k = 1.0 - (-6.0 * time.delta_secs()).exp();
    ortho.scale += (want_h / win.height() - ortho.scale) * k;
    let (vw, vh) = (win.width() * ortho.scale, win.height() * ortho.scale);

    let target = Vec2::new((ax + bx) / 2.0, -((ay + by) / 2.0 - 100.0));
    let (min_x, max_x) = (c.x + vw / 2.0, c.x + c.w - vw / 2.0);
    let (min_y, max_y) = (-(c.y + c.h) + vh / 2.0, -c.y - vh / 2.0);
    let goal = Vec2::new(
        if min_x < max_x { target.x.clamp(min_x, max_x) } else { c.x + c.w / 2.0 },
        if min_y < max_y { target.y.clamp(min_y, max_y) } else { -(c.y + c.h / 2.0) },
    );
    tf.translation.x += (goal.x - tf.translation.x) * k;
    tf.translation.y += (goal.y - tf.translation.y) * k;
    // tremor do golpe
    if game.shake.1 > 0 {
        let t = game.ticks as f32;
        let amp = game.shake.0 * game.shake.1 as f32 / 8.0;
        tf.translation.x += (t * 2.3).sin() * amp;
        tf.translation.y += (t * 3.1).cos() * amp;
    }
}

fn draw_debug(mut gizmos: Gizmos, game: Res<Game>) {
    if !game.debug {
        return;
    }
    for col in &game.level.collisions {
        let color = match col.kind {
            CollisionKind::Hard => Color::srgb(1.0, 0.25, 0.25),
            CollisionKind::Soft => Color::srgb(0.3, 1.0, 0.4),
            CollisionKind::NoSlide => Color::srgb(1.0, 0.6, 0.1),
            CollisionKind::Bouncy => Color::srgb(0.4, 0.7, 1.0),
            CollisionKind::Lava => Color::srgb(1.0, 0.1, 0.8),
            CollisionKind::Other => Color::srgb(1.0, 1.0, 0.3),
        };
        gizmos.line_2d(Vec2::new(col.x1, -col.y1), Vec2::new(col.x2, -col.y2), color);
    }
    // normais das superfícies (apontam para fora do sólido)
    for s in &game.world.geo.surfaces {
        let m = Vec2::new((s.a.0 + s.b.0) / 2.0, -(s.a.1 + s.b.1) / 2.0);
        gizmos.line_2d(m, m + Vec2::new(s.n.0, -s.n.1) * 18.0, Color::srgba(1.0, 1.0, 1.0, 0.35));
    }
    for (i, a) in game.world.actors.iter().enumerate() {
        let (cx, cy) = a.center();
        let col = if game.world.invulnerable(i) {
            Color::srgba(0.4, 0.8, 1.0, 0.9)
        } else if a.stun > 0 {
            Color::srgba(1.0, 0.3, 0.3, 0.9)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.45)
        };
        // hurtbox (golpes) e linha do corpo (colisão com o mapa)
        gizmos.rect_2d(Isometry2d::from_translation(Vec2::new(cx, -cy)), Vec2::new(HURTBOX_W, HURTBOX_H), col);
        gizmos.line_2d(Vec2::new(a.fighter.x, -a.fighter.y), Vec2::new(a.fighter.x, -(a.fighter.y - BODY_HEIGHT)), Color::srgb(0.2, 1.0, 1.0));
        for e in game.world.active_boxes(i) {
            gizmos.ellipse_2d(Isometry2d::from_translation(Vec2::new(e.cx, -e.cy)), Vec2::new(e.rx, e.ry), Color::srgb(1.0, 0.85, 0.1));
        }
    }
    // zona de KO
    let (x1, y1, x2, y2) = game.world.bounds;
    gizmos.rect_2d(Isometry2d::from_translation(Vec2::new((x1 + x2) / 2.0, -(y1 + y2) / 2.0)), Vec2::new(x2 - x1, y2 - y1), Color::srgba(1.0, 0.2, 0.9, 0.5));
}

fn hud(game: Res<Game>, mut windows: Query<&mut Window>, mut texts: Query<(&mut Text, &mut TextColor, &Hud), Without<InfoText>>, mut info: Query<&mut Text, With<InfoText>>) {
    for (mut text, mut color, h) in &mut texts {
        let a = &game.world.actors[h.0];
        *text = Text::new(format!("P{}  {:.0}%   KO {}", h.0 + 1, a.damage, a.kos));
        // do branco ao vermelho conforme o dano
        let k = (a.damage / 200.0).clamp(0.0, 1.0);
        color.0 = Color::srgb(1.0, 1.0 - 0.8 * k, 1.0 - 0.9 * k);
    }
    let p1 = &game.world.actors[0];
    let state = if let Some(atk) = &p1.attack {
        format!("{} (tick {})", atk.power, atk.tick)
    } else if p1.dodge.is_some() {
        "esquiva".to_string()
    } else {
        format!("{:?}", p1.fighter.motion())
    };
    if let Ok(mut t) = info.single_mut() {
        *t = Text::new(format!(
            "P2: {:?} (P alterna) | P1: {} | esquiva em {} | {}",
            game.p2, state, p1.dodge_cooldown, game.last_hit
        ));
    }
    if let Ok(mut w) = windows.single_mut() {
        w.title = format!("Brawlhalla recomp — {} — P1 {:.0}% x P2 {:.0}%", game.level.name, p1.damage, game.world.actors[1].damage);
    }
}

fn capture(mut commands: Commands, game: Res<Game>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = game.screenshot.clone() else { return };
    if game.ticks == game.shot_tick {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if game.ticks >= game.shot_tick + 45 {
        exit.write(AppExit::Success);
    }
}
