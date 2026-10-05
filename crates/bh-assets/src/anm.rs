//! Formato `anims/*.anm` (já descomprimido por `zblob::decode`), engenharia reversa a partir
//! do bytecode AS3 do `BrawlhallaAir.swf` (classes de animação ofuscadas `_-p3N/_-T4f/_-S2a/_-N3T/_-X5F`).
//!
//! ```text
//! arquivo   := ( bool continua, string chave, entrada )*  até continua == false
//! entrada   := string classe, string arquivo, u32 n, animação × n
//! animação  := string nome, u32 × 6 (cabeçalho), u32 k, u32 × k, u32 bytes, quadro × frame_count
//! quadro    := u16 id, ponto?, ponto?, u16 n, parte × n
//! parte     := bool igual_ao_anterior  -> (clona a parte homônima do quadro anterior; bool, se falso: i8 frame_no)
//!            | transformação nova
//! ponto     := bool presente, [f64, f64]
//! ```
//! Tudo little-endian; `string` é `u16` de tamanho + UTF-8.

use anyhow::{anyhow, bail, ensure, Result};

#[derive(Debug, Clone)]
pub struct AnmFile {
    pub entries: Vec<AnmEntry>,
}

#[derive(Debug, Clone)]
pub struct AnmEntry {
    /// Chave "Arquivo.swf/a_Classe".
    pub key: String,
    pub class: String,
    pub file: String,
    pub anims: Vec<Anim>,
}

#[derive(Debug, Clone)]
pub struct Anim {
    pub name: String,
    /// Seis `u32` do cabeçalho; `header[0]` é o número de quadros. Significado dos demais: pendente.
    pub header: [u32; 6],
    pub extra: Vec<u32>,
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub id: u16,
    pub p1: Option<(f64, f64)>,
    pub p2: Option<(f64, f64)>,
    pub parts: Vec<Part>,
}

/// Uma parte do esqueleto/sprite posicionada num quadro.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// Índice na lista `BoneTypes` (Init.swz).
    pub bone: u16,
    /// Matriz 2D no estilo Flash: a, b, c, d, tx, ty.
    pub m: [f32; 6],
    pub opacity: f32,
    /// Quadro do clipe de sprite da parte (1-based).
    pub frame_no: i32,
}

struct Cur<'a> {
    b: &'a [u8],
    o: usize,
}

impl<'a> Cur<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = self.b.get(self.o..self.o + n).ok_or_else(|| anyhow!("fim inesperado em {}", self.o))?;
        self.o += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into()?))
    }
    fn string(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn point(&mut self) -> Result<Option<(f64, f64)>> {
        if !self.bool()? {
            return Ok(None);
        }
        Ok(Some((self.f64()?, self.f64()?)))
    }
}

fn read_part(c: &mut Cur, prev: Option<&Part>) -> Result<Part> {
    let bone = c.u16()?;
    let opaque = c.bool()?;
    let mut m = [0f32; 6];
    if c.bool()? {
        let p = prev.ok_or_else(|| anyhow!("parte copia matriz sem parte anterior"))?;
        m[..4].copy_from_slice(&p.m[..4]);
    } else {
        let (mut identity, mut rot) = (false, false);
        if c.bool()? {
            if c.bool()? {
                identity = true;
            } else {
                rot = true;
            }
        }
        if identity {
            m[..4].copy_from_slice(&[1.0, 0.0, 0.0, 1.0]);
        } else {
            m[0] = c.f32()?;
            m[1] = c.f32()?;
            if rot {
                m[2] = m[1];
                m[3] = -m[0];
            } else {
                m[2] = c.f32()?;
                m[3] = c.f32()?;
            }
        }
    }
    if c.bool()? {
        let p = prev.ok_or_else(|| anyhow!("parte copia translação sem parte anterior"))?;
        m[4] = p.m[4];
        m[5] = p.m[5];
    } else {
        m[4] = c.f32()?;
        m[5] = c.f32()?;
    }
    let mut frame_no = 1;
    if c.bool()? {
        frame_no = c.u8()? as i8 as i32;
    }
    let opacity = if opaque { 1.0 } else { c.u8()? as f32 / 255.0 };
    Ok(Part { bone, m, opacity, frame_no })
}

fn read_frame(c: &mut Cur, prev: Option<&Frame>) -> Result<Frame> {
    let id = c.u16()?;
    let p1 = c.point()?;
    let p2 = c.point()?;
    let n = c.u16()? as usize;
    let mut parts: Vec<Part> = Vec::with_capacity(n);
    for i in 0..n {
        let part = if c.bool()? {
            let mut p = prev
                .and_then(|f| f.parts.get(i))
                .ok_or_else(|| anyhow!("parte {i} copia quadro anterior inexistente"))?
                .clone();
            if !c.bool()? {
                p.frame_no = c.u8()? as i8 as i32;
            }
            p
        } else {
            read_part(c, if i > 0 { parts.get(i - 1) } else { None })?
        };
        parts.push(part);
    }
    Ok(Frame { id, p1, p2, parts })
}

fn read_anim(c: &mut Cur) -> Result<Anim> {
    let name = c.string()?;
    let mut header = [0u32; 6];
    for h in &mut header {
        *h = c.u32()?;
    }
    let k = c.u32()? as usize;
    let extra = (0..k).map(|_| c.u32()).collect::<Result<Vec<_>>>()?;
    let bytes = c.u32()? as usize;
    let start = c.o;
    let data = c.take(bytes)?;
    let mut fc = Cur { b: data, o: 0 };
    let mut frames: Vec<Frame> = Vec::with_capacity(header[0] as usize);
    for _ in 0..header[0] {
        let f = read_frame(&mut fc, frames.last())?;
        frames.push(f);
    }
    ensure!(fc.o == bytes, "animação '{name}' em {start}: consumiu {} de {bytes} bytes", fc.o);
    Ok(Anim { name, header, extra, frames })
}

pub fn parse(data: &[u8]) -> Result<AnmFile> {
    let mut c = Cur { b: data, o: 0 };
    let mut entries = Vec::new();
    while c.bool()? {
        let key = c.string()?;
        let class = c.string()?;
        let file = c.string()?;
        let n = c.u32()?;
        let anims = (0..n).map(|_| read_anim(&mut c)).collect::<Result<Vec<_>>>()?;
        entries.push(AnmEntry { key, class, file, anims });
    }
    if c.o != data.len() {
        bail!("sobraram {} bytes após o último registro", data.len() - c.o);
    }
    Ok(AnmFile { entries })
}
