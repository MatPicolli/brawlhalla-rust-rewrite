//! Desenho da arte de um mapa (fundo + imagens de plataforma), compartilhado entre o visualizador e o jogo.

use std::path::{Component, Path, PathBuf};

use bevy::{prelude::*, sprite::Anchor};
use bh_assets::level::Level;

#[derive(Component)]
pub struct LevelEntity;

#[derive(Component)]
pub struct BackgroundSprite;

/// Resolve `a/b/../c` sem tocar no disco (o AssetServer recusa `..`).
pub fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Cria as entidades do mapa (todas marcadas com `LevelEntity`).
pub fn spawn(commands: &mut Commands, assets: &AssetServer, lv: &Level) {
    // Fundo cobrindo os limites da câmera.
    if let Some(bg) = &lv.background {
        let path = normalize(&Path::new("mapArt/Backgrounds").join(bg));
        let c = lv.camera;
        commands.spawn((
            LevelEntity,
            BackgroundSprite,
            Sprite { image: assets.load(path), custom_size: Some(Vec2::new(c.w, c.h)), ..default() },
            Transform::from_xyz(c.x + c.w / 2.0, -(c.y + c.h / 2.0), -10.0),
        ));
    }

    // Arte do mapa: cada imagem tem origem no canto superior esquerdo, y para baixo (Flash).
    for (i, p) in lv.placed.iter().enumerate() {
        if p.theme.is_some() || p.w == 0.0 || p.h == 0.0 {
            continue;
        }
        let path = normalize(&Path::new("mapArt").join(&lv.asset_dir).join(&p.asset));
        let [a, b, c, d, tx, ty] = p.m.0;
        // Troca de eixo y (Flash -> Bevy): F * M * F.
        let mut m = Mat4::from_cols_array(&[a, -b, 0.0, 0.0, -c, d, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, -ty, 0.0, 1.0]);
        let mirrored = p.w < 0.0;
        if mirrored {
            m *= Mat4::from_translation(Vec3::new(-p.w.abs(), 0.0, 0.0));
        }
        let mut t = Transform::from_matrix(m);
        t.translation.z = i as f32 * 0.01;
        commands.spawn((
            LevelEntity,
            Sprite {
                image: assets.load(path),
                custom_size: Some(Vec2::new(p.w.abs(), p.h)),
                flip_x: mirrored,
                ..default()
            },
            Anchor::TOP_LEFT,
            t,
        ));
    }
}
