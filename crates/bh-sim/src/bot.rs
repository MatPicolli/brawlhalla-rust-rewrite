//! Adversário simples controlado pelo computador. Não tenta imitar a IA do jogo original: só aproxima,
//! ataca quando está ao alcance, esquiva de vez em quando e volta ao palco quando é lançado para fora.

use crate::world::{ActorInput, World};

#[derive(Debug, Clone)]
pub struct Bot {
    rng: u32,
    /// Ticks restantes segurando o golpe pesado (carga).
    hold_heavy: u32,
    jump_cd: u32,
    /// Ticks até a próxima decisão de ataque (evita atacar em todo tick).
    wait: u32,
}

/// Alcance horizontal a partir do qual o bot para de correr e ataca.
const REACH: f32 = 190.0;

impl Bot {
    pub fn new(seed: u32) -> Self {
        Bot { rng: seed | 1, hold_heavy: 0, jump_cd: 0, wait: 30 }
    }

    fn rand(&mut self) -> f32 {
        // xorshift32
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn think(&mut self, w: &World, me: usize, target: usize) -> ActorInput {
        let mut inp = ActorInput::default();
        let (a, t) = (&w.actors[me], &w.actors[target]);
        self.jump_cd = self.jump_cd.saturating_sub(1);
        self.wait = self.wait.saturating_sub(1);
        if a.stun > 0 {
            self.hold_heavy = 0;
            return inp;
        }
        if self.hold_heavy > 0 {
            self.hold_heavy -= 1;
            inp.heavy_held = self.hold_heavy > 0;
            return inp;
        }
        let f = &a.fighter;
        let (dx, dy) = (t.fighter.x - f.x, t.fighter.y - f.y);
        let toward = if dx >= 0.0 { 1.0 } else { -1.0 };

        // --- fora do palco: volta para onde nasceu, gastando pulos quando começa a cair
        let floor_below = w.geo.sweep((f.x, f.y - 2.0), (0.0, 4000.0), false, None).is_some();
        if !f.grounded() && !floor_below {
            let home = a.spawn.0 - f.x;
            inp.input.move_x = if home >= 0.0 { 1.0 } else { -1.0 };
            let can_jump = f.wall != 0 || f.air_jumps_used < a.params.t.air_jumps;
            if f.vy > 5.0 && can_jump && self.jump_cd == 0 {
                inp.input.jump = true;
                self.jump_cd = 14;
            }
            return inp;
        }
        if a.attack.is_some() || a.dodge.is_some() {
            return inp;
        }

        // --- longe: corre até o alvo, pulando se ele está bem acima
        if dx.abs() > REACH || dy.abs() > 160.0 {
            if dx.abs() > 40.0 {
                inp.input.move_x = toward;
            }
            if dy < -180.0 && self.jump_cd == 0 && (f.grounded() || (f.vy > 0.0 && f.air_jumps_used < a.params.t.air_jumps)) {
                inp.input.jump = true;
                self.jump_cd = 20;
            }
            return inp;
        }

        // --- ao alcance
        if t.attack.is_some() && a.dodge_cooldown == 0 && self.rand() < 0.08 {
            inp.dodge = true;
            return inp;
        }
        if (f.facing as f32) != toward {
            // vira para o alvo antes de atacar
            inp.input.move_x = toward;
            return inp;
        }
        if a.attack_cooldown > 0 || self.wait > 0 {
            return inp;
        }
        self.wait = 20 + (self.rand() * 30.0) as u32;
        let r = self.rand();
        if r < 0.45 {
            inp.attack = true;
            inp.input.move_x = toward;
        } else if r < 0.7 {
            inp.attack = true;
        } else if r < 0.85 {
            inp.attack = true;
            inp.input.down = true;
        } else {
            inp.heavy = true;
            inp.heavy_held = true;
            self.hold_heavy = 15 + (self.rand() * 25.0) as u32;
        }
        inp
    }
}
