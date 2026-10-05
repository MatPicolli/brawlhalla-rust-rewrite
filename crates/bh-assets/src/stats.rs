//! Atributos de movimento por herói: `HeroType/Speed` aponta para a linha `Speed{n}` de `StatTypes` (Game.swz).
//! As unidades dos valores não são documentadas; quem os usa aplica uma escala (calibrada depois).

use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedStat {
    pub run_speed: f32,
    pub jump_x_impulse: f32,
    pub anim_speed: f32,
    pub acceleration: f32,
    pub friction: f32,
    pub air_run_speed: f32,
    pub air_acceleration: f32,
    pub air_friction: f32,
}

fn section<'a>(sections: &'a [Vec<u8>], root: &str) -> Option<String> {
    let tag = format!("<{root}>");
    sections.iter().find(|s| s.starts_with(tag.as_bytes())).map(|s| String::from_utf8_lossy(s).into_owned())
}

fn num(n: roxmltree::Node, tag: &str) -> Option<f32> {
    n.children().find(|c| c.has_tag_name(tag))?.text()?.trim().parse().ok()
}

/// Nível de velocidade (0..=11) do herói, p.ex. `Viking` -> 5.
pub fn hero_speed_level(game_sections: &[Vec<u8>], hero: &str) -> Result<u32> {
    let text = section(game_sections, "HeroTypes").ok_or_else(|| anyhow!("HeroTypes não encontrado"))?;
    let doc = roxmltree::Document::parse(&text)?;
    let node = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("HeroType") && n.attribute("HeroName") == Some(hero))
        .ok_or_else(|| anyhow!("herói '{hero}' não encontrado"))?;
    num(node, "Speed").map(|v| v as u32).ok_or_else(|| anyhow!("herói '{hero}' sem <Speed>"))
}

pub fn speed_stat(game_sections: &[Vec<u8>], level: u32) -> Result<SpeedStat> {
    let text = section(game_sections, "StatTypes").ok_or_else(|| anyhow!("StatTypes não encontrado"))?;
    let doc = roxmltree::Document::parse(&text)?;
    let name = format!("Speed{level}");
    let n = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("StatType") && n.attribute("StatName") == Some(name.as_str()))
        .ok_or_else(|| anyhow!("{name} não encontrado"))?;
    let get = |t: &str| num(n, t).ok_or_else(|| anyhow!("{name} sem <{t}>"));
    Ok(SpeedStat {
        run_speed: get("RunSpeed")?,
        jump_x_impulse: get("JumpXImpulse")?,
        anim_speed: get("AnimSpeed")?,
        acceleration: get("Acceleration")?,
        friction: get("Friction")?,
        air_run_speed: get("AirRunSpeed")?,
        air_acceleration: get("AirAcceleration")?,
        air_friction: get("AirFriction")?,
    })
}

pub fn hero_speed_stat(game_sections: &[Vec<u8>], hero: &str) -> Result<SpeedStat> {
    speed_stat(game_sections, hero_speed_level(game_sections, hero)?)
}

/// Níveis (0..=11) de Força, Destreza, Peso (= Defesa) e Velocidade do herói (`HeroType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeroLevels {
    pub strength: u32,
    pub dexterity: u32,
    pub weight: u32,
    pub speed: u32,
}

pub fn hero_levels(game_sections: &[Vec<u8>], hero: &str) -> Result<HeroLevels> {
    let text = section(game_sections, "HeroTypes").ok_or_else(|| anyhow!("HeroTypes não encontrado"))?;
    let doc = roxmltree::Document::parse(&text)?;
    let node = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("HeroType") && n.attribute("HeroName") == Some(hero))
        .ok_or_else(|| anyhow!("herói '{hero}' não encontrado"))?;
    let lvl = |t: &str| num(node, t).map(|v| (v as u32).min(11)).ok_or_else(|| anyhow!("herói '{hero}' sem <{t}>"));
    Ok(HeroLevels { strength: lvl("Strength")?, dexterity: lvl("Dexterity")?, weight: lvl("Weight")?, speed: lvl("Speed")? })
}

/// `ImpulseMult` da linha `Strength{level}` de `StatTypes`: multiplicador do knockback causado.
pub fn strength_impulse_mult(game_sections: &[Vec<u8>], level: u32) -> Result<f32> {
    let text = section(game_sections, "StatTypes").ok_or_else(|| anyhow!("StatTypes não encontrado"))?;
    let doc = roxmltree::Document::parse(&text)?;
    let name = format!("Strength{level}");
    let n = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("StatType") && n.attribute("StatName") == Some(name.as_str()))
        .ok_or_else(|| anyhow!("{name} não encontrado"))?;
    num(n, "ImpulseMult").ok_or_else(|| anyhow!("{name} sem <ImpulseMult>"))
}

/// Arma e golpes pesados (assinaturas) do herói para o slot `slot` (1 ou 2): neutro, lateral e para baixo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WeaponKit {
    pub weapon: String,
    pub sig_neutral: String,
    pub sig_side: String,
    pub sig_down: String,
}

pub fn hero_weapon_kit(game_sections: &[Vec<u8>], hero: &str, slot: u32) -> Result<WeaponKit> {
    let text = section(game_sections, "HeroTypes").ok_or_else(|| anyhow!("HeroTypes não encontrado"))?;
    let doc = roxmltree::Document::parse(&text)?;
    let node = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("HeroType") && n.attribute("HeroName") == Some(hero))
        .ok_or_else(|| anyhow!("herói '{hero}' não encontrado"))?;
    let get = |t: String| node.children().find(|c| c.has_tag_name(t.as_str())).and_then(|c| c.text()).unwrap_or("").trim().to_string();
    Ok(WeaponKit {
        weapon: get(format!("BaseWeapon{slot}")),
        sig_neutral: get(format!("SpecialPower{slot}")),
        sig_side: get(format!("SpecialPower{slot}_Forward")),
        sig_down: get(format!("SpecialPower{slot}_Down")),
    })
}
