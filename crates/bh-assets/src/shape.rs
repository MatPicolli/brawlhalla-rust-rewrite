//! Formas vetoriais SWF (DefineShape 1-4): parse, costura de arestas por estilo e tesselação.
//! Coordenadas de saída em pixels (twips / 20), y para baixo, como no Flash.

use std::collections::HashMap;

use anyhow::{bail, ensure, Result};
use lyon_tessellation::{
    math::{point, Point},
    path::Path,
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator, StrokeVertex,
    VertexBuffers,
};

pub struct BitReader<'a> {
    b: &'a [u8],
    pub pos: usize, // em bits
}

impl<'a> BitReader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, pos: 0 }
    }
    pub fn at(b: &'a [u8], byte: usize) -> Self {
        Self { b, pos: byte * 8 }
    }
    pub fn byte_pos(&self) -> usize {
        self.pos.div_ceil(8)
    }
    pub fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }
    pub fn ub(&mut self, n: u32) -> Result<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self.b.get(self.pos / 8).ok_or_else(|| anyhow::anyhow!("bits: fim dos dados"))?;
            v = (v << 1) | ((byte >> (7 - self.pos % 8)) & 1) as u32;
            self.pos += 1;
        }
        Ok(v)
    }
    pub fn sb(&mut self, n: u32) -> Result<i32> {
        if n == 0 {
            return Ok(0);
        }
        let v = self.ub(n)? as i32;
        Ok((v << (32 - n)) >> (32 - n))
    }
    pub fn fb(&mut self, n: u32) -> Result<f32> {
        Ok(self.sb(n)? as f32 / 65536.0)
    }
    pub fn u8(&mut self) -> Result<u8> {
        self.align();
        Ok(self.ub(8)? as u8)
    }
    pub fn u16(&mut self) -> Result<u16> {
        let lo = self.u8()? as u16;
        Ok(lo | (self.u8()? as u16) << 8)
    }
    pub fn rect(&mut self) -> Result<[i32; 4]> {
        let n = self.ub(5)?;
        let r = [self.sb(n)?, self.sb(n)?, self.sb(n)?, self.sb(n)?];
        self.align();
        Ok(r)
    }
    /// Matriz do Flash: [a, b, c, d, tx, ty] com tx/ty em pixels.
    pub fn matrix(&mut self) -> Result<[f32; 6]> {
        let (mut a, mut d) = (1.0, 1.0);
        let (mut b, mut c) = (0.0, 0.0);
        if self.ub(1)? == 1 {
            let n = self.ub(5)?;
            a = self.fb(n)?;
            d = self.fb(n)?;
        }
        if self.ub(1)? == 1 {
            let n = self.ub(5)?;
            b = self.fb(n)?;
            c = self.fb(n)?;
        }
        let n = self.ub(5)?;
        let tx = self.sb(n)? as f32 / 20.0;
        let ty = self.sb(n)? as f32 / 20.0;
        self.align();
        Ok([a, b, c, d, tx, ty])
    }
}

/// Composição de matrizes Flash: aplica `child` primeiro e depois `parent`.
pub fn mat_mul(p: &[f32; 6], c: &[f32; 6]) -> [f32; 6] {
    [
        p[0] * c[0] + p[2] * c[1],
        p[1] * c[0] + p[3] * c[1],
        p[0] * c[2] + p[2] * c[3],
        p[1] * c[2] + p[3] * c[3],
        p[0] * c[4] + p[2] * c[5] + p[4],
        p[1] * c[4] + p[3] * c[5] + p[5],
    ]
}

/// Transformação de cor: canal' = canal * mult + add (valores em 0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorXf {
    pub mult: [f32; 4],
    pub add: [f32; 4],
}

impl ColorXf {
    pub const ID: ColorXf = ColorXf { mult: [1.0; 4], add: [0.0; 4] };

    pub fn read(r: &mut BitReader) -> Result<ColorXf> {
        let has_add = r.ub(1)? == 1;
        let has_mult = r.ub(1)? == 1;
        let n = r.ub(4)?;
        let mut xf = ColorXf::ID;
        if has_mult {
            for i in 0..4 {
                xf.mult[i] = r.sb(n)? as f32 / 256.0;
            }
        }
        if has_add {
            for i in 0..4 {
                xf.add[i] = r.sb(n)? as f32 / 255.0;
            }
        }
        r.align();
        Ok(xf)
    }

    /// `self` aplicada depois de `inner`.
    pub fn after(self, inner: ColorXf) -> ColorXf {
        let mut o = ColorXf::ID;
        for i in 0..4 {
            o.mult[i] = inner.mult[i] * self.mult[i];
            o.add[i] = inner.add[i] * self.mult[i] + self.add[i];
        }
        o
    }

    pub fn apply(&self, c: [f32; 4]) -> [f32; 4] {
        let mut o = c;
        for i in 0..4 {
            o[i] = (c[i] * self.mult[i] + self.add[i]).clamp(0.0, 1.0);
        }
        o
    }
}

#[derive(Debug, Clone)]
pub enum Fill {
    Solid([f32; 4]),
    Linear { m: [f32; 6], stops: Vec<(f32, [f32; 4])> },
    Radial { m: [f32; 6], stops: Vec<(f32, [f32; 4])> },
    /// Bitmaps não são usados pela arte do jogo (não há tags de bitmap nos SWF); cinza neutro.
    Bitmap,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub width: f32,
    pub color: [f32; 4],
}

/// Geometria 2D com cor por vértice, já tesselada.
#[derive(Debug, Clone, Default)]
pub struct TriMesh {
    pub pos: Vec<[f32; 2]>,
    pub color: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
}

impl TriMesh {
    pub fn is_empty(&self) -> bool {
        self.idx.is_empty()
    }

    /// Anexa `other` transformada por `m` (matriz Flash) e `xf`.
    pub fn append(&mut self, other: &TriMesh, m: &[f32; 6], xf: &ColorXf) {
        let base = self.pos.len() as u32;
        let [a, b, c, d, tx, ty] = *m;
        for (p, col) in other.pos.iter().zip(&other.color) {
            self.pos.push([a * p[0] + c * p[1] + tx, b * p[0] + d * p[1] + ty]);
            self.color.push(xf.apply(*col));
        }
        self.idx.extend(other.idx.iter().map(|i| i + base));
    }
}

#[derive(Debug, Clone)]
pub struct Shape {
    pub id: u16,
    pub bounds: [i32; 4],
    pub mesh: TriMesh,
}

#[derive(Clone, Copy, Debug)]
struct Seg {
    a: (i32, i32),
    c: Option<(i32, i32)>,
    b: (i32, i32),
}

impl Seg {
    fn rev(self) -> Seg {
        Seg { a: self.b, c: self.c, b: self.a }
    }
}

/// Liga segmentos pelos extremos (coordenadas inteiras em twips: a igualdade é exata).
fn stitch(segs: &[Seg]) -> Vec<Vec<Seg>> {
    let mut starts: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        starts.entry(s.a).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut chains = Vec::new();
    for i in 0..segs.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut chain = vec![segs[i]];
        let origin = segs[i].a;
        loop {
            let end = chain.last().unwrap().b;
            if end == origin {
                break;
            }
            let next = starts.get(&end).and_then(|v| v.iter().copied().find(|&j| !used[j]));
            match next {
                Some(j) => {
                    used[j] = true;
                    chain.push(segs[j]);
                }
                None => break,
            }
        }
        chains.push(chain);
    }
    chains
}

fn px(p: (i32, i32)) -> Point {
    point(p.0 as f32 / 20.0, p.1 as f32 / 20.0)
}

fn chain_path(chains: &[Vec<Seg>], close_open: bool) -> Path {
    let mut b = Path::builder();
    for ch in chains {
        b.begin(px(ch[0].a));
        for s in ch {
            match s.c {
                Some(c) => b.quadratic_bezier_to(px(c), px(s.b)),
                None => b.line_to(px(s.b)),
            };
        }
        let closed = ch.last().unwrap().b == ch[0].a;
        b.end(closed || close_open);
    }
    b.build()
}

fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

fn gradient_color(stops: &[(f32, [f32; 4])], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    if stops.is_empty() {
        return [0.5, 0.5, 0.5, 1.0];
    }
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if t <= w[1].0 {
            let span = (w[1].0 - w[0].0).max(1e-6);
            return lerp4(w[0].1, w[1].1, (t - w[0].0) / span);
        }
    }
    stops.last().unwrap().1
}

fn invert(m: &[f32; 6]) -> [f32; 6] {
    let [a, b, c, d, tx, ty] = *m;
    let det = a * d - b * c;
    let det = if det.abs() < 1e-9 { 1e-9 } else { det };
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    [ia, ib, ic, id, -(ia * tx + ic * ty), -(ib * tx + id * ty)]
}

fn fill_color_at(f: &Fill, p: [f32; 2]) -> [f32; 4] {
    match f {
        Fill::Solid(c) => *c,
        Fill::Bitmap => [0.6, 0.6, 0.6, 1.0],
        Fill::Linear { m, stops } | Fill::Radial { m, stops } => {
            // Matriz de gradiente em pixels; espaço do gradiente = ±16384 twips = ±819.2 px.
            let inv = invert(m);
            let gx = inv[0] * p[0] + inv[2] * p[1] + inv[4];
            let gy = inv[1] * p[0] + inv[3] * p[1] + inv[5];
            let t = if matches!(f, Fill::Linear { .. }) {
                (gx + 819.2) / 1638.4
            } else {
                (gx * gx + gy * gy).sqrt() / 819.2
            };
            gradient_color(stops, t)
        }
    }
}

fn read_rgb(r: &mut BitReader, alpha: bool) -> Result<[f32; 4]> {
    let (rr, g, b) = (r.u8()?, r.u8()?, r.u8()?);
    let a = if alpha { r.u8()? } else { 255 };
    Ok([rr as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0])
}

fn read_gradient(r: &mut BitReader, alpha: bool, focal: bool) -> Result<Vec<(f32, [f32; 4])>> {
    r.align();
    let _spread_interp = r.ub(4)?;
    let n = r.ub(4)?;
    let mut stops = Vec::new();
    for _ in 0..n {
        let ratio = r.u8()? as f32 / 255.0;
        stops.push((ratio, read_rgb(r, alpha)?));
    }
    if focal {
        r.u16()?; // ponto focal 8.8
    }
    Ok(stops)
}

fn read_fill_styles(r: &mut BitReader, ver: u8, out: &mut Vec<Fill>) -> Result<()> {
    let alpha = ver >= 3;
    let mut n = r.u8()? as usize;
    if n == 0xFF && ver >= 2 {
        n = r.u16()? as usize;
    }
    for _ in 0..n {
        out.push(read_fill_style(r, alpha)?);
    }
    Ok(())
}

fn read_fill_style(r: &mut BitReader, alpha: bool) -> Result<Fill> {
    let kind = r.u8()?;
    Ok(match kind {
        0x00 => Fill::Solid(read_rgb(r, alpha)?),
        0x10 | 0x12 | 0x13 => {
            r.align();
            let m = r.matrix()?;
            let stops = read_gradient(r, alpha, kind == 0x13)?;
            if kind == 0x10 { Fill::Linear { m, stops } } else { Fill::Radial { m, stops } }
        }
        0x40..=0x43 => {
            r.u16()?;
            r.align();
            r.matrix()?;
            Fill::Bitmap
        }
        other => bail!("tipo de preenchimento desconhecido {other:#x}"),
    })
}

fn read_line_styles(r: &mut BitReader, ver: u8, out: &mut Vec<Line>) -> Result<()> {
    let mut n = r.u8()? as usize;
    if n == 0xFF {
        n = r.u16()? as usize;
    }
    for _ in 0..n {
        if ver >= 4 {
            let width = r.u16()? as f32 / 20.0;
            r.align();
            r.ub(2)?; // start cap
            let join = r.ub(2)?;
            let has_fill = r.ub(1)? == 1;
            r.ub(3)?; // noHScale, noVScale, pixelHinting
            r.ub(5)?;
            r.ub(1)?; // noClose
            r.ub(2)?; // end cap
            if join == 2 {
                r.u16()?;
            }
            let color = if has_fill {
                match read_fill_style(r, true)? {
                    Fill::Solid(c) => c,
                    _ => [0.0, 0.0, 0.0, 1.0],
                }
            } else {
                read_rgb(r, true)?
            };
            out.push(Line { width, color });
        } else {
            let width = r.u16()? as f32 / 20.0;
            out.push(Line { width, color: read_rgb(r, ver >= 3)? });
        }
    }
    Ok(())
}

/// `data` é o corpo da tag; `ver` é 1..=4 (DefineShape, 2, 3, 4).
pub fn parse(data: &[u8], ver: u8) -> Result<Shape> {
    ensure!(data.len() > 2, "shape vazia");
    let mut r = BitReader::at(data, 0);
    let id = r.u16()?;
    let bounds = r.rect()?;
    if ver >= 4 {
        r.rect()?; // edge bounds
        r.u8()?; // flags
    }
    let mut fills: Vec<Fill> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    read_fill_styles(&mut r, ver, &mut fills)?;
    read_line_styles(&mut r, ver, &mut lines)?;
    let mut nfill = r.ub(4)?;
    let mut nline = r.ub(4)?;
    let (mut fill_base, mut line_base) = (0usize, 0usize);

    let mut fill_segs: Vec<Vec<Seg>> = vec![Vec::new(); fills.len()];
    let mut line_segs: Vec<Vec<Seg>> = vec![Vec::new(); lines.len()];
    let (mut f0, mut f1, mut ls) = (0usize, 0usize, 0usize);
    let (mut x, mut y) = (0i32, 0i32);

    loop {
        if r.ub(1)? == 0 {
            let new_styles = r.ub(1)? == 1;
            let st_line = r.ub(1)? == 1;
            let st_f1 = r.ub(1)? == 1;
            let st_f0 = r.ub(1)? == 1;
            let st_move = r.ub(1)? == 1;
            if !(new_styles || st_line || st_f1 || st_f0 || st_move) {
                break;
            }
            if st_move {
                let n = r.ub(5)?;
                x = r.sb(n)?;
                y = r.sb(n)?;
            }
            if st_f0 {
                f0 = r.ub(nfill)? as usize;
            }
            if st_f1 {
                f1 = r.ub(nfill)? as usize;
            }
            if st_line {
                ls = r.ub(nline)? as usize;
            }
            if new_styles {
                fill_base = fills.len();
                line_base = lines.len();
                read_fill_styles(&mut r, ver, &mut fills)?;
                read_line_styles(&mut r, ver, &mut lines)?;
                fill_segs.resize(fills.len(), Vec::new());
                line_segs.resize(lines.len(), Vec::new());
                nfill = r.ub(4)?;
                nline = r.ub(4)?;
                // índices dos registros seguintes passam a ser relativos à nova tabela
                if !st_f0 {
                    f0 = 0;
                }
                if !st_f1 {
                    f1 = 0;
                }
                if !st_line {
                    ls = 0;
                }
            }
        } else {
            let from = (x, y);
            let seg;
            if r.ub(1)? == 1 {
                let n = r.ub(4)? + 2;
                let (dx, dy) = if r.ub(1)? == 1 {
                    (r.sb(n)?, r.sb(n)?)
                } else if r.ub(1)? == 0 {
                    (r.sb(n)?, 0)
                } else {
                    (0, r.sb(n)?)
                };
                x += dx;
                y += dy;
                seg = Seg { a: from, c: None, b: (x, y) };
            } else {
                let n = r.ub(4)? + 2;
                let (cdx, cdy, adx, ady) = (r.sb(n)?, r.sb(n)?, r.sb(n)?, r.sb(n)?);
                let c = (x + cdx, y + cdy);
                x = c.0 + adx;
                y = c.1 + ady;
                seg = Seg { a: from, c: Some(c), b: (x, y) };
            }
            let gf = |i: usize| if i == 0 { None } else { Some(fill_base + i - 1) };
            if let Some(i) = gf(f1).filter(|&i| i < fill_segs.len()) {
                fill_segs[i].push(seg);
            }
            if let Some(i) = gf(f0).filter(|&i| i < fill_segs.len()) {
                fill_segs[i].push(seg.rev());
            }
            if ls != 0 {
                let i = line_base + ls - 1;
                if i < line_segs.len() {
                    line_segs[i].push(seg);
                }
            }
        }
    }

    let mut mesh = TriMesh::default();
    let mut fill_tess = FillTessellator::new();
    for (i, segs) in fill_segs.iter().enumerate() {
        if segs.is_empty() {
            continue;
        }
        let path = chain_path(&stitch(segs), true);
        let mut buf: VertexBuffers<Point, u32> = VertexBuffers::new();
        let opts = FillOptions::default().with_fill_rule(FillRule::NonZero).with_tolerance(0.05);
        if fill_tess.tessellate_path(&path, &opts, &mut BuffersBuilder::new(&mut buf, |v: FillVertex| v.position())).is_err() {
            continue;
        }
        let base = mesh.pos.len() as u32;
        for p in &buf.vertices {
            let pos = [p.x, p.y];
            mesh.pos.push(pos);
            mesh.color.push(fill_color_at(&fills[i], pos));
        }
        mesh.idx.extend(buf.indices.iter().map(|k| k + base));
    }
    let mut stroke_tess = StrokeTessellator::new();
    for (i, segs) in line_segs.iter().enumerate() {
        if segs.is_empty() {
            continue;
        }
        let path = chain_path(&stitch(segs), false);
        let mut buf: VertexBuffers<Point, u32> = VertexBuffers::new();
        let opts = StrokeOptions::default().with_line_width(lines[i].width.max(0.5)).with_tolerance(0.05);
        if stroke_tess.tessellate_path(&path, &opts, &mut BuffersBuilder::new(&mut buf, |v: StrokeVertex| v.position())).is_err() {
            continue;
        }
        let base = mesh.pos.len() as u32;
        for p in &buf.vertices {
            mesh.pos.push([p.x, p.y]);
            mesh.color.push(lines[i].color);
        }
        mesh.idx.extend(buf.indices.iter().map(|k| k + base));
    }
    Ok(Shape { id, bounds, mesh })
}
