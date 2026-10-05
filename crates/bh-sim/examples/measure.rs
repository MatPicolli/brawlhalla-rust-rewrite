//! Medidas do movimento de Bödvar nas unidades reais do motor: `cargo run -p bh-sim --example measure`.
use std::fs;

use bh_assets::{install, stats, swz};
use bh_sim::{Fighter, Geometry, Input, Kind, Params, Tuning};

fn main() {
    let dir = install::locate(None).unwrap();
    let key = swz::find_key(&fs::read(dir.join("BrawlhallaAir.swf")).unwrap(), &fs::read(dir.join("Init.swz")).unwrap()).unwrap();
    let game = swz::decrypt(&fs::read(dir.join("Game.swz")).unwrap(), key).unwrap();
    let p = Params::new(&stats::hero_speed_stat(&game, "Viking").unwrap(), Tuning::default());
    let g = Geometry::new(&[((-1e6, 0.0), (1e6, 0.0), Kind::Hard, Some((0.0, -1.0)))]);
    let px_s = |v: f32| v * p.t.dt * 60.0;

    // corrida
    let mut f = Fighter::new(0.0, 0.0, &p);
    f.step(&Input::default(), &p, &g);
    let mut ticks = 0;
    while f.vx < p.run_speed - 0.01 && ticks < 200 {
        f.step(&Input { move_x: 1.0, ..Default::default() }, &p, &g);
        ticks += 1;
    }
    println!("corrida: velocidade máx {:.2} un = {:.0} px/s, atinge em {ticks} ticks ({:.2} s)", p.run_speed, px_s(p.run_speed), ticks as f32 / 60.0);
    let mut stop = 0;
    while f.vx > 0.01 {
        f.step(&Input::default(), &p, &g);
        stop += 1;
    }
    println!("para em {stop} ticks ({:.2} s) soltando a direção", stop as f32 / 60.0);

    // pulo
    let mut f = Fighter::new(0.0, 0.0, &p);
    f.step(&Input::default(), &p, &g);
    f.step(&Input { jump: true, ..Default::default() }, &p, &g);
    let (mut top, mut t_top) = (0.0f32, 0);
    for t in 0..400 {
        f.step(&Input::default(), &p, &g);
        if -f.y > top {
            top = -f.y;
            t_top = t;
        }
        if f.grounded() {
            println!("pulo: altura {top:.0} px no tick {t_top}, aterrissa após {t} ticks ({:.2} s)", t as f32 / 60.0);
            break;
        }
    }
    // pulo duplo
    let mut f = Fighter::new(0.0, 0.0, &p);
    f.step(&Input::default(), &p, &g);
    f.step(&Input { jump: true, ..Default::default() }, &p, &g);
    for _ in 0..24 {
        f.step(&Input::default(), &p, &g);
    }
    f.step(&Input { jump: true, ..Default::default() }, &p, &g);
    let mut top = 0.0f32;
    for _ in 0..400 {
        f.step(&Input::default(), &p, &g);
        top = top.max(-f.y);
        if f.grounded() {
            break;
        }
    }
    println!("pulo + pulo no ar perto do ápice: altura máxima {top:.0} px");
    println!("queda: terminal {:.0} px/s (70 un), queda rápida {:.0} px/s (85 un)", px_s(70.0), px_s(85.0));
}
