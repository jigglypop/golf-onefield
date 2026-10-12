//! 웹 엔진(web3d/src/engine) 검산용 참값: 기본 장(default_field)을 아주 고운 걸음 RK4 로 3차원(스핀축 포함) 적분한다.
//! 장을 바꾸면 다시 뽑는다:  cargo run --profile quick --bin webrefs > ../web3d/test/web_refs.csv
//!
//! 열: mph, 발사각 deg, 스핀 rpm, 스핀축 deg(엔진 부호: + 이면 -z 로 휨), 캐리 yd, 좌우 yd, 최고 높이 yd, 착지각 deg, 체공 s
//! 샷: PGA 12행 × 스핀축 (0, 10, -25), LPGA 11행, 아마추어·극단 5개

use flightbench::engine::{default_field, dimless, Aero, U_MS, YD};
use flightbench::harness::{LPGA11, PGA12};

fn main() {
    let ae = default_field();
    let tu = U_MS / 9.81;
    let d = |s: &[f64; 7], oy: f64, oz: f64| -> [f64; 7] {
        let (ux, uy, uz, w) = (s[3], s[4], s[5], s[6]);
        let sp = (ux * ux + uy * uy + uz * uz).sqrt();
        let (kd, kl) = ae.kk(sp, w);
        let (cx, cy, cz) = (oy * uz - oz * uy, oz * ux, -oy * ux);
        [ux, uy, uz, -kd * ux + kl * cx, -kd * uy + kl * cy - 1.0, -kd * uz + kl * cz, -ae.beta() * w * sp]
    };
    let mut shots: Vec<(f64, f64, f64, f64)> = Vec::new();
    for r in PGA12.iter() {
        for a in [0.0, 10.0, -25.0] {
            shots.push((r[0], r[1], r[2], a));
        }
    }
    for r in LPGA11.iter() {
        shots.push((r[0], r[1], r[2], 0.0));
    }
    for &(m, g, s) in &[(135.0, 13.0, 3000.0), (100.0, 18.0, 6000.0), (60.0, 30.0, 6000.0), (190.0, 9.0, 2000.0), (40.0, 45.0, 3000.0)] {
        shots.push((m, g, s, 0.0));
    }
    println!("mph,deg,rpm,axis_deg,carry_yd,lateral_yd,apex_yd,land_deg,time_s");
    let dt = 2e-5;
    for (mph, deg, rpm, ax) in shots {
        let [v0, th, s0] = dimless(mph, deg, rpm);
        let a = f64::to_radians(ax);
        let (oy, oz) = (a.sin(), a.cos());
        let mut s = [0.0, 0.0, 0.0, v0 * th.cos(), v0 * th.sin(), 0.0, s0 * v0];
        let (mut t, mut apex) = (0.0, 0.0f64);
        loop {
            let k1 = d(&s, oy, oz);
            let p: [f64; 7] = std::array::from_fn(|i| s[i] + 0.5 * dt * k1[i]);
            let k2 = d(&p, oy, oz);
            let p: [f64; 7] = std::array::from_fn(|i| s[i] + 0.5 * dt * k2[i]);
            let k3 = d(&p, oy, oz);
            let p: [f64; 7] = std::array::from_fn(|i| s[i] + dt * k3[i]);
            let k4 = d(&p, oy, oz);
            let n: [f64; 7] = std::array::from_fn(|i| s[i] + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
            if n[1] < 0.0 && t > 0.0 {
                // 걸음이 아주 작아 선형 보간 오차는 무시할 만하다
                let f = s[1] / (s[1] - n[1]);
                let lerp = |i: usize| s[i] + f * (n[i] - s[i]);
                let land = (-lerp(4)).atan2((lerp(3).powi(2) + lerp(5).powi(2)).sqrt()).to_degrees();
                println!("{mph},{deg},{rpm},{ax},{:.6},{:.6},{:.6},{:.6},{:.6}", lerp(0) * YD, lerp(2) * YD, apex * YD, land, (t + f * dt) * tu);
                break;
            }
            apex = apex.max(n[1]);
            s = n;
            t += dt;
        }
    }
}
