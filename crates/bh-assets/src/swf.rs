//! Leitor mínimo de SWF: só o necessário para achar blocos ABC e suas constantes.

use std::io::Read;

use anyhow::{bail, Result};
use flate2::read::ZlibDecoder;

/// Devolve o corpo descomprimido do SWF (sem o cabeçalho de 8 bytes).
pub fn body(file: &[u8]) -> Result<Vec<u8>> {
    if file.len() < 8 {
        bail!("SWF curto demais");
    }
    match &file[..3] {
        b"FWS" => Ok(file[8..].to_vec()),
        b"CWS" => {
            let mut out = Vec::new();
            ZlibDecoder::new(&file[8..]).read_to_end(&mut out)?;
            Ok(out)
        }
        other => bail!("assinatura SWF não suportada: {:?}", other),
    }
}

pub struct Tag<'a> {
    pub code: u16,
    pub data: &'a [u8],
}

/// Percorre as tags do corpo (pula RECT do frame, frame rate e frame count).
pub fn tags(body: &[u8]) -> Result<Vec<Tag<'_>>> {
    let nbits = (body[0] >> 3) as usize;
    tags_at(body, (5 + nbits * 4 + 7) / 8 + 4)
}

/// Tags a partir de `off` (também serve para o corpo de um DefineSprite).
pub fn tags_at(body: &[u8], mut off: usize) -> Result<Vec<Tag<'_>>> {
    let mut out = Vec::new();
    while off + 2 <= body.len() {
        let h = u16::from_le_bytes([body[off], body[off + 1]]);
        off += 2;
        let code = h >> 6;
        let mut len = (h & 63) as usize;
        if len == 63 {
            len = u32::from_le_bytes(body[off..off + 4].try_into()?) as usize;
            off += 4;
        }
        if off + len > body.len() {
            bail!("tag {code} truncada");
        }
        out.push(Tag { code, data: &body[off..off + len] });
        off += len;
        if code == 0 {
            break;
        }
    }
    Ok(out)
}

/// Blocos ABC (DoABC = 72, DoABC2 = 82) de um SWF.
pub fn abc_blocks<'a>(tags: &[Tag<'a>]) -> Vec<&'a [u8]> {
    let mut out = Vec::new();
    for t in tags {
        match t.code {
            72 => out.push(t.data),
            82 if t.data.len() > 4 => {
                // flags u32 + nome terminado em NUL, depois o ABC
                if let Some(z) = t.data[4..].iter().position(|&b| b == 0) {
                    out.push(&t.data[4 + z + 1..]);
                }
            }
            _ => {}
        }
    }
    out
}

fn u30(b: &[u8], o: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    let mut shift = 0;
    loop {
        let x = *b.get(*o)?;
        *o += 1;
        v |= ((x & 0x7f) as u32) << shift;
        shift += 7;
        if x & 0x80 == 0 {
            return Some(v);
        }
        if shift > 35 {
            return None;
        }
    }
}

/// Constantes `uint` do constant pool de um bloco ABC.
pub fn abc_uints(abc: &[u8]) -> Vec<u32> {
    let mut o = 4; // minor + major version
    let mut out = Vec::new();
    let Some(n_int) = u30(abc, &mut o) else { return out };
    for _ in 1..n_int {
        if u30(abc, &mut o).is_none() {
            return out;
        }
    }
    let Some(n_uint) = u30(abc, &mut o) else { return out };
    for _ in 1..n_uint {
        match u30(abc, &mut o) {
            Some(v) => out.push(v),
            None => break,
        }
    }
    out
}
