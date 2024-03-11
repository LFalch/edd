use crate::parse::span::Span;

use std::rc::Rc;

use super::Type;

mod impls;
#[derive(Debug, Clone)]
pub struct Program(pub Box<[(Rc<str>, Decl)]>);

#[derive(Debug, Clone)]
pub enum Decl {
    Static(Span, Box<(Type, Expr)>),
    Const(Span, Box<(Type, Expr)>),
    Fn(Span, Box<[(Rc<str>, Type)]>, Box<(Type, Expr)>),
}
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Express(Span, Box<Type>, Expr),
    Let(Span, Rc<str>, Box<Type>, Expr),
    Var(Span, Rc<str>, Box<Type>, Expr),
    Rebind(Span, PlaceExpr, Expr),

    Return(Span, Expr),
}
#[derive(Debug, Clone, PartialEq)]
pub enum PlaceExpr {
    Ident(Span, Rc<str>),
    Deref(Span, Box<Expr>),
    Index(Span, Box<Expr>, Box<Expr>),
    FieldAccess(Span, Box<Expr>, Rc<str>),
}
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Ident(Span, Rc<str>),
    ConstBoolean(Span, bool),
    ConstI8(Span, i8),
    ConstU8(Span, u8),
    ConstI16(Span, i16),
    ConstU16(Span, u16),
    ConstI32(Span, i32),
    ConstU32(Span, u32),
    ConstFloat(Span, f64),
    ConstCompInteger(Span, i128),
    ConstUnit(Span),
    ConstString(Span, Rc<str>),
    ConstNull(Span),

    Ref(Span, Result<PlaceExpr, Box<Self>>),
    Array(Span, Box<[Self]>),
    StructConstructor(Span, Box<[(Option<Box<str>>, Expr)]>),
    /// Span, first type is the original type, the second is the target
    Cast(Span, Box<Self>, Box<Type>, Box<Type>),

    Add(Span, Box<Self>, Box<Self>),
    Sub(Span, Box<Self>, Box<Self>),
    Mul(Span, Box<Self>, Box<Self>),
    Div(Span, Box<Self>, Box<Self>),
    Concat(Span, Box<Self>, Box<Self>),

    Not(Span, Box<Self>),
    Neg(Span, Box<Self>),
    Deref(Span, Box<Self>),

    Block(Span, Box<[Statement]>),
    Lambda(Span, Box<[(Rc<str>, Type)]>, Type, Box<Self>),
    Call(Span, Rc<str>, Box<[Self]>),

    If(Span, Box<Self>, Box<Self>, Box<Self>),
    Eq(Span, Box<Self>, Box<Self>, Box<Type>),
    Neq(Span, Box<Self>, Box<Self>, Box<Type>),
    Lt(Span, Box<Self>, Box<Self>, Box<Type>),
    Lte(Span, Box<Self>, Box<Self>, Box<Type>),
    Gt(Span, Box<Self>, Box<Self>, Box<Type>),
    Gte(Span, Box<Self>, Box<Self>, Box<Type>),
}

#[allow(dead_code)]
impl Expr {
    fn spanned(self, span: Span) -> Self {
        match self {
            Expr::Ident(_, a) => Expr::Ident(span, a),
            Expr::ConstBoolean(_, a) => Expr::ConstBoolean(span, a),
            Expr::ConstI8(_, a) => Expr::ConstI8(span, a),
            Expr::ConstU8(_, a) => Expr::ConstU8(span, a),
            Expr::ConstI16(_, a) => Expr::ConstI16(span, a),
            Expr::ConstU16(_, a) => Expr::ConstU16(span, a),
            Expr::ConstI32(_, a) => Expr::ConstI32(span, a),
            Expr::ConstU32(_, a) => Expr::ConstU32(span, a),
            Expr::ConstFloat(_, a) => Expr::ConstFloat(span, a),
            Expr::ConstCompInteger(_, a) => Expr::ConstCompInteger(span, a),
            Expr::ConstUnit(_) => Expr::ConstUnit(span),
            Expr::ConstString(_, a) => Expr::ConstString(span, a),
            Expr::ConstNull(_) => Expr::ConstNull(span),
            Expr::Ref(_, a) => Expr::Ref(span, a),
            Expr::Array(_, a) => Expr::Array(span, a),
            Expr::StructConstructor(_, a) => Expr::StructConstructor(span, a),
            Expr::Cast(_, a, b, c) => Expr::Cast(span, a, b, c),
            Expr::Add(_, a, b) => Expr::Add(span, a, b),
            Expr::Sub(_, a, b) => Expr::Sub(span, a, b),
            Expr::Mul(_, a, b) => Expr::Mul(span, a, b),
            Expr::Div(_, a, b) => Expr::Div(span, a, b),
            Expr::Concat(_, a, b) => Expr::Concat(span, a, b),
            Expr::Not(_, a) => Expr::Not(span, a),
            Expr::Neg(_, a) => Expr::Neg(span, a),
            Expr::Deref(_, a) => Expr::Deref(span, a),
            Expr::Block(_, a) => Expr::Block(span, a),
            Expr::Lambda(_, a, b, c) => Expr::Lambda(span, a, b, c),
            Expr::Call(_, a, b) => Expr::Call(span, a, b),
            Expr::If(sp, a, b, c) => Expr::If(sp, a, b, c),
            Expr::Eq(sp, a, b, c) => Expr::Eq(sp, a, b, c),
            Expr::Neq(_, a, b, c) => Expr::Neq(span, a, b, c),
            Expr::Lt(sp, a, b, c) => Expr::Lt(sp, a, b, c),
            Expr::Lte(_, a, b, c) => Expr::Lte(span, a, b, c),
            Expr::Gt(sp, a, b, c) => Expr::Gt(sp, a, b, c),
            Expr::Gte(_, a, b, c) => Expr::Gte(span, a, b, c),
        }
    }
    fn is_const_zero(&self) -> bool {
        match self {
            Expr::ConstU8(_, 0)
            | Expr::ConstI8(_, 0)
            | Expr::ConstU16(_, 0)
            | Expr::ConstI16(_, 0)
            | Expr::ConstU32(_, 0)
            | Expr::ConstI32(_, 0)
            | Expr::ConstCompInteger(_, 0) => true,
            Expr::ConstFloat(_, f) => f.abs() < f64::EPSILON,
            _ => false,
        }
    }
    fn is_const_one(&self) -> bool {
        match self {
            Expr::ConstU8(_, 1)
            | Expr::ConstI8(_, 1)
            | Expr::ConstU16(_, 1)
            | Expr::ConstI16(_, 1)
            | Expr::ConstU32(_, 1)
            | Expr::ConstI32(_, 1)
            | Expr::ConstCompInteger(_, 1) => true,
            Expr::ConstFloat(_, f) => (f - 1.).abs() < f64::EPSILON,
            _ => false,
        }
    }
}
