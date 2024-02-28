use std::cmp::Ordering;
use std::fmt::{self, Display};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::i128;

use crate::rt::RuntimeError;

#[derive(Debug, Clone)]
pub enum Query {
    Inquire(Expr),
    Let(String, Expr),
    Var(String, Expr),
    Rebind(String, Expr),
}

#[derive(Debug, Clone, Copy)]
pub enum Literal {
    Integer(i128),
    Float(f64),
    Boolean(bool),
}

impl Hash for Literal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match *self {
            Literal::Float(f) => state.write_u64(f.to_bits()),
            Literal::Integer(i) => i.hash(state),
            Literal::Boolean(i) => i.hash(state),
        }
    }
}

mod literal_impl;

#[derive(Debug, Clone, Hash)]
pub enum Expr {
    Ident(String),
    Val(Literal),
    Add(Box<Self>, Box<Self>),
    Sub(Box<Self>, Box<Self>),
    Mul(Box<Self>, Box<Self>),
    Div(Box<Self>, Box<Self>),
    Pow(Box<Self>, Box<Self>),

    Not(Box<Self>),
    Ref(Box<Self>),
    Neg(Box<Self>),
    Deref(Box<Self>),

    If(Box<Self>, Box<Self>, Box<Self>),
    Eq(Box<Self>, Box<Self>),
    Neq(Box<Self>, Box<Self>),
    Lt(Box<Self>, Box<Self>),
    Lte(Box<Self>, Box<Self>),
    Gt(Box<Self>, Box<Self>),
    Gte(Box<Self>, Box<Self>),

    /// Not parsed
    Raise(RuntimeError)
}

fn try_binop<F, F2>(a: Expr, b: Expr, binop: F, fallback: F2) -> Expr
where
    F: FnOnce(Literal, Literal) -> Result<Literal, RuntimeError>,
    F2: FnOnce(Box<Expr>, Box<Expr>) -> Expr,
{
    match (a, b) {
        (Expr::Val(a), Expr::Val(b)) => binop(a, b)
            .map(Expr::Val)
            .unwrap_or_else(|e| Expr::Raise(e)),
        (a, b) => fallback(Box::new(a), Box::new(b)),
    }
}

const MAX_RECUR: usize = 256;

impl Expr {
    pub fn eval_const<F: FnMut(&str) -> Expr>(mut self, mut lookup: F) -> Self {
        let hash = |expr: &Expr| {
            let mut hsher = DefaultHasher::new();
            expr.hash(&mut hsher);
            hsher.finish()
        };

        let mut hashes = vec![hash(&self)];
        for _ in 0..MAX_RECUR {
            self = self.eval_const_inner(&mut lookup);
            let hash = hash(&self);
            if hashes.contains(&hash) {
                break;
            } else {
                hashes.push(hash);
            }
        }
        self
    }
    fn eval_const_inner<F: FnMut(&str) -> Expr>(self, lookup: &mut F) -> Self {
        match self {
            Expr::Ident(i) => lookup(&i),
            Expr::Val(v) => Expr::Val(v),
            Expr::Raise(e) => Expr::Raise(e),
            Expr::Add(a, b) => {
                let a = a.eval_const_inner(lookup);
                let b = b.eval_const_inner(lookup);

                if a.is_const_zero() {
                    b
                } else if b.is_const_zero() {
                    a
                } else {
                    try_binop(a, b, |a, b| a + b, Expr::Add)
                }
            }
            Expr::Sub(a, b) => {
                let a = a.eval_const_inner(lookup);
                let b = b.eval_const_inner(lookup);

                if b.is_const_zero() {
                    a
                } else {
                    try_binop(a, b, |a, b| a - b, Expr::Sub)
                }
            }
            Expr::Mul(a, b) => {
                let a = a.eval_const_inner(lookup);
                let b = b.eval_const_inner(lookup);

                if a.is_const_zero() || b.is_const_zero() {
                    Expr::Val(Literal::Integer(0))
                } else if a.is_const_one() || b.is_const_one() {
                    Expr::Val(Literal::Integer(1))
                } else {
                    try_binop(a, b, |a, b| a * b, Expr::Mul)
                }
            }
            Expr::Div(a, b) => {
                let a = a.eval_const_inner(lookup);
                let b = b.eval_const_inner(lookup);

                if b.is_const_zero() {
                    Expr::Raise(RuntimeError::DivideByZero)
                } else if b.is_const_one() {
                    a
                } else {
                    try_binop(a, b, |a, b| a / b, Expr::Div)
                }
            }
            Expr::Pow(a, b) => {
                let a = a.eval_const_inner(lookup);
                let b = b.eval_const_inner(lookup);

                match (a.is_const_zero(), b.is_const_zero()) {
                    (false, false) if a.is_const_one() => Expr::Val(Literal::Integer(1)),
                    (false, false) if b.is_const_one() => a,
                    (false, false) => try_binop(a, b, Literal::pow, Expr::Pow),
                    (true, false) => Expr::Val(Literal::Integer(0)),
                    (false, true) => Expr::Val(Literal::Integer(1)),
                    (true, true) => Expr::Raise(RuntimeError::ZeroToTheZeroeth),
                }
            }
            Expr::If(cond, if_true, if_false) => {
                let cond = cond.eval_const_inner(lookup);

                match cond {
                    Expr::Val(Literal::Boolean(true)) => if_true.eval_const_inner(lookup),
                    Expr::Val(Literal::Boolean(false)) => if_false.eval_const_inner(lookup),
                    Expr::Val(_) => Expr::Raise(RuntimeError::ExpectedBooleanInCond),
                    c => Expr::If(
                        Box::new(c),
                        Box::new(if_true.eval_const_inner(lookup)),
                        Box::new(if_false.eval_const_inner(lookup)),
                    ),
                }
            }
            Expr::Not(rhs) => {
                let rhs = rhs.eval_const_inner(lookup);
                match rhs {
                    Expr::Val(Literal::Boolean(b)) => Expr::Val(Literal::Boolean(!b)),
                    Expr::Val(_) => Expr::Raise(RuntimeError::InvalidOperation("not", "non-boolean")),
                    _ => Expr::Not(Box::new(rhs)),
                }
            }
            Expr::Ref(rhs) => {
                let rhs = rhs.eval_const_inner(lookup);
                Expr::Ref(Box::new(rhs))
            }
            Expr::Neg(rhs) => {
                let rhs = rhs.eval_const_inner(lookup);
                match rhs {
                    Expr::Val(Literal::Integer(i @ i128::MAX)) => Expr::Raise(RuntimeError::IntOverflow("neg", i, 0)),
                    Expr::Val(Literal::Integer(i)) => Expr::Val(Literal::Integer(-i)),
                    Expr::Val(Literal::Float(f)) => Expr::Val(Literal::Float(f)),
                    Expr::Val(_) => Expr::Raise(RuntimeError::InvalidOperation("neg", "non-numeral")),
                    _ => Expr::Neg(Box::new(rhs)),
                }
            }
            Expr::Deref(rhs) => {
                let rhs = rhs.eval_const_inner(lookup);
                Expr::Deref(Box::new(rhs))
            }
            Expr::Eq(a, b) => try_binop(
                a.eval_const_inner(lookup),
                b.eval_const_inner(lookup),
                |a, b| a.cmp_op(b, Ordering::Equal, false),
                Expr::Eq,
            ),
            Expr::Neq(a, b) => try_binop(
                a.eval_const_inner(lookup),
                b.eval_const_inner(lookup),
                |a, b| a.cmp_op(b, Ordering::Equal, true),
                Expr::Neq,
            ),
            Expr::Lt(a, b) => try_binop(
                a.eval_const_inner(lookup),
                b.eval_const_inner(lookup),
                |a, b| a.cmp_op(b, Ordering::Less, false),
                Expr::Lt,
            ),
            Expr::Lte(a, b) => try_binop(
                a.eval_const_inner(lookup),
                b.eval_const_inner(lookup),
                |a, b| a.cmp_op(b, Ordering::Greater, true),
                Expr::Lte,
            ),
            Expr::Gt(a, b) => try_binop(
                a.eval_const_inner(lookup),
                b.eval_const_inner(lookup),
                |a, b| a.cmp_op(b, Ordering::Greater, false),
                Expr::Gt,
            ),
            Expr::Gte(a, b) => try_binop(
                a.eval_const_inner(lookup),
                b.eval_const_inner(lookup),
                |a, b| a.cmp_op(b, Ordering::Less, true),
                Expr::Gte,
            ),
        }
    }
    fn is_const_zero(&self) -> bool {
        match self {
            Self::Val(Literal::Integer(0)) => true,
            Self::Val(Literal::Float(f)) => f.abs() < f64::EPSILON,
            _ => false,
        }
    }
    fn is_const_one(&self) -> bool {
        match self {
            Self::Val(Literal::Integer(1)) => true,
            Self::Val(Literal::Float(f)) => (f - 1.).abs() < f64::EPSILON,
            _ => false,
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Ident(i) => write!(f, "{i}"),
            Expr::Val(v) => write!(f, "{v}"),
            Expr::Add(a, b) => write!(f, "({a} + {b})"),
            Expr::Sub(a, b) => write!(f, "({a} - {b})"),
            Expr::Mul(a, b) => write!(f, "({a} * {b})"),
            Expr::Div(a, b) => write!(f, "({a} / {b})"),
            Expr::Pow(a, b) => write!(f, "({a} ^ {b})"),
            Expr::If(cond, if_t, if_f) => write!(f, "(if {cond} then {if_t} else {if_f})"),
            Expr::Eq(a, b) => write!(f, "({a} == {b})"),
            Expr::Neq(a, b) => write!(f, "({a} != {b})"),
            Expr::Lt(a, b) => write!(f, "({a} < {b})"),
            Expr::Lte(a, b) => write!(f, "({a} <= {b})"),
            Expr::Gt(a, b) => write!(f, "({a} > {b})"),
            Expr::Gte(a, b) => write!(f, "({a} >= {b})"),
            Expr::Not(a) => write!(f, "!{a}"),
            Expr::Ref(a) => write!(f, "&{a}"),
            Expr::Neg(a) => write!(f, "-{a}"),
            Expr::Deref(a) => write!(f, "*{a}"),
            Expr::Raise(e) => write!(f, "raise(error({e}))"),
        }
    }
}

impl Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Query::Inquire(e) => write!(f, "{e}"),
            Query::Let(n, e) => write!(f, "let {n} = {e}"),
            Query::Var(n, e) => write!(f, "var {n} = {e}"),
            Query::Rebind(n, e) => write!(f, "{n} = {e}"),
        }
    }
}
