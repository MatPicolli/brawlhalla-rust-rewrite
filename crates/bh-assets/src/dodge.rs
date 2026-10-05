//! Esquivas (`DodgeTypes` em `Game.swz`) e os modificadores de tempo da Destreza (`StatTypes`).
//!
//! Velocidade da esquiva (bytecode `_-36I`): `60 × fator × multiplicador`, onde o fator vai de 0,6 a 1,0 conforme
//! a velocidade atual já aponta para a direção da esquiva, e o multiplicador varia entre `Speed*Mult` e
//! `Speed*MaxMult` com a velocidade atual. Durante a esquiva não há gravidade nem atrito; o lutador fica
//! invulnerável a partir de `StartInvuln` ticks.

use anyhow::{anyhow, Result};

use crate::powers::TimeMods;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DodgeDir {
    Neutral,
    Side,
    SideUp,
    SideDown,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DodgeType {
    pub name: String,
    pub dir: DodgeDir,
    /// "Ground", "Air" ou "Both".
    pub air_type: String,
    pub fast: bool,
    pub duration: u32,
    pub start_invuln: u32,
    pub speed_x_formula: String,
    pub speed_x: (f32, f32),
    pub speed_y_formula: String,
    pub speed_y: (f32, f32),
    pub cooldown: u32,
    pub cooldown_ground: u32,
    pub cooldown_landed: u32,
    /// Multiplicadores da velocidade ao terminar (x, y) e quando a direção contrária está apertada.
    pub exit: (f32, f32),
    pub exit_back: (f32, f32),
}

fn section(sections: &[Vec<u8>], root: &str) -> Option<String> {
    let tag = format!("<{root}>");
    sections.iter().find(|s| s.starts_with(tag.as_bytes())).map(|s| String::from_utf8_lossy(s).into_owned())
}

fn text<'a>(n: roxmltree::Node<'a, 'a>, tag: &str) -> &'a str {
    n.children().find(|c| c.has_tag_name(tag)).and_then(|c| c.text()).unwrap_or("").trim()
}

fn num(n: roxmltree::Node, tag: &str) -> f32 {
    text(n, tag).parse().unwrap_or(0.0)
}

/// Esquivas do conjunto padrão (a primeira ocorrência de cada nome).
pub fn dodge_types(game_sections: &[Vec<u8>]) -> Result<Vec<DodgeType>> {
    let xml = section(game_sections, "DodgeTypes").ok_or_else(|| anyhow!("DodgeTypes não encontrado"))?;
    let doc = roxmltree::Document::parse(&xml)?;
    let mut out: Vec<DodgeType> = Vec::new();
    for n in doc.root_element().children().filter(|n| n.has_tag_name("DodgeType")) {
        let name = n.attribute("DodgeName").unwrap_or("").to_string();
        if name == "Template" || out.iter().any(|d| d.name == name) {
            continue;
        }
        let dir = match text(n, "Direction") {
            "Side" => DodgeDir::Side,
            "SideUp" => DodgeDir::SideUp,
            "SideDown" => DodgeDir::SideDown,
            "Up" => DodgeDir::Up,
            "Down" => DodgeDir::Down,
            _ => DodgeDir::Neutral,
        };
        out.push(DodgeType {
            name,
            dir,
            air_type: text(n, "AirType").to_string(),
            fast: text(n, "IsFast").eq_ignore_ascii_case("true"),
            duration: num(n, "Duration") as u32,
            start_invuln: num(n, "StartInvuln") as u32,
            speed_x_formula: text(n, "SpeedXFormula").to_string(),
            speed_x: (num(n, "SpeedXMult"), num(n, "SpeedXMaxMult")),
            speed_y_formula: text(n, "SpeedYFormula").to_string(),
            speed_y: (num(n, "SpeedYMult"), num(n, "SpeedYMaxMult")),
            cooldown: num(n, "Cooldown") as u32,
            cooldown_ground: num(n, "CooldownGround") as u32,
            cooldown_landed: num(n, "CooldownLanded") as u32,
            exit: (num(n, "ExitX"), num(n, "ExitY")),
            exit_back: (num(n, "ExitBackX"), num(n, "ExitBackY")),
        });
    }
    Ok(out)
}

/// Esquiva não rápida para a direção e a situação (no ar ou no chão).
pub fn pick(types: &[DodgeType], dir: DodgeDir, airborne: bool) -> Option<&DodgeType> {
    types.iter().find(|d| {
        !d.fast && d.dir == dir && (d.air_type == "Both" || (airborne && d.air_type == "Air") || (!airborne && d.air_type == "Ground"))
    })
}

/// Modificadores de tempo da linha `Dexterity{level}` (o jogo divide os tempos por eles).
pub fn time_mods(game_sections: &[Vec<u8>], dexterity: u32) -> Result<TimeMods> {
    let xml = section(game_sections, "StatTypes").ok_or_else(|| anyhow!("StatTypes não encontrado"))?;
    let doc = roxmltree::Document::parse(&xml)?;
    let name = format!("Dexterity{dexterity}");
    let n = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("StatType") && n.attribute("StatName") == Some(name.as_str()))
        .ok_or_else(|| anyhow!("{name} não encontrado"))?;
    let get = |t: &str| {
        let v = num(n, t);
        if v > 0.0 { v } else { 1.0 }
    };
    Ok(TimeMods { recover: get("RecoverMod"), air_recover: get("AirRecoverMod"), sig_recover: get("SigRecoverMod"), min_charge: get("MinChargeMod") })
}
