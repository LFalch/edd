use std::{collections::HashMap, rc::Rc};

use crate::parse::location::Location;

use super::{unify_types, Result, Type, TypeErrorType};

#[derive(Debug, Clone)]
pub struct Symbol {
    mutable: bool,
    s_type: Type,
}
impl Symbol {
    fn new(s_type: Type, mutable: bool) -> Self {
        Symbol { mutable, s_type }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    map: HashMap<Rc<str>, Symbol>,
}

impl SymbolTable {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add<S: Into<Rc<str>>>(&mut self, mutable: bool, name: S, ty: Type) -> bool {
        self.map
            .insert(name.into(), Symbol::new(ty, mutable))
            .is_some()
    }
    pub fn lookup_raw(&self, name: &str) -> Result<Symbol, TypeErrorType> {
        self.map
            .get(name)
            .cloned()
            .ok_or_else(|| TypeErrorType::Undefined(name.into()))
    }
    pub fn lookup(&self, name: &str) -> Result<Type, TypeErrorType> {
        self.lookup_raw(name).map(|sym| sym.s_type)
    }
    pub fn specify(&mut self, loc: &Location, name: &str, t: &Type) -> Result<Type> {
        let et = &self.map.get(name).unwrap().s_type;
        let ut = unify_types(loc, et, t)?;
        self.map.get_mut(name).unwrap().s_type = ut.clone();

        Ok(ut)
    }
    pub fn mutate(&mut self, loc: &Location, name: &str, t: &Type) -> Result<Type> {
        let Some(Symbol { s_type, mutable }) = self.map.get(name) else {
            return Err(TypeErrorType::Undefined(name.into()).location(loc.clone()));
        };
        if !mutable {
            return Err(TypeErrorType::NotMutable(name.into()).location(loc.clone()));
        }
        let ut = unify_types(loc, t, s_type)?;
        self.map.get_mut(name).unwrap().s_type = ut.clone();

        Ok(ut)
    }
}
