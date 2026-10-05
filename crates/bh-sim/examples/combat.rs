//! Bödvar (martelo) contra Bödvar com os dados reais do jogo, sem gráficos:
//! `cargo run -p bh-sim --example combat`.
use std::fs;

use bh_assets::{dodge, install, powers::PowerTable, stats, swz};
use bh_sim::{
    combat::CombatStats,
    world::{Actor, ActorInput, World},
    Geometry, Input, Kind, Params, Tuning,
};

fn main() {
    let dir = install::locate(None).unwrap();
    let key = swz::find_key(&fs::read(dir.join("BrawlhallaAir.swf")).unwrap(), &fs::read(dir.join("Init.swz")).unwrap()).unwrap();
    let game = swz::decrypt(&fs::read(dir.join("Game.swz")).unwrap(), key).unwrap();
    let levels = stats::hero_levels(&game, "Viking").unwrap();
    let combat = CombatStats { strength: levels.strength, defense: levels.weight, impulse_mult: stats::strength_impulse_mult(&game, levels.strength).unwrap() };
    let mods = dodge::time_mods(&game, levels.dexterity).unwrap();
    let kit = stats::hero_weapon_kit(&game, "Viking", 1).unwrap();
    let params = Params::new(&stats::hero_speed_stat(&game, "Viking").unwrap(), Tuning::default());
    println!("Bödvar: {levels:?} {combat:?}\n{mods:?}\n{kit:?}");

    let none = ActorInput::default();
    let right = Input { move_x: 1.0, ..Default::default() };
    // (rótulo, entrada do 1º tick, ticks segurando o golpe pesado)
    let cases: [(&str, ActorInput, u32); 5] = [
        ("leve neutro", ActorInput { attack: true, ..none }, 0),
        ("leve lateral", ActorInput { attack: true, input: right, ..none }, 0),
        ("leve para baixo", ActorInput { attack: true, input: Input { down: true, ..Default::default() }, ..none }, 0),
        ("pesado neutro (carga mínima)", ActorInput { heavy: true, heavy_held: true, ..none }, 0),
        ("pesado neutro (carga máxima)", ActorInput { heavy: true, heavy_held: true, ..none }, 200),
    ];
    for (label, first, hold) in cases {
        println!("\n== {label}");
        for pre in [0.0f32, 100.0, 200.0] {
            let geo = Geometry::new(&[((-9000.0, 0.0), (9000.0, 0.0), Kind::Hard, Some((0.0, -1.0)))]);
            let mut w = World::new(geo, PowerTable::load(&game), (-9000.0, -9000.0, 9000.0, 9000.0));
            w.dodges = dodge::dodge_types(&game).unwrap();
            w.actors.push(Actor::with_kit((0.0, -1.0), params, combat, mods, kit.clone()));
            w.actors.push(Actor::with_kit((150.0, -1.0), params, combat, mods, kit.clone()));
            for _ in 0..3 {
                w.step(&[none; 2]);
            }
            w.actors[1].damage = pre;
            let x0 = w.actors[1].fighter.x;
            w.step(&[first, none]);
            let (mut hits, mut total, mut last_kb, mut max_dx, mut powers) = (0, 0.0f32, 0.0f32, 0.0f32, Vec::<String>::new());
            for t in 1..240 {
                let p1 = if t <= hold { ActorInput { heavy_held: true, ..none } } else { none };
                if let Some(a) = &w.actors[0].attack {
                    if powers.last() != Some(&a.power) {
                        powers.push(a.power.clone());
                    }
                }
                for h in w.step(&[p1, none]).hits {
                    hits += 1;
                    total += h.2;
                    last_kb = h.3;
                }
                max_dx = max_dx.max((w.actors[1].fighter.x - x0).abs());
            }
            println!(
                "  alvo com {pre:3.0}%: {hits} acerto(s), dano {total:5.2}, último knockback {last_kb:6.2} un ({:4.0} px/s), alvo voa {max_dx:5.0} px",
                last_kb * 0.384 * 60.0
            );
            if pre == 0.0 {
                println!("     cadeia: {}", powers.join(" -> "));
            }
        }
    }
}
