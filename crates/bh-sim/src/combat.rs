//! Matemática do golpe, extraída do bytecode do jogo (`_-E4h._-V1c`, `_-E4h._-i2J`, `_-Z1a._-J2J`).
//!
//! Resumo:
//! - dano  = dano_base × r37[11 + Força_atacante − Defesa_alvo]
//! - d     = (dano_acumulado_do_alvo + dano_deste_golpe) / 100
//! - knockback = fixo + max(variável × (d + d²/2) × ImpulseMult_da_Força − ImpulseNegation, 0)
//! - direção = normalizar((alvo − atacante) + (±OffsetX, OffsetY)); se `ImpulseToPoint`, balística até um ponto
//! - o alvo fica `FixedStunTime` ticks em hitstun; o vetor vira a velocidade dele (unidades/tick do motor)

use bh_assets::powers::Ellipse;

/// Hurtbox padrão do corpo (`hurtboxTypes.DEFAULT`), centrada na posição do lutador.
pub const HURTBOX_W: f32 = 145.0;
pub const HURTBOX_H: f32 = 160.0;
/// Distância do centro do corpo (posição usada pelos golpes) até os pés.
pub const CENTER_TO_FEET: f32 = HURTBOX_H / 2.0;

/// `_-E4h._-r37`: multiplicador de dano indexado por `11 + Força_do_atacante − Defesa_do_alvo`.
pub const DAMAGE_BY_LEVEL_DIFF: [f32; 23] = [
    0.8349, 0.8384, 0.8437, 0.851, 0.8604, 0.8719, 0.8857, 0.9022, 0.9215, 0.9439, 0.9699, 1.0, 1.031, 1.0594, 1.0852, 1.1084, 1.129,
    1.1469, 1.1623, 1.1751, 1.1853, 1.1928, 1.1978,
];

/// Atrito horizontal durante o hitstun (`v10 = 4.0` quando o lutador está atordoado).
pub const HITSTUN_FRICTION: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatStats {
    pub strength: u32,
    pub defense: u32,
    /// `ImpulseMult` da Força (`StatTypes`).
    pub impulse_mult: f32,
}

impl Default for CombatStats {
    fn default() -> Self {
        CombatStats { strength: 5, defense: 5, impulse_mult: 1.0 }
    }
}

/// O elipse (centro/raios) toca o retângulo (centro/largura/altura)?
pub fn ellipse_hits_rect(e: Ellipse, rect_cx: f32, rect_cy: f32, w: f32, h: f32) -> bool {
    if e.rx <= 0.0 || e.ry <= 0.0 {
        return false;
    }
    let nx = e.cx.clamp(rect_cx - w / 2.0, rect_cx + w / 2.0);
    let ny = e.cy.clamp(rect_cy - h / 2.0, rect_cy + h / 2.0);
    let (dx, dy) = ((nx - e.cx) / e.rx, (ny - e.cy) / e.ry);
    dx * dx + dy * dy <= 1.0
}

pub fn damage_dealt(base: f32, attacker: &CombatStats, victim: &CombatStats) -> f32 {
    let idx = (11 + attacker.strength as i32 - victim.defense as i32).clamp(0, 22) as usize;
    base * DAMAGE_BY_LEVEL_DIFF[idx]
}

/// Magnitude do knockback (unidades/tick do motor).
pub fn knockback(fixed: f32, variable: f32, victim_damage: f32, hit_damage: f32, impulse_mult: f32, ignore_strength: bool) -> f32 {
    let d = (victim_damage + hit_damage) / 100.0;
    let mult = if ignore_strength { 1.0 } else { impulse_mult };
    let var = (variable * (d + d * d / 2.0) * mult).max(0.0);
    fixed + var
}

/// Direção do knockback (normalizada). `attacker`/`victim` são os centros dos corpos; `mirrored` = atacante olhando
/// para a esquerda. Coordenadas com y para baixo.
pub fn direction(attacker: (f32, f32), victim: (f32, f32), offset: (f32, f32), mirrored: bool, lock_45: bool) -> (f32, f32) {
    let ox = if mirrored { -offset.0 } else { offset.0 };
    let (mut x, mut y) = (victim.0 - attacker.0 + ox, victim.1 - attacker.1 + offset.1);
    if lock_45 {
        let (ax, ay) = (x.abs(), y.abs());
        if ax > ay {
            y = if y <= 0.0 { -ax } else { ax };
        } else if ax < ay {
            x = if x <= 0.0 { -ay } else { ay };
        }
    }
    let len = (x * x + y * y).sqrt();
    if len == 0.0 {
        return (0.0, -1.0);
    }
    (x / len, y / len)
}

/// `ImpulseToPoint`: velocidade (unidades/tick) que leva o alvo de `victim` a `target` em `ticks` ticks,
/// compensando o atrito de hitstun e a gravidade. Devolve `(vx, vy)` sem normalizar.
pub fn impulse_to_point(victim: (f32, f32), target: (f32, f32), ticks: u32, dt: f32, gravity: f32) -> (f32, f32) {
    let n = ticks.max(1) as f32;
    let dist = (target.0 - victim.0).abs();
    let decel = HITSTUN_FRICTION * dt;
    let mut vx = dist / (n * dt) + ((n - 1.0) * decel) / 2.0;
    if vx < n * decel {
        vx = (8.0 * dist).sqrt();
    }
    if target.0 < victim.0 {
        vx = -vx;
    }
    let vy = (target.1 - victim.1) / (n * dt) - (n * gravity * dt) / 2.0;
    (vx, vy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ell(cx: f32, cy: f32, rx: f32, ry: f32) -> Ellipse {
        Ellipse { cx, cy, rx, ry }
    }

    #[test]
    fn ellipse_vs_body_box() {
        // corpo em (0,0) com 145x160; golpe do martelo a 83 px na frente
        assert!(ellipse_hits_rect(ell(83.0, -4.0, 49.0, 24.0), 120.0, 0.0, HURTBOX_W, HURTBOX_H));
        // longe demais
        assert!(!ellipse_hits_rect(ell(83.0, -4.0, 49.0, 24.0), 400.0, 0.0, HURTBOX_W, HURTBOX_H));
        // raio zero nunca acerta
        assert!(!ellipse_hits_rect(ell(0.0, 0.0, 0.0, 10.0), 0.0, 0.0, HURTBOX_W, HURTBOX_H));
        // borda: precisa tocar de fato (cantos do elipse não contam)
        assert!(!ellipse_hits_rect(ell(100.0, 100.0, 10.0, 10.0), 0.0, 0.0, HURTBOX_W, HURTBOX_H));
    }

    #[test]
    fn damage_scales_with_strength_minus_defense() {
        let b = CombatStats { strength: 6, defense: 5, impulse_mult: 1.074 };
        // Bödvar (Força 6) contra Bödvar (Defesa 5): índice 12 -> 1.031
        assert!((damage_dealt(5.0, &b, &b) - 5.155).abs() < 1e-4);
        let weak = CombatStats { strength: 0, defense: 5, impulse_mult: 1.0 };
        let strong = CombatStats { strength: 11, defense: 5, impulse_mult: 1.0 };
        assert!(damage_dealt(10.0, &weak, &b) < 10.0 && damage_dealt(10.0, &strong, &b) > 10.0);
        // índice limitado ao intervalo da tabela
        let victim0 = CombatStats { strength: 5, defense: 0, impulse_mult: 1.0 };
        assert_eq!(damage_dealt(1.0, &strong, &victim0), DAMAGE_BY_LEVEL_DIFF[22]);
        let victim11 = CombatStats { strength: 5, defense: 11, impulse_mult: 1.0 };
        assert_eq!(damage_dealt(1.0, &weak, &victim11), DAMAGE_BY_LEVEL_DIFF[0]);
    }

    #[test]
    fn knockback_grows_with_accumulated_damage() {
        // HammerSide (variável 8, fixo 56) em Bödvar (ImpulseMult 1.074)
        let k0 = knockback(56.0, 8.0, 0.0, 16.0, 1.074, false);
        let k100 = knockback(56.0, 8.0, 100.0, 16.0, 1.074, false);
        let k200 = knockback(56.0, 8.0, 200.0, 16.0, 1.074, false);
        assert!(k0 > 56.0 && k100 > k0 && k200 > k100);
        // d = 0.16: 8 * (0.16 + 0.0128) * 1.074 = 1.4846 -> 57.48
        assert!((k0 - 57.4846).abs() < 1e-3, "{k0}");
        // sem componente variável (HammerNeutral): sempre o impulso fixo
        assert_eq!(knockback(50.0, 0.0, 300.0, 5.0, 1.074, false), 50.0);
        // IgnoreStrength remove o multiplicador da Força
        assert!(knockback(0.0, 10.0, 100.0, 0.0, 1.5, true) < knockback(0.0, 10.0, 100.0, 0.0, 1.5, false));
    }

    #[test]
    fn direction_uses_offset_and_mirror() {
        // alvo à direita do atacante, offset (1000, 200): quase horizontal, levemente para baixo
        let d = direction((0.0, 0.0), (120.0, 0.0), (1000.0, 200.0), false, false);
        assert!(d.0 > 0.97 && d.1 > 0.0 && d.1 < 0.25, "{d:?}");
        // atacante olhando para a esquerda inverte o x do offset
        let m = direction((0.0, 0.0), (-120.0, 0.0), (1000.0, 200.0), true, false);
        assert!((m.0 + d.0).abs() < 0.02 && (m.1 - d.1).abs() < 0.02, "{m:?} vs {d:?}");
        // vetor nulo: sobe
        assert_eq!(direction((5.0, 5.0), (5.0, 5.0), (0.0, 0.0), false, false), (0.0, -1.0));
        // 45 graus travados
        let l = direction((0.0, 0.0), (100.0, 10.0), (0.0, 0.0), false, true);
        assert!((l.0.abs() - l.1.abs()).abs() < 1e-5);
    }

    #[test]
    fn impulse_to_point_reaches_target_in_n_ticks() {
        let (dt, g, n) = (0.384, 3.75, 21u32);
        let (v0, target) = ((0.0f32, 0.0f32), (203.0f32, -12.0f32));
        let (mut vx, mut vy) = impulse_to_point(v0, target, n, dt, g);
        let (mut x, mut y) = v0;
        for _ in 0..n {
            x += vx * dt;
            y += vy * dt;
            vx = if vx > 0.0 { (vx - HITSTUN_FRICTION * dt).max(0.0) } else { (vx + HITSTUN_FRICTION * dt).min(0.0) };
            vy += g * dt;
        }
        assert!((x - target.0).abs() < 12.0 && (y - target.1).abs() < 12.0, "chegou em ({x:.1}, {y:.1})");
    }
}
