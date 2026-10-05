//! Mapas (`<LevelDesc>` em `Dynamic.swz`): arte, plataformas aninhadas, colisões e spawns.
//! Sistema de coordenadas do Flash: x para a direita, y para baixo.

use anyhow::{Context, Result};
use roxmltree::Node;

#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Matriz afim 2D no estilo Flash (a, b, c, d, tx, ty): x' = a*x + c*y + tx, y' = b*x + d*y + ty.
#[derive(Debug, Clone, Copy)]
pub struct Mat(pub [f32; 6]);

impl Mat {
    pub const ID: Mat = Mat([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    /// `self` aplicada depois de `inner` (inner é o espaço do filho).
    pub fn then_inner(self, inner: Mat) -> Mat {
        let [a, b, c, d, tx, ty] = self.0;
        let [ia, ib, ic, id, itx, ity] = inner.0;
        Mat([
            a * ia + c * ib,
            b * ia + d * ib,
            a * ic + c * id,
            b * ic + d * id,
            a * itx + c * ity + tx,
            b * itx + d * ity + ty,
        ])
    }

    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, tx, ty] = self.0;
        (a * x + c * y + tx, b * x + d * y + ty)
    }

    /// Translação, escala e rotação (graus) na ordem de composição do Flash.
    pub fn trs(x: f32, y: f32, sx: f32, sy: f32, rot_deg: f32) -> Mat {
        let (s, c) = rot_deg.to_radians().sin_cos();
        Mat([c * sx, s * sx, -s * sy, c * sy, x, y])
    }
}

/// Uma imagem de `mapArt/<asset_dir>/` posicionada no mundo.
#[derive(Debug, Clone)]
pub struct Placed {
    /// Nome relativo a `mapArt/<asset_dir>/` (pode conter `../`).
    pub asset: String,
    /// Transformação do canto superior esquerdo da imagem (tamanho `w`x`h`) para o mundo.
    pub m: Mat,
    pub w: f32,
    pub h: f32,
    pub instance: String,
    /// Plataformas temáticas (Christmas etc.) só aparecem com o tema ativo.
    pub theme: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionKind {
    Hard,
    Soft,
    NoSlide,
    Bouncy,
    Lava,
    Other,
}

/// Segmento de colisão: horizontal (`y1 == y2`) ou vertical (`x1 == x2`).
#[derive(Debug, Clone)]
pub struct Collision {
    pub kind: CollisionKind,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub team: Option<i32>,
    /// `NormalX`/`NormalY` do XML, quando presentes (senão o jogo calcula a normal ao carregar).
    pub normal: Option<(f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct Level {
    pub name: String,
    pub asset_dir: String,
    pub camera: Rect,
    pub background: Option<String>,
    pub placed: Vec<Placed>,
    pub collisions: Vec<Collision>,
    pub respawns: Vec<(f32, f32)>,
    pub item_spawns: Vec<Rect>,
}

fn f(n: Node, k: &str) -> Option<f32> {
    n.attribute(k)?.trim().parse().ok()
}

fn walk(n: Node, parent: Mat, theme: Option<&str>, out: &mut Vec<Placed>) {
    for c in n.children().filter(|c| c.is_element()) {
        match c.tag_name().name() {
            "Platform" => {
                let sx = f(c, "ScaleX").or(f(c, "Scale")).unwrap_or(1.0);
                let sy = f(c, "ScaleY").or(f(c, "Scale")).unwrap_or(1.0);
                let m = parent.then_inner(Mat::trs(
                    f(c, "X").unwrap_or(0.0),
                    f(c, "Y").unwrap_or(0.0),
                    sx,
                    sy,
                    f(c, "Rotation").unwrap_or(0.0),
                ));
                let th = c.attribute("Theme").or(theme);
                // a instância própria do nó é herdada pelos filhos só como contexto de tema
                walk(c, m, th, out);
            }
            "Asset" => {
                let (w, h) = (f(c, "W").unwrap_or(0.0), f(c, "H").unwrap_or(0.0));
                // W negativo = espelhado horizontalmente em torno de X (a imagem se estende para o lado negativo).
                let m = parent.then_inner(Mat::trs(
                    f(c, "X").unwrap_or(0.0),
                    f(c, "Y").unwrap_or(0.0),
                    1.0,
                    1.0,
                    f(c, "Rotation").unwrap_or(0.0),
                ));
                out.push(Placed {
                    asset: c.attribute("AssetName").unwrap_or_default().to_string(),
                    m,
                    w,
                    h,
                    instance: n.attribute("InstanceName").unwrap_or_default().to_string(),
                    theme: theme.map(str::to_string),
                });
            }
            _ => {}
        }
    }
}

fn collision(n: Node) -> Option<Collision> {
    let kind = match n.tag_name().name() {
        "HardCollision" | "GameModeHardCollision" => CollisionKind::Hard,
        "SoftCollision" | "BouncySoftCollision" | "SoftPressurePlateCollision" => CollisionKind::Soft,
        "NoSlideCollision" | "BouncyNoSlideCollision" => CollisionKind::NoSlide,
        "BouncyHardCollision" => CollisionKind::Bouncy,
        "LavaCollision" => CollisionKind::Lava,
        "PressurePlateCollision" | "MudCollision" => CollisionKind::Other,
        _ => return None,
    };
    // Horizontal: X1,X2,Y. Vertical: X,Y1,Y2. Aceita também X1,X2,Y1,Y2.
    let (x1, x2) = (f(n, "X1").or(f(n, "X")), f(n, "X2").or(f(n, "X")));
    let (y1, y2) = (f(n, "Y1").or(f(n, "Y")), f(n, "Y2").or(f(n, "Y")));
    Some(Collision {
        kind,
        x1: x1?,
        y1: y1?,
        x2: x2?,
        y2: y2?,
        team: n.attribute("Team").and_then(|t| t.parse().ok()),
        normal: if n.has_attribute("NormalX") || n.has_attribute("NormalY") { Some((f(n, "NormalX").unwrap_or(0.0), f(n, "NormalY").unwrap_or(0.0))) } else { None },
    })
}

pub fn parse_level(xml: &str) -> Result<Level> {
    let doc = roxmltree::Document::parse(xml).context("XML do mapa inválido")?;
    let root = doc.root_element();
    let mut lv = Level {
        name: root.attribute("LevelName").unwrap_or_default().to_string(),
        asset_dir: root.attribute("AssetDir").unwrap_or_default().to_string(),
        camera: Rect::default(),
        background: None,
        placed: Vec::new(),
        collisions: Vec::new(),
        respawns: Vec::new(),
        item_spawns: Vec::new(),
    };
    walk(root, Mat::ID, None, &mut lv.placed);
    for n in root.children().filter(|c| c.is_element()) {
        match n.tag_name().name() {
            "CameraBounds" => {
                lv.camera = Rect {
                    x: f(n, "X").unwrap_or(0.0),
                    y: f(n, "Y").unwrap_or(0.0),
                    w: f(n, "W").unwrap_or(0.0),
                    h: f(n, "H").unwrap_or(0.0),
                }
            }
            "Background" if lv.background.is_none() => {
                lv.background = n.attribute("AssetName").map(str::to_string)
            }
            "Respawn" => lv.respawns.push((f(n, "X").unwrap_or(0.0), f(n, "Y").unwrap_or(0.0))),
            "ItemSpawn" => lv.item_spawns.push(Rect {
                x: f(n, "X").unwrap_or(0.0),
                y: f(n, "Y").unwrap_or(0.0),
                w: f(n, "W").unwrap_or(0.0),
                h: f(n, "H").unwrap_or(0.0),
            }),
            _ => {
                if let Some(c) = collision(n) {
                    lv.collisions.push(c);
                }
            }
        }
    }
    Ok(lv)
}

/// Lê todos os mapas das seções decifradas do `Dynamic.swz`.
pub fn parse_levels(sections: &[Vec<u8>]) -> Vec<Level> {
    let mut out = Vec::new();
    for s in sections {
        let text = String::from_utf8_lossy(s);
        if text.trim_start().starts_with("<LevelDesc") {
            if let Ok(l) = parse_level(&text) {
                out.push(l);
            }
        }
    }
    out
}

/// Margens de KO além dos limites da câmera (`LeftKill`, `TopKill`, `RightKill`, `BottomKill` de `LevelTypes`,
/// em `Init.swz`): devolve (esquerda, cima, direita, baixo).
pub fn kill_margins(init_sections: &[Vec<u8>], level_name: &str) -> Option<(f32, f32, f32, f32)> {
    let s = init_sections.iter().find(|s| s.starts_with(b"<LevelTypes>"))?;
    let text = String::from_utf8_lossy(s);
    let doc = roxmltree::Document::parse(&text).ok()?;
    let n = doc.root_element().children().find(|n| n.has_tag_name("LevelType") && n.attribute("LevelName") == Some(level_name))?;
    let get = |t: &str| n.children().find(|c| c.has_tag_name(t)).and_then(|c| c.text()).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
    Some((get("LeftKill"), get("TopKill"), get("RightKill"), get("BottomKill")))
}
