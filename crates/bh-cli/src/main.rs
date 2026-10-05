use std::{fs, path::PathBuf};

use anyhow::Result;
use bh_assets::{anm, bones, install, movie::Movie, powers::PowerTable, swz, zblob};
use clap::{Parser, Subcommand};

/// Ferramentas de assets do Brawlhalla recomp.
#[derive(Parser)]
struct Cli {
    /// Pasta de instalação do jogo (padrão: $BRAWLHALLA_DIR ou o caminho do Steam)
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Mostra a chave dos .swz e quantas seções cada arquivo tem
    Key,
    /// Decifra Engine/Init/Game/Dynamic.swz para <out>/<nome>/<n>.{xml,csv}
    SwzDump { out: PathBuf },
    /// Descomprime um .anm ou language.*.bin (caminho relativo à pasta do jogo)
    Unz { file: PathBuf, out: PathBuf },
    /// Faz o parse de todos os anims/*.anm e confere que cada animação fecha nos bytes declarados
    AnmCheck,
    /// Lista as partes de um quadro: anm-frame Animation_Hammer.anm Ready 0
    AnmFrame { name: String, anim: String, frame: usize },
    /// Tesela todas as formas de Gfx_*.swf e bones/*.swf e conta falhas
    SwfCheck,
    /// Mostra um poder como o simulador o enxerga (ex.: power-info HammerNeutral)
    PowerInfo { name: String },
    /// Mostra os ids dos quadros de uma animação (anm-ids Animation_Hammer.anm a__HammerAnimation AttackCombo1)
    AnmIds { name: String, class: String, anim: String },
    /// Resume um .anm (nome relativo a anims/, ex. Animation_Aang.anm)
    AnmInfo { name: String },
}

const SWZ_FILES: [&str; 4] = ["Engine", "Init", "Game", "Dynamic"];

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = install::locate(cli.dir.as_deref())?;
    let key = || -> Result<u32> {
        swz::find_key(
            &fs::read(dir.join("BrawlhallaAir.swf"))?,
            &fs::read(dir.join("Init.swz"))?,
        )
    };

    match cli.cmd {
        Cmd::Key => {
            let key = key()?;
            println!("chave: {key}");
            for n in SWZ_FILES {
                let secs = swz::decrypt(&fs::read(dir.join(format!("{n}.swz")))?, key)?;
                println!("{n}.swz: {} seções", secs.len());
            }
        }
        Cmd::SwzDump { out } => {
            let key = key()?;
            for n in SWZ_FILES {
                let secs = swz::decrypt(&fs::read(dir.join(format!("{n}.swz")))?, key)?;
                let d = out.join(n);
                fs::create_dir_all(&d)?;
                for (i, s) in secs.iter().enumerate() {
                    let ext = if s.first() == Some(&b'<') { "xml" } else { "csv" };
                    fs::write(d.join(format!("{i:03}.{ext}")), s)?;
                }
                println!("{n}: {} seções -> {}", secs.len(), d.display());
            }
        }
        Cmd::AnmCheck => {
            let (mut ok, mut bad, mut anims, mut frames) = (0, 0, 0usize, 0usize);
            let mut files: Vec<_> = fs::read_dir(dir.join("anims"))?.collect::<Result<_, _>>()?;
            files.sort_by_key(|e| e.file_name());
            for e in files {
                let name = e.file_name().to_string_lossy().into_owned();
                match zblob::decode(&fs::read(e.path())?).and_then(|d| anm::parse(&d)) {
                    Ok(f) => {
                        ok += 1;
                        for en in &f.entries {
                            anims += en.anims.len();
                            frames += en.anims.iter().map(|a| a.frames.len()).sum::<usize>();
                        }
                    }
                    Err(err) => {
                        bad += 1;
                        println!("FALHA {name}: {err:#}");
                    }
                }
            }
            println!("{ok} ok, {bad} com falha; {anims} animações, {frames} quadros");
        }
        Cmd::AnmFrame { name, anim, frame } => {
            let init = swz::decrypt(&fs::read(dir.join("Init.swz"))?, key()?)?;
            let names = bones::bone_names(&init)?;
            let f = anm::parse(&zblob::decode(&fs::read(dir.join("anims").join(&name))?)?)?;
            for en in &f.entries {
                for a in en.anims.iter().filter(|a| a.name == anim) {
                    let Some(fr) = a.frames.get(frame) else { continue };
                    println!("{} :: {} quadro {} ({} partes)", en.key, a.name, frame, fr.parts.len());
                    for (i, p) in fr.parts.iter().enumerate() {
                        let n = names.get(p.bone as usize).map(String::as_str).unwrap_or("?");
                        println!("  {i:2} {n:<34} f{} op={:.2} m={:?}", p.frame_no, p.opacity, p.m);
                    }
                }
            }
        }
        Cmd::SwfCheck => {
            let mut files: Vec<PathBuf> = Vec::new();
            for e in fs::read_dir(&dir)? {
                let p = e?.path();
                let n = p.file_name().unwrap().to_string_lossy().into_owned();
                if n.starts_with("Gfx_") && n.ends_with(".swf") {
                    files.push(p);
                }
            }
            for e in fs::read_dir(dir.join("bones"))? {
                files.push(e?.path());
            }
            files.sort();
            let (mut shapes, mut failed, mut empty, mut tris, mut broken_files) = (0usize, 0usize, 0usize, 0usize, 0usize);
            for f in &files {
                let mut mv = match Movie::load(f) {
                    Ok(m) => m,
                    Err(e) => {
                        broken_files += 1;
                        println!("FALHA {}: {e:#}", f.display());
                        continue;
                    }
                };
                for id in mv.shape_ids() {
                    shapes += 1;
                    match mv.shape(id) {
                        None => failed += 1,
                        Some(s) if s.mesh.is_empty() => empty += 1,
                        Some(s) => tris += s.mesh.idx.len() / 3,
                    }
                }
            }
            println!("{} arquivos ({broken_files} falharam), {shapes} formas: {failed} com erro, {empty} vazias, {tris} triângulos", files.len());
        }
        Cmd::PowerInfo { name } => {
            let game = swz::decrypt(&fs::read(dir.join("Game.swz"))?, key()?)?;
            let table = PowerTable::load(&game);
            println!("{} poderes", table.len());
            match table.get(&name) {
                Some(p) => {
                    println!("{} anim={} alvo={} ar={} recover={} stun={} cooldown={} combo={:?} se-acertar={:?} impulso-ao-ponto={}", p.name, p.cast_anim, p.target_method, p.is_air, p.recover, p.stun, p.cooldown, p.combo, p.combo_if_hit, p.impulse_to_point);
                    for (h, hit) in p.hits.iter().enumerate() {
                        println!(
                            "  hit {h}: pre={} ativo={} eventos={} dano={} fixo={} variavel={} offset={:?}",
                            hit.pre, hit.active, hit.events(), hit.damage_at(0), hit.fixed_at(0), hit.variable_at(0), (hit.offset_at(0), hit.offset_at(hit.active))
                        );
                        if let Some(b) = hit.boxes.first() {
                            println!("      evento 0: {b:?}");
                        }
                    }
                    println!("  agenda (hit, evento, tick): {:?}", p.schedule());
                }
                None => println!("poder '{name}' não encontrado"),
            }
        }
        Cmd::AnmIds { name, class, anim } => {
            let f = anm::parse(&zblob::decode(&fs::read(dir.join("anims").join(&name))?)?)?;
            for en in f.entries.iter().filter(|e| e.class == class) {
                for a in en.anims.iter().filter(|a| a.name == anim) {
                    println!("{} hdr={:?} extra={:?}", a.name, a.header, a.extra);
                    println!("ids: {:?}", a.frames.iter().map(|f| f.id).collect::<Vec<_>>());
                    println!("partes: {:?}", a.frames.iter().map(|f| f.parts.len()).collect::<Vec<_>>());
                }
            }
        }
        Cmd::AnmInfo { name } => {
            let f = anm::parse(&zblob::decode(&fs::read(dir.join("anims").join(&name))?)?)?;
            for en in &f.entries {
                println!("{} ({} anims)", en.key, en.anims.len());
                for a in &en.anims {
                    println!("  {:<20} hdr={:?} extra={} quadros={}", a.name, a.header, a.extra.len(), a.frames.len());
                }
            }
        }
        Cmd::Unz { file, out } => {
            let data = zblob::decode(&fs::read(dir.join(file))?)?;
            fs::write(&out, &data)?;
            println!("{} bytes -> {}", data.len(), out.display());
        }
    }
    Ok(())
}
