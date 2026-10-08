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
                    _ => {
                        // 정수 지수는 곱으로 (빠르고 음수 밑에서도 정의됨)
                        if y == 2.0 {
                            x * x
                        } else if y == 3.0 {
                            x * x * x
                        } else {
                            x.powf(y)
                        }
                    }
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
                    _ => x.powf(y),
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
}
