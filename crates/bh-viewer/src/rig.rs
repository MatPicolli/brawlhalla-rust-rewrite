//! Arte de um personagem montada a partir do traje + arma + animações do `.anm`, com caches de malhas.

use std::{collections::HashMap, fs, path::Path};

use anyhow::{anyhow, Result};
use bevy::{prelude::*, sprite_render::AlphaMode2d};
use bh_assets::{
    anm::{self, Frame},
    bones,
    character::{self, Layer, Library},
    swz, zblob,
};

use crate::charview::to_bevy_mesh;

pub struct Animation {
    pub name: String,
    pub frames: Vec<Frame>,
}

pub struct CharArt {
    lib: Library,
    names: Vec<String>,
    layers: Vec<Layer>,
    resolved: HashMap<u16, Option<(String, String)>>,
    meshes: HashMap<(String, String, usize, bool), Option<Handle<Mesh>>>,
    mats: HashMap<u8, Handle<ColorMaterial>>,
}

pub struct PartDraw {
    pub mesh: Handle<Mesh>,
    pub material: Handle<ColorMaterial>,
    pub transform: Transform,
}

pub struct Loaded {
    pub art: CharArt,
    pub anims: Vec<Animation>,
    pub hero: String,
    pub game_sections: Vec<Vec<u8>>,
    pub init_sections: Vec<Vec<u8>>,
}

impl CharArt {
    /// Carrega traje (`costume`) e arma (`weapon`, p.ex. "Hammer") e as animações da classe da arma.
    pub fn load(dir: &Path, costume: &str, weapon: &str) -> Result<Loaded> {
        let key = swz::find_key(&fs::read(dir.join("BrawlhallaAir.swf"))?, &fs::read(dir.join("Init.swz"))?)?;
        let init = swz::decrypt(&fs::read(dir.join("Init.swz"))?, key)?;
        let game = swz::decrypt(&fs::read(dir.join("Game.swz"))?, key)?;
        let names = bones::bone_names(&init)?;

        let costumes = character::costumes(&game);
        let c = costumes
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(costume))
            .ok_or_else(|| anyhow!("traje '{costume}' não encontrado ({} trajes)", costumes.len()))?;
        let skins = character::weapon_skins(&game);
        let skin = skins.iter().find(|s| s.weapon.eq_ignore_ascii_case(weapon) && s.hero == c.hero);
        let mut layers = c.layers.clone();
        if let Some(s) = skin {
            layers.extend(s.layers.clone());
        }
        println!("traje {} (herói {}), arma {weapon}, skin {:?}", c.name, c.hero, skin.map(|s| &s.name));

        let file = anm::parse(&zblob::decode(&fs::read(dir.join("anims").join(format!("Animation_{weapon}.anm")))?)?)?;
        let class = format!("a__{weapon}Animation");
        let entry = file
            .entries
            .into_iter()
            .find(|e| e.class == class)
            .ok_or_else(|| anyhow!("classe {class} não encontrada em Animation_{weapon}.anm"))?;
        let anims = entry.anims.into_iter().map(|a| Animation { name: a.name, frames: a.frames }).collect();

        let art = CharArt { lib: Library::new(dir), names, layers, resolved: HashMap::new(), meshes: HashMap::new(), mats: HashMap::new() };
        Ok(Loaded { art, anims, hero: c.hero.clone(), game_sections: game, init_sections: init })
    }

    /// Partes visíveis de um quadro. Com `mirrored` o personagem olha para a esquerda: o chamador aplica
    /// escala x = -1 na raiz e as malhas daqui já vêm com o enrolamento dos triângulos invertido.
    pub fn draw_frame(
        &mut self,
        frame: &Frame,
        mirrored: bool,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<ColorMaterial>,
    ) -> Vec<PartDraw> {
        let mut out = Vec::with_capacity(frame.parts.len());
        for (i, p) in frame.parts.iter().enumerate() {
            let name = self.names.get(p.bone as usize).cloned().unwrap_or_default();
            let layers = &self.layers;
            let lib = &mut self.lib;
            let res = self.resolved.entry(p.bone).or_insert_with(|| lib.resolve(&name, layers)).clone();
            let Some((file, sym)) = res else { continue };
            let sprite_frame = (p.frame_no.max(1) - 1) as usize;
            let handle = self
                .meshes
                .entry((file.clone(), sym.clone(), sprite_frame, mirrored))
                .or_insert_with(|| {
                    let m = lib.mesh(&file, &sym, sprite_frame)?;
                    (!m.is_empty()).then(|| meshes.add(to_bevy_mesh(&m, mirrored)))
                })
                .clone();
            let Some(mesh) = handle else { continue };

            let [a, b, c, d, tx, ty] = p.m;
            let mut transform =
                Transform::from_matrix(Mat4::from_cols_array(&[a, -b, 0.0, 0.0, -c, d, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, -ty, 0.0, 1.0]));
            transform.translation.z = i as f32 * 0.01;
            let bucket = (p.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
            let material = self
                .mats
                .entry(bucket)
                .or_insert_with(|| {
                    materials.add(ColorMaterial {
                        color: Color::srgba(1.0, 1.0, 1.0, bucket as f32 / 255.0),
                        alpha_mode: if bucket == 255 { AlphaMode2d::Opaque } else { AlphaMode2d::Blend },
                        ..default()
                    })
                })
                .clone();
            out.push(PartDraw { mesh, material, transform });
        }
        out
    }
}
