//! Um SWF carregado: formas, sprites com display list por quadro e símbolos exportados.
//! `symbol_mesh` achata um símbolo num quadro em uma única malha colorida (sem animar sub-sprites).

use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, Result};

use crate::{
    shape::{self, mat_mul, BitReader, ColorXf, Shape, TriMesh},
    swf,
};

const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

#[derive(Debug, Clone)]
pub struct Placed {
    pub id: u16,
    pub m: [f32; 6],
    pub xf: ColorXf,
    /// Camada de máscara: não é desenhada (as máscaras não são aplicadas).
    pub clip_depth: Option<u16>,
}

#[derive(Debug, Default)]
pub struct Sprite {
    /// Lista de exibição (ordenada por profundidade) a cada ShowFrame.
    pub frames: Vec<Vec<Placed>>,
}

pub struct Movie {
    body: Vec<u8>,
    shape_tags: HashMap<u16, (usize, usize, u8)>,
    pub sprites: HashMap<u16, Sprite>,
    pub symbols: HashMap<String, u16>,
    shapes: HashMap<u16, Option<Shape>>,
    flat: HashMap<(u16, usize), TriMesh>,
}

fn run_timeline(tags: &[swf::Tag]) -> Result<Vec<Vec<Placed>>> {
    let mut dl: BTreeMap<u16, Placed> = BTreeMap::new();
    let mut frames = Vec::new();
    for t in tags {
        match t.code {
            1 => frames.push(dl.values().cloned().collect()),
            26 | 70 => {
                let d = t.data;
                let mut r = BitReader::at(d, 0);
                let f1 = r.u8()?;
                let f2 = if t.code == 70 { r.u8()? } else { 0 };
                let depth = r.u16()?;
                let has_move = f1 & 0x01 != 0;
                let has_char = f1 & 0x02 != 0;
                if t.code == 70 && (f2 & 0x08 != 0 || (f2 & 0x10 != 0 && has_char)) {
                    while r.u8()? != 0 {}
                }
                let id = if has_char { Some(r.u16()?) } else { None };
                let m = if f1 & 0x04 != 0 { Some(r.matrix()?) } else { None };
                let xf = if f1 & 0x08 != 0 { Some(ColorXf::read(&mut r)?) } else { None };
                if f1 & 0x10 != 0 {
                    r.u16()?; // ratio (morph) ignorado
                }
                if f1 & 0x20 != 0 {
                    while r.u8()? != 0 {}
                }
                let clip = if f1 & 0x40 != 0 { Some(r.u16()?) } else { None };

                let entry = if has_move && dl.contains_key(&depth) {
                    dl.get_mut(&depth).unwrap()
                } else {
                    dl.entry(depth).or_insert(Placed { id: 0, m: IDENTITY, xf: ColorXf::ID, clip_depth: None })
                };
                if let Some(id) = id {
                    entry.id = id;
                }
                if let Some(m) = m {
                    entry.m = m;
                }
                if let Some(xf) = xf {
                    entry.xf = xf;
                }
                if clip.is_some() {
                    entry.clip_depth = clip;
                }
            }
            4 => {
                // PlaceObject antigo: id, depth, matrix, [cxform]
                let mut r = BitReader::at(t.data, 0);
                let id = r.u16()?;
                let depth = r.u16()?;
                let m = r.matrix()?;
                dl.insert(depth, Placed { id, m, xf: ColorXf::ID, clip_depth: None });
            }
            28 => {
                let depth = u16::from_le_bytes(t.data[..2].try_into()?);
                dl.remove(&depth);
            }
            5 => {
                let depth = u16::from_le_bytes(t.data[2..4].try_into()?);
                dl.remove(&depth);
            }
            _ => {}
        }
    }
    Ok(frames)
}

impl Movie {
    pub fn from_bytes(file: &[u8]) -> Result<Movie> {
        let body = swf::body(file)?;
        let tags = swf::tags(&body)?;
        let base = body.as_ptr() as usize;
        let mut shape_tags = HashMap::new();
        let mut sprites = HashMap::new();
        let mut symbols = HashMap::new();
        for t in &tags {
            let off = t.data.as_ptr() as usize - base;
            match t.code {
                2 | 22 | 32 | 83 => {
                    let ver = match t.code {
                        2 => 1,
                        22 => 2,
                        32 => 3,
                        _ => 4,
                    };
                    let id = u16::from_le_bytes(t.data[..2].try_into()?);
                    shape_tags.insert(id, (off, t.data.len(), ver));
                }
                39 => {
                    let id = u16::from_le_bytes(t.data[..2].try_into()?);
                    let inner = swf::tags_at(&body[..off + t.data.len()], off + 4)?;
                    sprites.insert(id, Sprite { frames: run_timeline(&inner)? });
                }
                76 | 56 => {
                    let n = u16::from_le_bytes(t.data[..2].try_into()?) as usize;
                    let mut p = 2;
                    for _ in 0..n {
                        let id = u16::from_le_bytes(t.data[p..p + 2].try_into()?);
                        let end = t.data[p + 2..].iter().position(|&b| b == 0).ok_or_else(|| anyhow!("símbolo sem fim"))?;
                        symbols.insert(String::from_utf8_lossy(&t.data[p + 2..p + 2 + end]).into_owned(), id);
                        p += 2 + end + 1;
                    }
                }
                _ => {}
            }
        }
        Ok(Movie { body, shape_tags, sprites, symbols, shapes: HashMap::new(), flat: HashMap::new() })
    }

    pub fn load(path: &std::path::Path) -> Result<Movie> {
        Movie::from_bytes(&std::fs::read(path)?)
    }

    pub fn sprite_frames(&self, symbol: &str) -> Option<usize> {
        self.sprites.get(self.symbols.get(symbol)?).map(|s| s.frames.len())
    }

    pub fn shape_ids(&self) -> Vec<u16> {
        let mut v: Vec<u16> = self.shape_tags.keys().copied().collect();
        v.sort_unstable();
        v
    }

    /// Forma tesselada (None se o parse falhar).
    pub fn shape(&mut self, id: u16) -> Option<&Shape> {
        if !self.shapes.contains_key(&id) {
            let parsed = self.shape_tags.get(&id).and_then(|&(off, len, ver)| shape::parse(&self.body[off..off + len], ver).ok());
            self.shapes.insert(id, parsed);
        }
        self.shapes.get(&id).and_then(|s| s.as_ref())
    }

    fn flatten(&mut self, id: u16, frame: usize, m: &[f32; 6], xf: &ColorXf, out: &mut TriMesh, depth: u32) {
        if depth > 12 {
            return;
        }
        if let Some(sh) = self.shape(id) {
            out.append(&sh.mesh, m, xf);
            return;
        }
        let Some(sp) = self.sprites.get(&id) else { return };
        if sp.frames.is_empty() {
            return;
        }
        let items = sp.frames[frame.min(sp.frames.len() - 1)].clone();
        for p in items {
            if p.clip_depth.is_some() || p.id == 0 {
                continue;
            }
            self.flatten(p.id, 0, &mat_mul(m, &p.m), &xf.after(p.xf), out, depth + 1);
        }
    }

    /// Malha de um símbolo exportado no quadro `frame` (0-based). Sub-sprites usam o primeiro quadro.
    pub fn symbol_mesh(&mut self, symbol: &str, frame: usize) -> Option<&TriMesh> {
        let id = *self.symbols.get(symbol)?;
        if !self.flat.contains_key(&(id, frame)) {
            let mut mesh = TriMesh::default();
            self.flatten(id, frame, &IDENTITY, &ColorXf::ID, &mut mesh, 0);
            self.flat.insert((id, frame), mesh);
        }
        self.flat.get(&(id, frame))
    }
}
