//! Montagem de personagens: trajes e skins de arma (camadas `arquivo.swf/sufixo`) e a resolução
//! de um osso do `.anm` para o símbolo de arte (`osso_sufixo`) dentro dos SWF do jogo.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::movie::Movie;
use crate::shape::TriMesh;

/// Camada de arte: o símbolo de um osso `B` é `B_<suffix>` dentro de `file`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub file: String,
    pub suffix: String,
}

#[derive(Debug, Clone)]
pub struct Costume {
    pub name: String,
    pub hero: String,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone)]
pub struct WeaponSkin {
    pub name: String,
    pub weapon: String,
    pub hero: String,
    pub layers: Vec<Layer>,
}

fn parse_layer(s: &str) -> Option<Layer> {
    let (file, suffix) = s.trim().split_once('/')?;
    (!file.is_empty() && !suffix.is_empty()).then(|| Layer { file: file.to_string(), suffix: suffix.to_string() })
}

/// Seção CSV do Game.swz: a primeira linha é o nome da tabela, a segunda o cabeçalho.
pub(crate) fn csv_section<'a>(sections: &'a [Vec<u8>], table: &str) -> Option<&'a [u8]> {
    sections.iter().find_map(|s| {
        let nl = s.iter().position(|&b| b == b'\n')?;
        (String::from_utf8_lossy(&s[..nl]).trim() == table).then(|| &s[nl + 1..])
    })
}

pub(crate) fn read_rows(data: &[u8]) -> Vec<HashMap<String, String>> {
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(data);
    let headers: Vec<String> = match rdr.headers() {
        Ok(h) => h.iter().map(str::to_string).collect(),
        Err(_) => return Vec::new(),
    };
    rdr.records()
        .flatten()
        .map(|r| headers.iter().cloned().zip(r.iter().map(str::to_string)).collect())
        .collect()
}

fn layers_of(row: &HashMap<String, String>, prefix: &str) -> Vec<Layer> {
    // GfxType.CustomArt, GfxType.CustomArt2 ... (a ordem importa: camadas posteriores sobrescrevem)
    let mut keyed: Vec<(u32, &String)> = row
        .iter()
        .filter_map(|(k, v)| {
            let rest = k.strip_prefix(prefix)?;
            let n = if rest.is_empty() { 1 } else { rest.parse().ok()? };
            (!v.trim().is_empty()).then_some((n, v))
        })
        .collect();
    keyed.sort_by_key(|(n, _)| *n);
    keyed.into_iter().filter_map(|(_, v)| parse_layer(v)).collect()
}

pub fn costumes(game_sections: &[Vec<u8>]) -> Vec<Costume> {
    let Some(data) = csv_section(game_sections, "costumeTypes") else { return Vec::new() };
    read_rows(data)
        .into_iter()
        .map(|r| Costume {
            name: r.get("CostumeName").cloned().unwrap_or_default(),
            hero: r.get("OwnerHero").cloned().unwrap_or_default(),
            layers: layers_of(&r, "GfxType.CustomArt"),
        })
        .collect()
}

pub fn weapon_skins(game_sections: &[Vec<u8>]) -> Vec<WeaponSkin> {
    let Some(data) = csv_section(game_sections, "weaponSkinTypes") else { return Vec::new() };
    read_rows(data)
        .into_iter()
        .map(|r| WeaponSkin {
            name: r.get("WeaponSkinName").cloned().unwrap_or_default(),
            weapon: r.get("BaseWeapon").cloned().unwrap_or_default(),
            hero: r.get("OwnerHero").cloned().unwrap_or_default(),
            layers: layers_of(&r, "CustomArt"),
        })
        .collect()
}

/// SWF do jogo carregados sob demanda (formas tesseladas em cache).
pub struct Library {
    dir: PathBuf,
    movies: HashMap<String, Option<Movie>>,
}

impl Library {
    pub fn new(game_dir: &Path) -> Self {
        Self { dir: game_dir.to_path_buf(), movies: HashMap::new() }
    }

    pub fn movie(&mut self, file: &str) -> Option<&mut Movie> {
        if !self.movies.contains_key(file) {
            let m = Movie::load(&self.dir.join(file)).ok();
            self.movies.insert(file.to_string(), m);
        }
        self.movies.get_mut(file).and_then(|m| m.as_mut())
    }

    /// Procura o símbolo de arte do `bone` nas camadas (a última que tiver o símbolo vence).
    /// Ossos `...Right` sem arte própria reaproveitam a arte do osso sem o sufixo.
    pub fn resolve(&mut self, bone: &str, layers: &[Layer]) -> Option<(String, String)> {
        let mut candidates = vec![bone.to_string()];
        if let Some(base) = bone.strip_suffix("Right") {
            candidates.push(base.to_string());
        }
        for name in candidates {
            let mut found = None;
            for l in layers {
                let sym = format!("{name}_{}", l.suffix);
                if self.movie(&l.file).is_some_and(|m| m.symbols.contains_key(&sym)) {
                    found = Some((l.file.clone(), sym));
                }
            }
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// Malha de um símbolo (quadro 0-based) já resolvido por `resolve`.
    pub fn mesh(&mut self, file: &str, symbol: &str, frame: usize) -> Option<TriMesh> {
        self.movie(file)?.symbol_mesh(symbol, frame).cloned()
    }
}
