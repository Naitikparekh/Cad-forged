//! Values of the AutoLISP interpreter.
use std::cell::RefCell;
use std::rc::Rc;

/// A user function (`defun` or `lambda`). AutoLISP binds parameters and locals dynamically.
#[derive(Debug)]
pub struct Lambda {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub locals: Vec<String>,
    pub body: Vec<Value>,
}

#[derive(Clone, Debug)]
pub enum Value {
    Nil,
    T,
    Int(i64),
    Real(f64),
    Str(Rc<str>),
    /// Upper-cased symbol name.
    Sym(Rc<str>),
    Cons(Rc<(Value, Value)>),
    /// An entity name (the id of an entity of the active drawing).
    Ename(u64),
    /// A selection set (entity ids).
    Sel(Rc<RefCell<Vec<u64>>>),
    Fun(Rc<Lambda>),
}

/// Why an evaluation stopped.
#[derive(Debug)]
pub enum LErr {
    /// `error: ...`
    Msg(String),
    /// The user cancelled (Escape) while a command or `get...` function was waiting.
    Cancel,
    /// `(exit)` / `(quit)`
    Exit,
}

impl From<String> for LErr {
    fn from(s: String) -> LErr {
        LErr::Msg(s)
    }
}

impl From<&str> for LErr {
    fn from(s: &str) -> LErr {
        LErr::Msg(s.to_string())
    }
}

pub type LResult<T = Value> = Result<T, LErr>;

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }

    pub fn sym(s: &str) -> Value {
        Value::Sym(Rc::from(s.to_uppercase().as_str()))
    }

    pub fn cons(a: Value, b: Value) -> Value {
        Value::Cons(Rc::new((a, b)))
    }

    pub fn bool(b: bool) -> Value {
        if b {
            Value::T
        } else {
            Value::Nil
        }
    }

    /// A proper list.
    pub fn list(items: Vec<Value>) -> Value {
        items.into_iter().rev().fold(Value::Nil, |tail, x| Value::cons(x, tail))
    }

    /// A point as `(x y 0.0)`.
    pub fn point(x: f64, y: f64) -> Value {
        Value::list(vec![Value::Real(x), Value::Real(y), Value::Real(0.0)])
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }

    pub fn is_true(&self) -> bool {
        !self.is_nil()
    }

    pub fn is_atom(&self) -> bool {
        !matches!(self, Value::Cons(_))
    }

    /// The elements of a proper list (`nil` is the empty list); `None` for anything else or a dotted list.
    pub fn to_vec(&self) -> Option<Vec<Value>> {
        let mut out = vec![];
        let mut cur = self.clone();
        loop {
            match cur {
                Value::Nil => return Some(out),
                Value::Cons(c) => {
                    out.push(c.0.clone());
                    cur = c.1.clone();
                }
                _ => return None,
            }
        }
    }

    pub fn car(&self) -> Option<Value> {
        match self {
            Value::Cons(c) => Some(c.0.clone()),
            Value::Nil => Some(Value::Nil),
            _ => None,
        }
    }

    pub fn cdr(&self) -> Option<Value> {
        match self {
            Value::Cons(c) => Some(c.1.clone()),
            Value::Nil => Some(Value::Nil),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Real(r) => Some(*r),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Real(r) => Some(*r as i64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// `(x y)` or `(x y z)` of numbers as a 2D point.
    pub fn as_point(&self) -> Option<(f64, f64)> {
        let v = self.to_vec()?;
        if v.len() < 2 || v.len() > 3 {
            return None;
        }
        Some((v[0].as_f64()?, v[1].as_f64()?))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::T => "T",
            Value::Int(_) => "INT",
            Value::Real(_) => "REAL",
            Value::Str(_) => "STR",
            Value::Sym(_) => "SYM",
            Value::Cons(_) => "LIST",
            Value::Ename(_) => "ENAME",
            Value::Sel(_) => "PICKSET",
            Value::Fun(_) => "USUBR",
        }
    }

    /// `equal`: structural equality (numbers compare by value, int and real alike).
    pub fn equal(&self, o: &Value) -> bool {
        match (self, o) {
            (Value::Nil, Value::Nil) | (Value::T, Value::T) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Int(_) | Value::Real(_), Value::Int(_) | Value::Real(_)) => self.as_f64() == o.as_f64(),
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Sym(a), Value::Sym(b)) => a == b,
            (Value::Ename(a), Value::Ename(b)) => a == b,
            (Value::Sel(a), Value::Sel(b)) => Rc::ptr_eq(a, b),
            (Value::Fun(a), Value::Fun(b)) => Rc::ptr_eq(a, b),
            (Value::Cons(a), Value::Cons(b)) => Rc::ptr_eq(a, b) || (a.0.equal(&b.0) && a.1.equal(&b.1)),
            _ => false,
        }
    }

    /// `eq`: the same object (atoms by value; lists by identity).
    pub fn eq(&self, o: &Value) -> bool {
        match (self, o) {
            (Value::Cons(a), Value::Cons(b)) => Rc::ptr_eq(a, b),
            (Value::Str(a), Value::Str(b)) => Rc::ptr_eq(a, b),
            (Value::Real(_), Value::Real(_)) | (Value::Int(_), Value::Int(_)) => self.equal(o),
            _ => self.equal(o),
        }
    }
}
