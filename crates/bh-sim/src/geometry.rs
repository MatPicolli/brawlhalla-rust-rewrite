//! Colisão do mapa: superfícies (segmentos) com normal fixa e bloqueio de um lado só, e a varredura de um
//! ponto ao longo de um vetor. Segue o comportamento do motor original (`_-B3Y._-b3i`, `_-93X._-s1F`):
//! - superfícies duras bloqueiam só quem se move contra a normal; a normal aponta para fora do sólido e é
//!   obtida por paridade de raio (número ímpar de superfícies duras de um lado = interior);
//! - superfícies finas (soft) têm a normal para cima e só bloqueiam quem parte do lado da normal.
//!
//! Coordenadas do Flash: x para a direita, y para baixo.

use bh_assets::level::{CollisionKind, Level};

pub type P = (f32, f32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Hard,
    Soft,
}

#[derive(Debug, Clone, Copy)]
pub struct Surface {
    pub a: P,
    pub b: P,
    pub kind: Kind,
    /// Normal unitária, apontando para fora do sólido (para cima nas plataformas finas).
    pub n: P,
}

impl Surface {
    pub fn is_vertical(&self) -> bool {
        self.a.0 == self.b.0
    }
    pub fn is_horizontal(&self) -> bool {
        self.a.1 == self.b.1
    }
    /// Dá para ficar em pé: a normal aponta para cima e a superfície não é vertical.
    pub fn is_floor(&self) -> bool {
        !self.is_vertical() && self.n.1 < 0.0
    }
    pub fn top(&self) -> f32 {
        self.a.1.min(self.b.1)
    }
    pub fn bottom(&self) -> f32 {
        self.a.1.max(self.b.1)
    }
    /// Direção unitária ao longo da superfície com componente x positiva (ou para baixo, se vertical).
    pub fn dir(&self) -> P {
        let (mut dx, mut dy) = (self.b.0 - self.a.0, self.b.1 - self.a.1);
        if dx < 0.0 || (dx == 0.0 && dy < 0.0) {
            dx = -dx;
            dy = -dy;
        }
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        (dx / len, dy / len)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Hit {
    pub surf: usize,
    /// Ponto de contato.
    pub point: P,
    /// Fração do vetor percorrida até o contato (0..=1).
    pub t: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Geometry {
    pub surfaces: Vec<Surface>,
}

fn cross(a: P, b: P) -> f32 {
    a.0 * b.1 - a.1 * b.0
}

/// Desloca a coordenada para o meio de uma célula de 10 (os vértices dos mapas ficam em múltiplos de 10),
/// para o raio de paridade não passar exatamente por um vértice.
fn snap(v: f32) -> f32 {
    v + (5.0 - v.rem_euclid(10.0))
}

impl Geometry {
    /// Constrói a geometria; `normal` explícita (do XML) tem prioridade sobre a calculada.
    pub fn new(segs: &[(P, P, Kind, Option<P>)]) -> Self {
        let hard: Vec<(P, P)> = segs.iter().filter(|s| s.2 == Kind::Hard).map(|s| (s.0, s.1)).collect();
        let mut surfaces = Vec::with_capacity(segs.len());
        for (i, &(a, b, kind, explicit)) in segs.iter().enumerate() {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len == 0.0 {
                continue;
            }
            // perpendicular "à esquerda" do sentido a->b; o sinal é decidido abaixo
            let mut n = (dy / len, -dx / len);
            if let Some(e) = explicit.filter(|e| e.0 != 0.0 || e.1 != 0.0) {
                let l = (e.0 * e.0 + e.1 * e.1).sqrt();
                surfaces.push(Surface { a, b, kind, n: (e.0 / l, e.1 / l) });
                continue;
            }
            match kind {
                Kind::Soft => {
                    if n.1 > 0.0 || (n.1 == 0.0 && n.0 > 0.0) {
                        n = (-n.0, -n.1);
                    }
                }
                Kind::Hard => {
                    let (mx, my) = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
                    let self_idx = segs[..i].iter().filter(|s| s.2 == Kind::Hard).count();
                    if dx == 0.0 {
                        // raio para +x: ímpar = o interior está à direita, a normal aponta para a esquerda
                        let inside_right = Self::ray_parity(&hard, self_idx, mx, snap(my), true);
                        n = if inside_right { (-1.0, 0.0) } else { (1.0, 0.0) };
                    } else {
                        // raio para +y (baixo): ímpar = o interior está abaixo, a normal aponta para cima
                        let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
                        let x = snap(mx).clamp(lo + (hi - lo) * 0.25, hi - (hi - lo) * 0.25);
                        let y_on = a.1 + (x - a.0) * (dy / dx);
                        let inside_below = Self::ray_parity(&hard, self_idx, x, y_on, false);
                        if (n.1 > 0.0) == inside_below {
                            n = (-n.0, -n.1);
                        }
                    }
                }
            }
            surfaces.push(Surface { a, b, kind, n });
        }
        Geometry { surfaces }
    }

    /// Conta as superfícies duras cruzadas por um raio a partir de (x, y) para +x (`horizontal`) ou +y.
    fn ray_parity(hard: &[(P, P)], skip: usize, x: f32, y: f32, horizontal: bool) -> bool {
        let mut n = 0;
        for (i, &(a, b)) in hard.iter().enumerate() {
            if i == skip {
                continue;
            }
            if horizontal {
                let (lo, hi) = (a.1.min(b.1), a.1.max(b.1));
                if a.1 == b.1 || y < lo || y >= hi {
                    continue;
                }
                let xi = a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1);
                if xi > x {
                    n += 1;
                }
            } else {
                let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
                if a.0 == b.0 || x < lo || x >= hi {
                    continue;
                }
                let yi = a.1 + (x - a.0) * (b.1 - a.1) / (b.0 - a.0);
                if yi > y {
                    n += 1;
                }
            }
        }
        n % 2 == 1
    }

    pub fn from_level(level: &Level) -> Self {
        let mut segs = Vec::new();
        for c in &level.collisions {
            let kind = match c.kind {
                CollisionKind::Hard | CollisionKind::NoSlide | CollisionKind::Bouncy => Kind::Hard,
                CollisionKind::Soft => Kind::Soft,
                CollisionKind::Lava | CollisionKind::Other => continue,
            };
            segs.push(((c.x1, c.y1), (c.x2, c.y2), kind, c.normal));
        }
        Geometry::new(&segs)
    }

    /// O movimento `v` passa pela superfície sem colidir (está saindo pelo lado da normal)?
    fn passes(s: &Surface, v: P) -> bool {
        let (nx, ny) = s.n;
        if s.is_vertical() {
            v.0 != 0.0 && (v.0 > 0.0) == (nx > 0.0)
        } else if s.is_horizontal() {
            v.1 != 0.0 && (v.1 > 0.0) == (ny > 0.0)
        } else if v.0 == 0.0 {
            (v.1 > 0.0) == (ny > 0.0)
        } else if v.1 == 0.0 {
            (v.0 > 0.0) == (nx > 0.0)
        } else {
            (v.1 > 0.0) == (ny > 0.0) && (v.0 > 0.0) == (nx > 0.0)
        }
    }

    /// Primeiro contato do ponto `p` movendo-se por `v`. `hard_only` ignora as plataformas finas.
    pub fn sweep(&self, p: P, v: P, hard_only: bool, ignore: Option<usize>) -> Option<Hit> {
        if v.0 == 0.0 && v.1 == 0.0 {
            return None;
        }
        let mut best: Option<Hit> = None;
        for (i, s) in self.surfaces.iter().enumerate() {
            if Some(i) == ignore || (hard_only && s.kind == Kind::Soft) || Self::passes(s, v) {
                continue;
            }
            if s.kind == Kind::Soft {
                // só segura quem parte do lado da normal (de cima)
                let d = (p.0 - s.a.0) * s.n.0 + (p.1 - s.a.1) * s.n.1;
                if d < 0.0 {
                    continue;
                }
            }
            let e = (s.b.0 - s.a.0, s.b.1 - s.a.1);
            let denom = cross(v, e);
            if denom.abs() < 1e-9 {
                continue;
            }
            let qp = (s.a.0 - p.0, s.a.1 - p.1);
            let t = cross(qp, e) / denom;
            let u = cross(qp, v) / denom;
            if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
                continue;
            }
            if best.is_none_or(|b| t < b.t) {
                best = Some(Hit { surf: i, point: (p.0 + v.0 * t, p.1 + v.1 * t), t });
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plataforma retangular 0..400 x 100..300 e uma plataforma fina acima dela.
    fn block() -> Geometry {
        Geometry::new(&[
            ((0.0, 100.0), (400.0, 100.0), Kind::Hard, None),   // topo
            ((400.0, 100.0), (400.0, 300.0), Kind::Hard, None), // lado direito
            ((0.0, 300.0), (400.0, 300.0), Kind::Hard, None),   // fundo (mesmo sentido do topo, como no XML)
            ((0.0, 300.0), (0.0, 100.0), Kind::Hard, None),     // lado esquerdo
            ((100.0, -150.0), (300.0, -150.0), Kind::Soft, None),
        ])
    }

    #[test]
    fn normals_point_out_of_the_solid() {
        let g = block();
        assert_eq!(g.surfaces[0].n, (0.0, -1.0), "topo: para cima");
        assert_eq!(g.surfaces[1].n, (1.0, 0.0), "lado direito: para a direita");
        assert_eq!(g.surfaces[2].n, (0.0, 1.0), "fundo: para baixo");
        assert_eq!(g.surfaces[3].n, (-1.0, 0.0), "lado esquerdo: para a esquerda");
        assert_eq!(g.surfaces[4].n, (0.0, -1.0), "plataforma fina: para cima");
        assert!(g.surfaces[0].is_floor() && !g.surfaces[2].is_floor() && !g.surfaces[1].is_floor());
    }

    #[test]
    fn slanted_underside_normal_points_down_and_out() {
        // polígono do Brawlhaven: topo, chanfro esquerdo de baixo, fundo, chanfro direito de baixo, quina superior esquerda
        let g = Geometry::new(&[
            ((780.0, 1170.0), (1420.0, 1170.0), Kind::Hard, None),
            ((740.0, 1220.0), (860.0, 1340.0), Kind::Hard, None),
            ((860.0, 1340.0), (1330.0, 1340.0), Kind::Hard, None),
            ((1330.0, 1340.0), (1420.0, 1170.0), Kind::Hard, None),
            ((740.0, 1220.0), (780.0, 1170.0), Kind::Hard, None),
        ]);
        assert!(g.surfaces[0].n.1 < 0.0);
        let s = g.surfaces[1].n;
        assert!(s.0 < 0.0 && s.1 > 0.0, "chanfro esquerdo de baixo aponta para baixo-esquerda: {s:?}");
        assert!(g.surfaces[2].n.1 > 0.0);
        let r = g.surfaces[3].n;
        assert!(r.0 > 0.0 && r.1 > 0.0, "chanfro direito de baixo aponta para baixo-direita: {r:?}");
        let c = g.surfaces[4].n;
        assert!(c.0 < 0.0 && c.1 < 0.0, "quina superior esquerda aponta para cima-esquerda: {c:?}");
    }

    #[test]
    fn sweep_is_one_sided() {
        let g = block();
        // caindo sobre o topo: bate
        let h = g.sweep((50.0, 50.0), (0.0, 100.0), false, None).expect("deveria bater no topo");
        assert_eq!((h.surf, h.point), (0, (50.0, 100.0)));
        // subindo de dentro para fora pelo topo: não bate (sai pelo lado da normal)
        assert!(g.sweep((50.0, 150.0), (0.0, -100.0), false, None).is_none());
        // subindo por baixo: bate no fundo
        let h = g.sweep((50.0, 350.0), (0.0, -100.0), false, None).expect("deveria bater no fundo");
        assert_eq!(h.surf, 2);
        // andando contra o lado esquerdo
        let h = g.sweep((-50.0, 200.0), (100.0, 0.0), false, None).expect("deveria bater na parede");
        assert_eq!(h.surf, 3);
    }

    #[test]
    fn soft_platform_only_blocks_from_above() {
        let g = block();
        assert!(g.sweep((200.0, -100.0), (0.0, -100.0), false, None).is_none(), "subindo atravessa");
        let h = g.sweep((200.0, -200.0), (0.0, 100.0), false, None).expect("caindo de cima para");
        assert_eq!(h.surf, 4);
        assert!(g.sweep((200.0, -200.0), (0.0, 100.0), true, None).is_none(), "ignorada com hard_only");
        // partindo de baixo da plataforma e descendo: não é ela que segura
        assert!(g.sweep((200.0, -140.0), (0.0, 100.0), false, None).is_none());
    }

    #[test]
    fn explicit_normal_wins() {
        let g = Geometry::new(&[((0.0, 0.0), (100.0, 0.0), Kind::Hard, Some((0.0, 2.0)))]);
        assert_eq!(g.surfaces[0].n, (0.0, 1.0));
    }
}
