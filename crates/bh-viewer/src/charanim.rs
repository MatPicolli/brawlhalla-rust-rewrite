//! Modo `--char TRAJE --weapon ARMA [--anim NOME] [--frame N]`: personagem animado com o `.anm` da arma.
//!
//! Teclas: ← → trocam de animação · Espaço pausa · , . passo a passo · ↑ ↓ velocidade · Esc sai.

use std::path::{Path, PathBuf};

use anyhow::Result;
use bevy::{
    prelude::*,
    render::view::screenshot::{save_to_disk, Screenshot},
};

use crate::rig::{Animation, CharArt};

#[derive(Resource)]
struct Viewer {
    art: CharArt,
    anims: Vec<Animation>,
    cur: usize,
    time: f32,
    playing: bool,
    fps: f32,
    shown: Option<(usize, usize)>,
    screenshot: Option<PathBuf>,
    ticks: u32,
}

#[derive(Component)]
struct PartEntity;

pub struct Options {
    pub costume: String,
    pub weapon: String,
    pub anim: Option<String>,
    pub frame: Option<usize>,
    pub screenshot: Option<PathBuf>,
}

pub fn run(dir: &Path, o: Options) -> Result<()> {
    let loaded = CharArt::load(dir, &o.costume, &o.weapon)?;
    let cur = o.anim.as_ref().and_then(|n| loaded.anims.iter().position(|a| a.name.eq_ignore_ascii_case(n))).unwrap_or(0);
    println!("{} animações; começando em '{}'", loaded.anims.len(), loaded.anims[cur].name);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Brawlhalla recomp — personagem".into(), resolution: (1000, 800).into(), ..default() }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.45, 0.5, 0.55)))
        .insert_resource(Viewer {
            art: loaded.art,
            anims: loaded.anims,
            cur,
            time: o.frame.unwrap_or(0) as f32,
            playing: o.frame.is_none(),
            fps: 60.0,
            shown: None,
            screenshot: o.screenshot,
            ticks: 0,
        })
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn((
                Camera2d,
                Transform::from_xyz(0.0, 60.0, 0.0),
                Projection::Orthographic(OrthographicProjection { scale: 0.45, ..OrthographicProjection::default_2d() }),
            ));
        })
        .add_systems(Update, (controls, advance, rebuild, capture).chain())
        .run();
    Ok(())
}

fn controls(keys: Res<ButtonInput<KeyCode>>, mut v: ResMut<Viewer>, mut exit: MessageWriter<AppExit>) {
    let n = v.anims.len();
    if keys.just_pressed(KeyCode::ArrowRight) {
        v.cur = (v.cur + 1) % n;
        v.time = 0.0;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        v.cur = (v.cur + n - 1) % n;
        v.time = 0.0;
    }
    if keys.just_pressed(KeyCode::Space) {
        v.playing = !v.playing;
    }
    if keys.just_pressed(KeyCode::Comma) {
        v.playing = false;
        v.time -= 1.0;
    }
    if keys.just_pressed(KeyCode::Period) {
        v.playing = false;
        v.time += 1.0;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        v.fps = (v.fps * 1.5).min(240.0);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        v.fps = (v.fps / 1.5).max(2.0);
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn advance(time: Res<Time>, mut v: ResMut<Viewer>) {
    if v.playing {
        v.time += time.delta_secs() * v.fps;
    }
}

fn rebuild(
    mut commands: Commands,
    mut v: ResMut<Viewer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    old: Query<Entity, With<PartEntity>>,
    mut windows: Query<&mut Window>,
) {
    let v = &mut *v;
    let n = v.anims[v.cur].frames.len().max(1);
    let fi = (v.time.floor() as i64).rem_euclid(n as i64) as usize;
    if v.shown == Some((v.cur, fi)) {
        return;
    }
    v.shown = Some((v.cur, fi));
    for e in &old {
        commands.entity(e).despawn();
    }
    if let Ok(mut w) = windows.single_mut() {
        w.title = format!("{} — quadro {}/{} — {:.0} fps", v.anims[v.cur].name, fi + 1, n, v.fps);
    }
    let Some(frame) = v.anims[v.cur].frames.get(fi) else { return };
    for d in v.art.draw_frame(frame, false, &mut meshes, &mut materials) {
        commands.spawn((PartEntity, Mesh2d(d.mesh), MeshMaterial2d(d.material), d.transform));
    }
}

fn capture(mut commands: Commands, mut v: ResMut<Viewer>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = v.screenshot.clone() else { return };
    v.ticks += 1;
    if v.ticks == 30 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if v.ticks == 80 {
        exit.write(AppExit::Success);
    }
}
