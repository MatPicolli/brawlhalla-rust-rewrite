//! Trajetória do lutador com a entrada do `--demo`, sem gráficos: `cargo run -p bh-sim --example trace`.
use std::fs;

use bh_assets::{install, level, stats, swz};
use bh_sim::{Fighter, Geometry, Input, Params, Tuning};

fn main() {
    let dir = install::locate(None).unwrap();
    let key = swz::find_key(&fs::read(dir.join("BrawlhallaAir.swf")).unwrap(), &fs::read(dir.join("Init.swz")).unwrap()).unwrap();
    let dynamic = swz::decrypt(&fs::read(dir.join("Dynamic.swz")).unwrap(), key).unwrap();
    let game = swz::decrypt(&fs::read(dir.join("Game.swz")).unwrap(), key).unwrap();
    let lv = level::parse_levels(&dynamic).into_iter().find(|l| l.name == "Brawlhaven").unwrap();
    let geo = Geometry::from_level(&lv);
    let p = Params::new(&stats::hero_speed_stat(&game, "Viking").unwrap(), Tuning::default());
    let spawn = lv.respawns[0];
    let mut f = Fighter::new(spawn.0, spawn.1, &p);
    for tick in 0..400u32 {
        let input = match tick {
            0..=150 => Input::default(),
            151..=240 => Input { move_x: 1.0, ..Default::default() },
            241 => Input { move_x: 1.0, jump: true, ..Default::default() },
            242..=275 => Input { move_x: 1.0, ..Default::default() },
            276 => Input { move_x: 1.0, jump: true, ..Default::default() },
            _ => Input { move_x: 1.0, ..Default::default() },
        };
        f.step(&input, &p, &geo);
        if tick % 10 == 0 || (240..250).contains(&tick) {
            println!("t={tick:3} x={:8.1} y={:8.1} vx={:7.1} vy={:7.1} ground={}", f.x, f.y, f.vx, f.vy, f.grounded());
        }
    }
}
