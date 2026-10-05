use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

const DEFAULT_DIR: &str = r"D:\Steam\steamapps\common\Brawlhalla";

/// Pasta de instalação: `--dir`, depois `BRAWLHALLA_DIR`, depois o caminho padrão do Steam.
pub fn locate(explicit: Option<&Path>) -> Result<PathBuf> {
    let dir = match explicit {
        Some(p) => p.to_path_buf(),
        None => std::env::var_os("BRAWLHALLA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DIR)),
    };
    if !dir.join("BrawlhallaAir.swf").is_file() {
        bail!("BrawlhallaAir.swf não encontrado em {}", dir.display());
    }
    Ok(dir)
}
