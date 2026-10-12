//! 상자 제약 레벤버그-마쿼트 하나.
//!
//! 이전에는 harness(`lm`), engine(`fit_h2`), calib, screen, spininv 에 따로 있었다. 갈린 것은 설정뿐이다:
//! 야코비안 걸음(상대 / 고정), 감쇠 꼴, 반복 수, 멈춤 조건. 그 값들을 `LmCfg` 로 받아 산술은 그대로 두었다
//! (각 복사본의 숫자와 같아야 한다 — 이전 판과 출력이 같은지는 시험대 수치로 대조했다).
//!
//! 풀이 순서: 바깥 반복마다 야코비안(전진 차분) 한 번 → 정규방정식 $J^T J,\ J^T r$ 한 번 → 안쪽에서 $\mu$ 를 올려 가며
//! $(J^T J + \text{감쇠})\,\delta = -J^T r$ 를 풀고 상자로 자른 걸음이 비용을 줄이면 받아들인다.

use crate::linalg::solve;

/// 야코비안 걸음 $h_k$
pub enum Step<'a> {
    /// $h_k = \text{rel}\cdot\max(|p_k|, \text{floor})$ (매개변수 크기에 비례)
    Rel { rel: f64, floor: f64 },
    /// 매개변수마다 고정한 걸음
    Fixed(&'a [f64]),
}

/// 대각 감쇠
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Damping {
    /// $A_{xx} \leftarrow A_{xx}(1+\mu) + 10^{-12}$
    Mul,
    /// $A_{xx} \leftarrow A_{xx} + \mu\max(A_{xx}, 10^{-12})$
    Diag,
}

pub struct LmCfg<'a> {
    pub lo: &'a [f64],
    pub hi: &'a [f64],
    pub step: Step<'a>,
    pub damping: Damping,
    /// 바깥 반복 최대
    pub max_iter: usize,
    /// 안쪽(μ 올리기) 시도 최대
    pub tries: usize,
    /// 받아들일 때 μ ← max(0.3 μ, mu_min)
    pub mu_min: f64,
    /// 받아들인 걸음의 상대 개선 $(c_{old}-c_{new})/c_{old}$ 가 이 값 이하면 그 걸음을 받고 끝낸다 (음수면 안 씀)
    pub stop_rel: f64,
    /// Some(v): 비용에서 유한하지 않은 잔차 하나를 제곱 대신 v 로 센다 (calib). None: 그냥 제곱합.
    pub nonfinite_cost: Option<f64>,
}

pub struct LmOut {
    pub p: Vec<f64>,
    pub cost: f64,
    /// 잔차 함수 호출 수
    pub evals: usize,
    /// 마지막 바깥 반복의 $J^T J$ (공분산 어림에 쓴다)
    pub jtj: Vec<Vec<f64>>,
}

/// 야코비안 열 공급자: (p, r, h) → 열 k 의 $(r(p + h_k e_k) - r(p))_i / h_k$, `cols[k][i]`
pub type JacFn<'a> = &'a mut dyn FnMut(&[f64], &[f64], &[f64]) -> Vec<Vec<f64>>;

/// 레벤버그-마쿼트. `res` 는 매개변수 → 잔차. `jac` 가 None 이면 열을 `res` 로 하나씩 구한다(직렬).
pub fn lm(cfg: &LmCfg, p0: &[f64], res: &mut dyn FnMut(&[f64]) -> Vec<f64>, mut jac: Option<JacFn>) -> LmOut {
    let n = p0.len();
    let cost = |r: &[f64]| match cfg.nonfinite_cost {
        None => r.iter().map(|x| x * x).sum::<f64>(),
        Some(big) => r.iter().map(|x| if x.is_finite() { x * x } else { big }).sum::<f64>(),
    };
    let mut p = p0.to_vec();
    let mut r = res(&p);
    let mut evals = 1usize;
    let mut mu = 1e-3;
    let mut jtj = vec![vec![0.0; n]; n];
    for _ in 0..cfg.max_iter {
        let h: Vec<f64> = (0..n)
            .map(|k| match &cfg.step {
                Step::Rel { rel, floor } => rel * p[k].abs().max(*floor),
                Step::Fixed(s) => s[k],
            })
            .collect();
        let cols: Vec<Vec<f64>> = match jac.as_mut() {
            Some(f) => f(&p, &r, &h),
            None => (0..n)
                .map(|k| {
                    let mut q = p.clone();
                    q[k] += h[k];
                    let rq = res(&q);
                    (0..r.len()).map(|i| (rq[i] - r[i]) / h[k]).collect()
                })
                .collect(),
        };
        evals += n;
        jtj = vec![vec![0.0; n]; n];
        let mut jtr = vec![0.0; n];
        for i in 0..r.len() {
            for a in 0..n {
                jtr[a] += cols[a][i] * r[i];
                for b in 0..n {
                    jtj[a][b] += cols[a][i] * cols[b][i];
                }
            }
        }
        let mut improved = false;
        for _ in 0..cfg.tries {
            let a: Vec<Vec<f64>> = (0..n)
                .map(|x| {
                    let mut row = jtj[x].clone();
                    row[x] = match cfg.damping {
                        Damping::Mul => row[x] * (1.0 + mu) + 1e-12,
                        Damping::Diag => row[x] + mu * row[x].max(1e-12),
                    };
                    row.push(-jtr[x]);
                    row
                })
                .collect();
            let dp = solve(a);
            let q: Vec<f64> = (0..n).map(|k| (p[k] + dp[k]).clamp(cfg.lo[k], cfg.hi[k])).collect();
            let rq = res(&q);
            evals += 1;
            if cost(&rq) < cost(&r) {
                let rel = (cost(&r) - cost(&rq)) / cost(&r).max(1e-30);
                p = q;
                r = rq;
                mu = (mu * 0.3).max(cfg.mu_min);
                if rel <= cfg.stop_rel {
                    return LmOut { p, cost: cost(&r), evals, jtj };
                }
                improved = true;
                break;
            }
            mu *= 10.0;
        }
        if !improved {
            break;
        }
    }
    LmOut { p, cost: cost(&r), evals, jtj }
}
