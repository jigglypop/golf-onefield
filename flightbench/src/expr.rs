//! 항을 글로 적어 바로 계산하는 작은 수식 해석기 (다시 컴파일하지 않고 항을 바꾸기 위해).
//!
//! 문법: 숫자, 이름, + - * / ^, 단항 -, 괄호, 함수 sqrt exp ln abs tanh atan min(a,b) max(a,b) pow(a,b).
//! 이름은 변수 표(slot)로 미리 풀어 두므로 계산 중에 문자열을 찾지 않는다.

#[derive(Clone, Debug)]
pub enum E {
    Num(f64),
    Var(usize),
    Neg(Box<E>),
    Bin(u8, Box<E>, Box<E>),
    F1(u8, Box<E>),
    F2(u8, Box<E>, Box<E>),
}

/// x^y. 정수 지수(|y| ≤ 64)는 곱으로 접는 `powi` (빠르고 음수 밑에서도 정의됨; `powf` 와 마지막 몇 비트만 다르다),
/// 그 밖은 `powf`. 위기 항의 ^4, ^12 처럼 식마다 수만 번 불리는 곳이라 `powf` 의 지수·로그 계산을 피한다.
#[inline]
pub fn pow(x: f64, y: f64) -> f64 {
    if y == 2.0 {
        x * x
    } else if y == 3.0 {
        x * x * x
    } else if y.fract() == 0.0 && y.abs() <= 64.0 {
        x.powi(y as i32)
    } else {
        x.powf(y)
    }
}

impl E {
    #[inline]
    pub fn eval(&self, v: &[f64]) -> f64 {
        match self {
            E::Num(x) => *x,
            E::Var(i) => v[*i],
            E::Neg(a) => -a.eval(v),
            E::Bin(op, a, b) => {
                let (x, y) = (a.eval(v), b.eval(v));
                match op {
                    b'+' => x + y,
                    b'-' => x - y,
                    b'*' => x * y,
                    b'/' => x / y,
                    _ => pow(x, y),
                }
            }
            E::F1(f, a) => {
                let x = a.eval(v);
                match f {
                    0 => x.sqrt(),
                    1 => x.exp(),
                    2 => x.ln(),
                    3 => x.abs(),
                    4 => x.tanh(),
                    _ => x.atan(),
                }
            }
            E::F2(f, a, b) => {
                let (x, y) = (a.eval(v), b.eval(v));
                match f {
                    0 => x.min(y),
                    1 => x.max(y),
                    _ => pow(x, y),
                }
            }
        }
    }
}

pub struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    names: &'a [String],
}

pub fn parse(src: &str, names: &[String]) -> Result<E, String> {
    let mut p = Parser { s: src.as_bytes(), i: 0, names };
    let e = p.sum()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("'{}' 의 {} 번째 글자에서 남은 입력: {}", src, p.i, &src[p.i..]));
    }
    Ok(e)
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn sum(&mut self) -> Result<E, String> {
        let mut a = self.prod()?;
        while let Some(c @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            a = E::Bin(c, Box::new(a), Box::new(self.prod()?));
        }
        Ok(a)
    }
    fn prod(&mut self) -> Result<E, String> {
        let mut a = self.unary()?;
        while let Some(c @ (b'*' | b'/')) = self.peek() {
            self.i += 1;
            a = E::Bin(c, Box::new(a), Box::new(self.unary()?));
        }
        Ok(a)
    }
    fn unary(&mut self) -> Result<E, String> {
        if self.peek() == Some(b'-') {
            self.i += 1;
            return Ok(E::Neg(Box::new(self.unary()?)));
        }
        self.power()
    }
    fn power(&mut self) -> Result<E, String> {
        let a = self.atom()?;
        if self.peek() == Some(b'^') {
            self.i += 1;
            return Ok(E::Bin(b'^', Box::new(a), Box::new(self.unary()?)));
        }
        Ok(a)
    }
    fn atom(&mut self) -> Result<E, String> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let e = self.sum()?;
                self.expect(b')')?;
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || matches!(self.s[self.i], b'.' | b'e' | b'E')
                    || (matches!(self.s[self.i], b'+' | b'-') && matches!(self.s[self.i - 1], b'e' | b'E')))
                {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.s[st..self.i]).unwrap();
                t.parse().map(E::Num).map_err(|_| format!("숫자 아님: {t}"))
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_') {
                    self.i += 1;
                }
                let id = std::str::from_utf8(&self.s[st..self.i]).unwrap().to_string();
                if self.peek() == Some(b'(') {
                    self.i += 1;
                    let a = self.sum()?;
                    let f1 = ["sqrt", "exp", "ln", "abs", "tanh", "atan"].iter().position(|f| *f == id);
                    if let Some(k) = f1 {
                        self.expect(b')')?;
                        return Ok(E::F1(k as u8, Box::new(a)));
                    }
                    let f2 = ["min", "max", "pow"].iter().position(|f| *f == id).ok_or(format!("모르는 함수: {id}"))?;
                    self.expect(b',')?;
                    let b = self.sum()?;
                    self.expect(b')')?;
                    return Ok(E::F2(f2 as u8, Box::new(a), Box::new(b)));
                }
                if id == "pi" {
                    return Ok(E::Num(std::f64::consts::PI));
                }
                self.names.iter().position(|n| *n == id).map(E::Var).ok_or(format!("모르는 이름: {id} (쓸 수 있는 이름: {:?})", self.names))
            }
            c => Err(format!("{} 번째에서 예상 밖: {:?}", self.i, c.map(|c| c as char))),
        }
    }
    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("{} 번째에 '{}' 가 필요", self.i, c as char))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic() {
        let n: Vec<String> = ["u", "w"].iter().map(|s| s.to_string()).collect();
        let e = parse("2*u + w^2 - sqrt(4) + min(u, w) + 1e-1", &n).unwrap();
        assert!((e.eval(&[3.0, 2.0]) - (6.0 + 4.0 - 2.0 + 2.0 + 0.1)).abs() < 1e-12);
        let e = parse("-u^2", &n).unwrap();
        assert_eq!(e.eval(&[3.0, 0.0]), -9.0);
    }

    /// 정수 지수의 powi 길은 powf 와 마지막 몇 비트만 다르다 (상대 4e-15 이내), 음수 밑·음수 지수·0 도 같은 값
    #[test]
    fn integer_pow_matches_powf() {
        for &y in &[4.0f64, 5.0, 6.0, 12.0, 16.0, -2.0, -4.0, 1.0, 0.0, 64.0] {
            for i in 0..400 {
                let x = 0.05 + 0.025 * i as f64;
                let (a, b) = (pow(x, y), x.powf(y));
                assert!(((a - b) / b).abs() < 4e-15, "{x}^{y}: {a} 대 {b}");
                let (a, b) = (pow(-x, y), (-x).powf(y)); // 음수 밑
                assert!(((a - b) / b).abs() < 4e-15, "(-{x})^{y}: {a} 대 {b}");
            }
        }
        assert_eq!(pow(0.0, 12.0), 0.0);
        assert!(pow(0.0, -2.0).is_infinite());
        assert_eq!(pow(2.0, 0.5), 2f64.powf(0.5)); // 정수가 아니면 powf 그대로
        // 식 해석기에서도 같은 길
        let n: Vec<String> = ["Re"].iter().map(|s| s.to_string()).collect();
        let e = parse("(Re/76621.4458)^12", &n).unwrap();
        let x: f64 = 60000.0 / 76621.4458;
        assert!(((e.eval(&[60000.0]) - x.powf(12.0)) / x.powf(12.0)).abs() < 4e-15);
    }
}
