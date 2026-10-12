//! 작은 선형 풀이 하나. 가우스-조던 소거(부분 피벗)로 확대 행렬을 푼다.
//!
//! 이전에는 같은 알고리즘이 다섯 곳(harness, screen, spininv, calib, engine)에 따로 있었다. 꼴만 다르고 산술은 같아서
//! (피벗은 `max_by` + `total_cmp`, 소거는 `a[r][k] -= f * a[c][k]`) 하나로 모아도 결과 비트가 같다.

/// 확대 행렬 a (n행; 각 행은 n+1칸 이상, 앞 n칸 계수 + a[i][n] 오른쪽 변)를 제자리에서 소거한다.
/// 행의 꼴은 `Vec<f64>` 든 `[f64; W]` 든 상관없다.
fn eliminate<R: AsRef<[f64]> + AsMut<[f64]>>(a: &mut [R]) {
    let n = a.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i].as_ref()[c].abs().total_cmp(&a[j].as_ref()[c].abs())).unwrap();
        a.swap(c, p);
        for r in 0..n {
            if r != c {
                let f = a[r].as_ref()[c] / a[c].as_ref()[c];
                for k in c..=n {
                    let v = f * a[c].as_ref()[k];
                    a[r].as_mut()[k] -= v;
                }
            }
        }
    }
}

/// 행이 `Vec<f64>` 인 확대 행렬을 풀어 해 x (길이 n) 를 돌려준다.
pub fn solve(mut a: Vec<Vec<f64>>) -> Vec<f64> {
    eliminate(&mut a);
    let n = a.len();
    (0..n).map(|i| a[i][n] / a[i][i]).collect()
}

/// 크기가 상수인 확대 행렬 [[f64; W]; N] (W ≥ N + 1; 오른쪽 변은 a[i][N], 나머지 칸은 안 쓴다)
pub fn solve_fixed<const N: usize, const W: usize>(mut a: [[f64; W]; N]) -> [f64; N] {
    const { assert!(W > N, "확대 행렬 폭 W 는 N + 1 이상") };
    eliminate(&mut a);
    std::array::from_fn(|i| a[i][N] / a[i][i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_and_agrees_across_shapes() {
        // x = [1, -2, 3] 인 3×3 계. 세 꼴이 같은 비트의 해를 낸다.
        let m = [[2.0, 1.0, -1.0], [-3.0, -1.0, 2.0], [-2.0, 1.0, 2.0]];
        let x = [1.0, -2.0, 3.0];
        let rhs: Vec<f64> = m.iter().map(|r| r.iter().zip(&x).map(|(a, b)| a * b).sum()).collect();
        let v = solve((0..3).map(|i| { let mut r = m[i].to_vec(); r.push(rhs[i]); r }).collect());
        let mut fixed = [[0.0; 5]; 3]; // 폭 5 > 4: 남는 칸은 무시된다
        for i in 0..3 {
            fixed[i][..3].copy_from_slice(&m[i]);
            fixed[i][3] = rhs[i];
        }
        let f = solve_fixed::<3, 5>(fixed);
        for i in 0..3 {
            assert!((v[i] - x[i]).abs() < 1e-12, "{v:?}");
            assert_eq!(v[i].to_bits(), f[i].to_bits());
        }
    }
}
