use std::error::Error;
use std::fmt::Display;
use std::rc::Rc;
use std::result::Result as StdResult;

use lazy_static::lazy_static;
use pest_derive::Parser;

use pest::iterators::Pairs;
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest::Parser;

pub mod ast;

use self::ast::{Expr, Literal, Statement};
use crate::get_only_one;
use crate::ttype::Type;

lazy_static! {
    static ref EXPR_PARSER: PrattParser<Rule> = {
        use Assoc::*;
        use Rule::*;

        PrattParser::new()
            .op(Op::infix(eq, Left)
                | Op::infix(neq, Left)
                | Op::infix(lt, Left)
                | Op::infix(lte, Left)
                | Op::infix(gt, Left)
                | Op::infix(gte, Left))
            .op(Op::infix(concat, Left))
            .op(Op::infix(add, Left) | Op::infix(subtract, Left))
            .op(Op::infix(multiply, Left) | Op::infix(divide, Left))
            .op(Op::prefix(not) | Op::prefix(r#ref) | Op::prefix(neg) | Op::prefix(deref))
            .op(Op::infix(cast_as, Left))
    };
}

pub fn parse(line: &str) -> Result<Box<[Statement]>> {
    let pairs = EddParser::parse(Rule::program, line)?;

    Ok(EddParser::parse_program(pairs))
}

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct EddParser;

impl EddParser {
    fn parse_literal(lit: Pairs<Rule>) -> Literal {
        let pair = get_only_one(lit);
        match pair.as_rule() {
            Rule::num => {
                let s = pair.as_str();
                s.parse()
                    .map(Literal::Integer)
                    .or_else(|_| s.parse().map(Literal::Float))
                    .unwrap()
            }
            Rule::boolean => match pair.as_str() {
                "true" => Literal::Boolean(true),
                "false" => Literal::Boolean(false),
                _ => unreachable!(),
            },
            Rule::string => {
                let mut buf = String::new();
                for part in pair.into_inner() {
                    match part.as_rule() {
                        Rule::string_part => {
                            buf.push_str(part.as_str());
                        },
                        Rule::escape_c => match part.as_str() {
                            "n" => buf.push('\n'),
                            "r" => buf.push('\r'),
                            "0" => buf.push('\0'),
                            "t" => buf.push('\t'),
                            "\\" => buf.push('\\'),
                            "\'" => buf.push('\''),
                            "\"" => buf.push('\"'),
                            x if x.starts_with('x') => match u8::from_str_radix(&x[1..], 16) {
                                Ok(c @ 0..=0x7f) => buf.push(c as char),
                                _ => unreachable!(),
                            }
                            _ => todo!("return invalid escape sequence error"),
                        },
                        _ => unreachable!(),
                    }
                }
                Literal::String(buf.into())
            },
            r => unreachable!("{r:?}"),
        }
    }
    fn parse_type(mut pairs_t: Pairs<Rule>) -> Option<Type> {
        let t = pairs_t.next();
        assert!(pairs_t.next().is_none());
        let t = get_only_one(t?.into_inner());
        Some(match t.as_rule() {
            Rule::primitive => match t.as_str() {
                "bool" => Type::Bool,
                "byte" => Type::Byte,
                "u8" => Type::U8,
                "i8" => Type::I8,
                "i16" => Type::I16,
                "u16" => Type::U16,
                "i32" => Type::I32,
                "u32" => Type::U32,
                "float" => Type::I8,
                "()" => Type::Unit,
                _ => unreachable!(),
            }
            Rule::opt => Type::Option(Rc::new(Self::parse_type(t.into_inner()).unwrap())),
            Rule::ptr => Type::Pointer(Rc::new(Self::parse_type(t.into_inner()).unwrap())),
            Rule::slice => Type::Slice(Rc::new(Self::parse_type(t.into_inner()).unwrap())),
            Rule::arrptr => Type::ArrayPointer(Rc::new(Self::parse_type(t.into_inner()).unwrap())),
            Rule::array => {
                let mut ps = t.into_inner();
                let size = ps.next().unwrap().as_str().parse().unwrap();
                let t = get_only_one(ps).into_inner();
                Type::Array(Rc::new(Self::parse_type(t).unwrap()), size)
            }
            Rule::r#type => Self::parse_type(t.into_inner()).unwrap(),
            _ => unreachable!(),
        })
    }
    fn parse_expr(expr: Pairs<Rule>) -> Expr {
        EXPR_PARSER
            .map_primary(|primary| match primary.as_rule() {
                Rule::literal => Expr::Const(Self::parse_literal(primary.into_inner())),
                Rule::ident => Expr::Ident(primary.as_str().into()),
                Rule::expr => Self::parse_expr(primary.into_inner()),
                Rule::r#if => {
                    let mut pairs = primary.into_inner();
                    let c = Self::parse_expr(pairs.next().unwrap().into_inner());
                    let t = Self::parse_expr(pairs.next().unwrap().into_inner());
                    let e = Self::parse_expr(get_only_one(pairs).into_inner());
                    Expr::If(Box::new(c), Box::new(t), Box::new(e))
                }
                Rule::lambda => {
                    let mut pairs = primary.into_inner();
                    let idents = pairs
                        .next()
                        .unwrap()
                        .into_inner()
                        .map(|p| p.as_str().into())
                        .collect();
                    let body = Self::parse_expr(get_only_one(pairs).into_inner());

                    Expr::Lambda(idents, Box::new(body))
                }
                Rule::call => {
                    let mut pairs = primary.into_inner();
                    let name = pairs.next().unwrap().as_str().into();
                    let exprs = get_only_one(pairs)
                        .into_inner()
                        .map(|p| Self::parse_expr(p.into_inner()))
                        .collect();

                    Expr::Call(name, exprs)
                }
                r => unreachable!("{r:?}"),
            })
            .map_infix(|lhs, op, rhs| match op.as_rule() {
                Rule::add => Expr::Add(Box::new(lhs), Box::new(rhs)),
                Rule::subtract => Expr::Sub(Box::new(lhs), Box::new(rhs)),
                Rule::multiply => Expr::Mul(Box::new(lhs), Box::new(rhs)),
                Rule::divide => Expr::Div(Box::new(lhs), Box::new(rhs)),
                Rule::eq => Expr::Eq(Box::new(lhs), Box::new(rhs)),
                Rule::neq => Expr::Neq(Box::new(lhs), Box::new(rhs)),
                Rule::lt => Expr::Lt(Box::new(lhs), Box::new(rhs)),
                Rule::lte => Expr::Lte(Box::new(lhs), Box::new(rhs)),
                Rule::gt => Expr::Gt(Box::new(lhs), Box::new(rhs)),
                Rule::gte => Expr::Gte(Box::new(lhs), Box::new(rhs)),
                Rule::concat => Expr::Concat(Box::new(lhs), Box::new(rhs)),
                Rule::cast_as => Expr::Cast(Box::new(lhs), todo!("{}", rhs)),
                _ => unreachable!(),
            })
            .map_prefix(|op, rhs| match op.as_rule() {
                Rule::not => Expr::Not(Box::new(rhs)),
                Rule::r#ref => Expr::Ref(Box::new(rhs)),
                Rule::neg => Expr::Neg(Box::new(rhs)),
                Rule::deref => Expr::Deref(Box::new(rhs)),
                _ => unreachable!(),
            })
            .parse(expr)
    }
    fn parse_statement(mut stmnt: Pairs<Rule>) -> Statement {
        let Some(stmnt) = stmnt.next() else {
            return Statement::Express(Expr::Const(Literal::Unit));
        };
        match stmnt.as_rule() {
            Rule::expr => Statement::Express(Self::parse_expr(stmnt.into_inner())),
            Rule::let_decl => {
                let mut binding = stmnt.into_inner();
                let id = binding.next().unwrap().as_str().into();
                let t_annotation = Self::parse_type(binding.next().unwrap().into_inner());
                let expr = Self::parse_expr(binding.next().unwrap().into_inner());
                Statement::Let(id, t_annotation, expr)
            }
            Rule::var_decl => {
                let mut binding = stmnt.into_inner();
                let id = binding.next().unwrap().as_str().into();
                let t_annotation = Self::parse_type(binding.next().unwrap().into_inner());
                let expr = Self::parse_expr(binding.next().unwrap().into_inner());
                Statement::Var(id, t_annotation, expr)
            }
            Rule::rebind => {
                let mut binding = stmnt.into_inner();
                let id = binding.next().unwrap().as_str().into();
                let expr = Self::parse_expr(binding.next().unwrap().into_inner());
                Statement::Rebind(id, expr)
            }
            Rule::r#return => {
                let expr = get_only_one(stmnt.into_inner());
                Statement::Express(Self::parse_expr(expr.into_inner()))
            }
            e => unreachable!("{e:?}"),
        }
    }
    fn parse_program(mut program: Pairs<Rule>) -> Box<[Statement]> {
        let stmnts = program.next().unwrap()
            .into_inner()
            .map(|p| Self::parse_statement(p.into_inner()))
            .collect();

        assert_eq!(program.next().unwrap().as_rule(), Rule::EOI);

        stmnts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(Box<pest::error::Error<Rule>>);

impl From<pest::error::Error<Rule>> for ParseError {
    fn from(e: pest::error::Error<Rule>) -> Self {
        ParseError(Box::new(e))
    }
}

impl From<ParseError> for pest::error::Error<Rule> {
    fn from(value: ParseError) -> Self {
        *value.0
    }
}

impl Display for ParseError {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

pub type Result<T> = StdResult<T, ParseError>;