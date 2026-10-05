//! Modo `--symbol ARQUIVO.swf SIMBOLO [QUADRO]`: desenha um símbolo vetorial de um SWF do jogo.

use std::path::{Path, PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::view::screenshot::{save_to_disk, Screenshot},
};
use bh_assets::{movie::Movie, shape::TriMesh};

/// Converte a malha do SWF (y para baixo, cores sRGB) para o Bevy (y para cima, cores lineares).
/// `flip_winding`: inverte o enrolamento (usado quando o personagem é espelhado em x pela raiz).
pub fn to_bevy_mesh(m: &TriMesh, flip_winding: bool) -> Mesh {
    let pos: Vec<[f32; 3]> = m.pos.iter().map(|p| [p[0], -p[1], 0.0]).collect();
    let col: Vec<[f32; 4]> = m
        .color
        .iter()
        .map(|c| {
            let l = Color::srgba(c[0], c[1], c[2], c[3]).to_linear();
            [l.red, l.green, l.blue, l.alpha]
        })
        .collect();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    // O espelhamento em y (Flash -> Bevy) inverte o enrolamento dos triângulos; um espelhamento extra em x o restaura.
    let idx: Vec<u32> = if flip_winding {
        m.idx.clone()
    } else {
        m.idx.chunks_exact(3).flat_map(|t| [t[0], t[2], t[1]]).collect()
    };
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

#[derive(Resource)]
struct SymbolMesh {
    mesh: TriMesh,
    screenshot: Option<PathBuf>,
    frames: u32,
}

pub fn run(dir: &Path, file: &str, symbol: &str, frame: usize, screenshot: Option<PathBuf>) -> anyhow::Result<()> {
    let path = if file.starts_with("bones/") || file.starts_with("Gfx_") || file.contains('/') { dir.join(file) } else { dir.join(file) };
    let mut movie = Movie::load(&path)?;
    let mesh = movie
        .symbol_mesh(symbol, frame)
        .ok_or_else(|| anyhow::anyhow!("símbolo '{symbol}' não encontrado em {file}"))?
        .clone();
    println!("{symbol}: {} vértices, {} triângulos", mesh.pos.len(), mesh.idx.len() / 3);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: format!("{symbol}"), resolution: (900, 900).into(), ..default() }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.45, 0.5, 0.55)))
        .insert_resource(SymbolMesh { mesh, screenshot, frames: 0 })
        .add_systems(Startup, setup)
        .add_systems(Update, capture)
        .run();
    Ok(())
}

fn setup(mut commands: Commands, data: Res<SymbolMesh>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for p in &data.mesh.pos {
        min = min.min(Vec2::new(p[0], -p[1]));
        max = max.max(Vec2::new(p[0], -p[1]));
    }
    let size = (max - min).max(Vec2::splat(1.0));
    let center = (min + max) / 2.0;
    println!("bounds {min:?} .. {max:?}; cor[0]={:?}; idx[0..6]={:?}", data.mesh.color.first(), &data.mesh.idx[..6.min(data.mesh.idx.len())]);
    commands.spawn((
        Camera2d,
        Transform::from_xyz(center.x, center.y, 0.0),
        Projection::Orthographic(OrthographicProjection { scale: size.max_element() / 800.0, ..OrthographicProjection::default_2d() }),
    ));
    commands.spawn((Mesh2d(meshes.add(to_bevy_mesh(&data.mesh, false))), MeshMaterial2d(materials.add(ColorMaterial::default()))));
}

fn capture(mut commands: Commands, mut data: ResMut<SymbolMesh>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = data.screenshot.clone() else { return };
    data.frames += 1;
    if data.frames == 20 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if data.frames == 60 {
        exit.write(AppExit::Success);
    }
}
