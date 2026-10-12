//! 기본 장(공유위기12)의 계수가 같은 숫자로 흩어져 있는 네 곳이 서로 어긋나지 않는지 본다.
//!
//!   flightbench/src/engine.rs      default_field()            (기준)
//!   golf-core/src/lib.rs           Coef::SHARED_RE
//!   web3d/src/engine/model.ts      SHARED_RE_PARAMS
//!   golf-core/bench/bench.mjs      const COEF
//!
//! 읽기 방법(조금 거칠다): 각 파일에서 기준 블록(첫 `{` 가 열린 줄부터 중괄호가 다시 닫힐 때까지)을 잘라
//! `{ } , ;` 를 공백으로 바꾸고 공백으로 쪼갠 뒤, `v: e: d: c: l:` 와 `u0:`(Rust·JS) / `r0:`(TS) 바로 뒤의 숫자 토큰을 읽는다.
//! u0 는 `74003.0320 / RE_PER_U` 꼴이라 앞의 숫자(= 임계 레이놀즈 수 Re0)만 읽는다. 파일 모양이 바뀌어 읽지 못하면
//! 조용히 넘어가지 않고 실패한다(동기화를 확인할 수 없다는 뜻이다).

use std::path::Path;

const KEYS: [&str; 6] = ["v", "e", "re0", "d", "c", "l"];

fn read(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e} (계수 동기화를 확인할 수 없다)", p.display()))
}

/// anchor 를 포함한 첫 줄부터 중괄호 깊이가 0 으로 돌아오는 줄까지
fn block_from(text: &str, anchor: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    let mut started = false;
    for line in text.lines() {
        if !started && !line.contains(anchor) {
            continue;
        }
        started = true;
        out.push_str(line);
        out.push('\n');
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        if depth <= 0 && out.contains('{') {
            return out;
        }
    }
    panic!("기준 블록을 찾지 못했다: '{anchor}'");
}

/// 블록에서 [v, e, re0, d, c, l] 를 읽는다
fn coefs(block: &str, where_: &str) -> [f64; 6] {
    // 줄 끝 주석(// ...)은 버린다
    let cleaned: String = block.lines().map(|l| l.split("//").next().unwrap()).collect::<Vec<_>>().join(" ");
    let spaced = cleaned.replace(['{', '}', ',', ';'], " ");
    let toks: Vec<&str> = spaced.split_whitespace().collect();
    let mut out = [f64::NAN; 6];
    for w in toks.windows(2) {
        let key = match w[0] {
            "v:" => "v",
            "e:" => "e",
            "u0:" | "r0:" => "re0",
            "d:" => "d",
            "c:" => "c",
            "l:" => "l",
            _ => continue,
        };
        let i = KEYS.iter().position(|k| *k == key).unwrap();
        if out[i].is_nan() {
            out[i] = w[1].parse().unwrap_or_else(|_| panic!("{where_}: '{}' 뒤의 '{}' 는 숫자가 아니다", w[0], w[1]));
        }
    }
    for (k, v) in KEYS.iter().zip(&out) {
        assert!(!v.is_nan(), "{where_}: 계수 '{k}' 를 읽지 못했다");
    }
    out
}

fn show(c: &[f64; 6]) -> String {
    KEYS.iter().zip(c).map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
}

#[test]
fn default_field_coefficients_are_in_sync() {
    // 기준 1: 엔진 소스의 default_field() 글자
    let engine_src = coefs(&block_from(&read("src/engine.rs"), "pub fn default_field()"), "engine.rs");
    // 기준 2: 실제로 컴파일된 값. 글자 읽기가 맞는지도 이것으로 확인한다.
    let f = flightbench::engine::default_field();
    let engine_num = [f.v, f.e, f.u0 * flightbench::harness::RE_PER_U, f.d, f.c, f.l];
    for i in 0..6 {
        let tol = if KEYS[i] == "re0" { 1e-9 } else { 0.0 };
        assert!(
            (engine_src[i] - engine_num[i]).abs() <= tol * engine_num[i].abs(),
            "engine.rs 글자와 default_field() 값이 다르다 ({}): {} 대 {}",
            KEYS[i],
            engine_src[i],
            engine_num[i]
        );
    }

    let others = [
        ("golf-core/src/lib.rs (Coef::SHARED_RE)", "../golf-core/src/lib.rs", "pub const SHARED_RE: Coef"),
        ("web3d/src/engine/model.ts (SHARED_RE_PARAMS)", "../web3d/src/engine/model.ts", "export const SHARED_RE_PARAMS"),
        ("golf-core/bench/bench.mjs (COEF)", "../golf-core/bench/bench.mjs", "const COEF"),
    ];
    let mut bad = Vec::new();
    for (name, rel, anchor) in others {
        let c = coefs(&block_from(&read(rel), anchor), name);
        // 같은 글자는 같은 f64 가 된다. 어긋나면 숫자가 다른 것이다 (Re0 만 1e-12 상대 여유).
        let same = (0..6).all(|i| c[i] == engine_src[i] || (KEYS[i] == "re0" && (c[i] - engine_src[i]).abs() <= 1e-12 * engine_src[i]));
        if !same {
            bad.push(format!("{name}\n    {}\n  engine.rs default_field()\n    {}", show(&c), show(&engine_src)));
        }
    }
    assert!(bad.is_empty(), "기본 장 계수가 엔진과 어긋났다 — 같은 숫자로 맞춰야 한다:\n  {}", bad.join("\n  "));
}

/// 읽기 도구가 어긋남을 실제로 잡는지: 숫자 하나를 바꾼 글자를 넣으면 다르게 읽힌다.
#[test]
fn parser_sees_a_changed_digit() {
    let a = coefs("Coef {\n v: 0.1383,\n e: 0.2540,\n u0: 74003.0320 / RE_PER_U,\n d: 1.4838,\n c: 2.8062,\n l: 6.5014, // 메모\n};", "t");
    let b = coefs("Coef {\n v: 0.1383,\n e: 0.2540,\n u0: 74003.0320 / RE_PER_U,\n d: 1.4838,\n c: 2.8062,\n l: 6.5015,\n};", "t");
    assert_ne!(a, b);
    assert_eq!(a, [0.1383, 0.2540, 74003.0320, 1.4838, 2.8062, 6.5014]);
    let ts = coefs("export const P = {\n  v: 0.1383,\n  e: 0.254,\n  r0: 74003.032,\n  n: 12,\n  d: 1.4838,\n  c: 2.8062,\n  l: 6.5014,\n  beta: 0.06,\n} as const;", "t");
    assert_eq!(ts, a);
}
