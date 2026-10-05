//! Container simples usado por `anims/*.anm` e `languages/*.bin`: `[u32 LE tamanho][zlib]`.

use std::io::Read;

use anyhow::{bail, Context, Result};
use flate2::read::ZlibDecoder;

pub fn decode(raw: &[u8]) -> Result<Vec<u8>> {
    if raw.len() < 4 {
        bail!("arquivo curto demais");
    }
    let expected = u32::from_le_bytes(raw[..4].try_into()?) as usize;
    let mut out = Vec::with_capacity(expected);
    ZlibDecoder::new(&raw[4..])
        .read_to_end(&mut out)
        .context("zlib inválido")?;
    if out.len() != expected {
        bail!("tamanho {} != esperado {}", out.len(), expected);
    }
    Ok(out)
}
