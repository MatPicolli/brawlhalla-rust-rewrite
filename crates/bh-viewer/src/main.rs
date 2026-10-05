//! Visualizador de mapas do Brawlhalla (arte de `mapArt/` + colisões do `Dynamic.swz`).
//!
//! Teclas: ← → trocam de mapa · roda do mouse = zoom · arrastar = mover · C = colisões · B = fundo · Esc = sair
//! Opções: `--level NOME`, `--screenshot ARQ.png` (salva e sai).

mod charanim;
mod charview;
mod game;
mod levelart;
mod rig;

use levelart::{BackgroundSprite, LevelEntity};

use std::{
    fs,
    path::PathBuf,
};

use anyhow::Result;
use bevy::{
    asset::{AssetPlugin, UnapprovedPathMode},
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    render::view::screenshot::{save_to_disk, Screenshot},
};
use bh_assets::{
    install,
    level::{CollisionKind, Level},
    swz,
};

const SWZ_SECTIONS_DYNAMIC: &str = "Dynamic.swz";

#[derive(Resource)]
struct Levels {
    list: Vec<Level>,
    current: usize,
    dirty: bool,
}

#[derive(Resource)]
struct Opts {
    screenshot: Option<PathBuf>,
}

#[derive(Resource, Default)]
struct View {
    collisions: bool,
    background: bool,
    frames: u32,
    fitted: bool,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();

    let dir = install::locate(None)?;
    if args.iter().any(|a| a == "--play") {
        return game::run(
            &dir,
            game::Options {
                level: arg("--level").unwrap_or_else(|| "Brawlhaven".into()),
                costume: arg("--char").unwrap_or_else(|| "Viking".into()),
                weapon: arg("--weapon").unwrap_or_else(|| "Hammer".into()),
                demo: arg("--demo").unwrap_or_default(),
                screenshot: arg("--screenshot").map(PathBuf::from),
                shot_tick: arg("--shot-tick").and_then(|t| t.parse().ok()).unwrap_or(200),
            },
        );
    }
    if let Some(costume) = arg("--char") {
        return charanim::run(
            &dir,
            charanim::Options {
                costume,
                weapon: arg("--weapon").unwrap_or_else(|| "Hammer".into()),
                anim: arg("--anim"),
                frame: arg("--frame").and_then(|f| f.parse().ok()),
                screenshot: arg("--screenshot").map(PathBuf::from),
            },
        );
    }
    if let Some(i) = args.iter().position(|a| a == "--symbol") {
        let file = args.get(i + 1).ok_or_else(|| anyhow::anyhow!("--symbol ARQUIVO SIMBOLO [QUADRO]"))?;
        let symbol = args.get(i + 2).ok_or_else(|| anyhow::anyhow!("--symbol ARQUIVO SIMBOLO [QUADRO]"))?;
        let frame = args.get(i + 3).and_then(|f| f.parse().ok()).unwrap_or(0);
        return charview::run(&dir, file, symbol, frame, arg("--screenshot").map(PathBuf::from));
    }
    let key = swz::find_key(&fs::read(dir.join("BrawlhallaAir.swf"))?, &fs::read(dir.join("Init.swz"))?)?;
    let sections = swz::decrypt(&fs::read(dir.join(SWZ_SECTIONS_DYNAMIC))?, key)?;
    let mut list = bh_assets::level::parse_levels(&sections);
    list.sort_by(|a, b| a.name.cmp(&b.name));
    let current = arg("--level")
        .and_then(|n| list.iter().position(|l| l.name.eq_ignore_ascii_case(&n)))
        .unwrap_or(0);
    println!("{} mapas carregados", list.len());

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
        .insert_resource(Levels { list, current, dirty: true })
        .insert_resource(Opts { screenshot: arg("--screenshot").map(PathBuf::from) })
        .insert_resource(View { collisions: true, background: true, ..default() })
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(Update, (input, rebuild_level, camera_controls, draw_overlay, screenshot_and_exit).chain())
        .run();
    Ok(())
}

fn input(keys: Res<ButtonInput<KeyCode>>, mut levels: ResMut<Levels>, mut view: ResMut<View>, mut exit: MessageWriter<AppExit>) {
    let n = levels.list.len();
    if n > 0 {
        if keys.just_pressed(KeyCode::ArrowRight) {
            levels.current = (levels.current + 1) % n;
            levels.dirty = true;
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            levels.current = (levels.current + n - 1) % n;
            levels.dirty = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyC) {
        view.collisions = !view.collisions;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        view.background = !view.background;
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn rebuild_level(
    mut commands: Commands,
    mut levels: ResMut<Levels>,
    mut view: ResMut<View>,
    assets: Res<AssetServer>,
    old: Query<Entity, With<LevelEntity>>,
    mut windows: Query<&mut Window>,
) {
    if !levels.dirty {
        return;
    }
    levels.dirty = false;
    view.fitted = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let Some(lv) = levels.list.get(levels.current) else { return };
    if let Ok(mut w) = windows.single_mut() {
        w.title = format!("Brawlhalla recomp — {} ({}/{})", lv.name, levels.current + 1, levels.list.len());
    }
    println!("mapa: {} ({} imagens, {} colisões)", lv.name, lv.placed.len(), lv.collisions.len());

    levelart::spawn(&mut commands, &assets, lv);
}

fn camera_controls(
    mut view: ResMut<View>,
    levels: Res<Levels>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window>,
    mut cam: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut tf, mut proj)) = cam.single_mut() else { return };
    let Projection::Orthographic(ortho) = &mut *proj else { return };
    let Ok(win) = windows.single() else { return };

    if !view.fitted {
        if let Some(lv) = levels.list.get(levels.current) {
            let c = lv.camera;
            if c.w > 0.0 && c.h > 0.0 {
                ortho.scale = (c.w / win.width()).max(c.h / win.height()) * 1.05;                tf.translation.x = c.x + c.w / 2.0;
                tf.translation.y = -(c.y + c.h / 2.0);
            }
        }
        view.fitted = true;
    }
    if scroll.delta.y != 0.0 {
        ortho.scale = (ortho.scale * (1.0 - scroll.delta.y * 0.1)).clamp(0.05, 20.0);
    }
    if mouse.pressed(MouseButton::Left) {
        tf.translation.x -= motion.delta.x * ortho.scale;
        tf.translation.y += motion.delta.y * ortho.scale;
    }
}

fn draw_overlay(mut gizmos: Gizmos, levels: Res<Levels>, view: Res<View>, mut bg: Query<&mut Visibility, With<BackgroundSprite>>) {
    for mut v in &mut bg {
        *v = if view.background { Visibility::Inherited } else { Visibility::Hidden };
    }
    let Some(lv) = levels.list.get(levels.current) else { return };
    let c = lv.camera;
    gizmos.rect_2d(Isometry2d::from_translation(Vec2::new(c.x + c.w / 2.0, -(c.y + c.h / 2.0))), Vec2::new(c.w, c.h), Color::srgba(1.0, 1.0, 1.0, 0.25));
    if !view.collisions {
        return;
    }
    for col in &lv.collisions {
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
    for &(x, y) in &lv.respawns {
        gizmos.circle_2d(Isometry2d::from_translation(Vec2::new(x, -y)), 12.0, Color::srgb(0.2, 0.9, 1.0));
    }
}

fn screenshot_and_exit(mut commands: Commands, opts: Res<Opts>, mut view: ResMut<View>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = &opts.screenshot else { return };
    view.frames += 1;
    // Espera os PNG/JPG carregarem antes de capturar.
    if view.frames == 240 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
    }
    if view.frames == 300 {
        exit.write(AppExit::Success);
    }
}
