//! Arquivos `.swz` (Engine/Init/Game/Dynamic): seções zlib cifradas com XOR + PRNG.
//! Formato documentado pela comunidade (brawlhalla-swz); a chave é uma constante `uint`
//! dentro do bytecode de `BrawlhallaAir.swf`.

use std::io::Read;

use anyhow::{bail, Result};
use flate2::read::ZlibDecoder;

use crate::{prng::Prng, swf};

const CHECKSUM_SEED: u32 = 771_006_925;

fn be32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// O cabeçalho carrega um checksum que só bate com a chave certa.
pub fn header_ok(swz: &[u8], key: u32) -> bool {
    let (Some(expected), Some(mix)) = (be32(swz, 0), be32(swz, 4)) else { return false };
    let mut prng = Prng::new(key ^ mix);
    let mut sum = CHECKSUM_SEED;
    for _ in 0..(key % 0x1f) + 5 {
        sum ^= prng.next_u32();
    }
    sum == expected
}

/// Procura entre as constantes `uint` do `BrawlhallaAir.swf` a que decifra o cabeçalho.
pub fn find_key(air_swf: &[u8], any_swz: &[u8]) -> Result<u32> {
    let body = swf::body(air_swf)?;
    let tags = swf::tags(&body)?;
    for abc in swf::abc_blocks(&tags) {
        for k in swf::abc_uints(abc) {
            if header_ok(any_swz, k) {
                return Ok(k);
            }
        }
    }
    bail!("chave do SWZ não encontrada no BrawlhallaAir.swf")
}

/// Decifra todas as seções do arquivo. Cada uma é um XML ou CSV de dados do jogo.
pub fn decrypt(swz: &[u8], key: u32) -> Result<Vec<Vec<u8>>> {
    if !header_ok(swz, key) {
        bail!("chave incorreta para este .swz");
    }
    let mut prng = Prng::new(key ^ be32(swz, 4).unwrap());
    for _ in 0..(key % 0x1f) + 5 {
        prng.next_u32();
    }
    let mut off = 8;
    let mut sections = Vec::new();
    while swz.len() - off > 12 {
        let csize = (be32(swz, off).unwrap() ^ prng.next_u32()) as usize;
        let usize_ = (be32(swz, off + 4).unwrap() ^ prng.next_u32()) as usize;
        let expected = be32(swz, off + 8).unwrap();
        off += 12;
        let Some(enc) = swz.get(off..off + csize) else {
            bail!("seção {} truncada", sections.len());
        };
        off += csize;

        let mut sum = prng.next_u32();
        let mut data = Vec::with_capacity(csize);
        for (i, &b) in enc.iter().enumerate() {
            let r = prng.next_u32();
            let bi = (i & 15) as u32;
            let plain = b ^ ((((0xffu32 << bi) & r) >> bi) as u8);
            data.push(plain);
            sum = (plain as u32) ^ sum.rotate_right((i % 7) as u32 + 1);
        }
        if sum != expected {
            bail!("checksum da seção {} não bate", sections.len());
        }
        let mut out = Vec::with_capacity(usize_);
        ZlibDecoder::new(&data[..]).read_to_end(&mut out)?;
        if out.len() != usize_ {
            bail!("seção {}: tamanho {} != {}", sections.len(), out.len(), usize_);
        }
        sections.push(out);
    }
    Ok(sections)
}
