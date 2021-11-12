pub mod text;
pub mod common;
pub mod interpreter;
pub mod arithmetics;

mod matcher;
#[cfg(test)]
mod tests;

#[macro_use]
extern crate mopa;

use text::{SExprSpace};

use std::fmt::{Display, Debug};
use std::collections::HashMap;

// Macros to simplify expression writing

#[macro_export]
macro_rules! expr {
    () => { Atom::expr(&[]) };
    ($x:ident) => { Atom::var(stringify!($x)) };
    ($x:literal) => { Atom::sym($x) };
    (($($x:tt),*)) => { Atom::expr(&[ $( expr!($x) , )* ]) };
    ($($x:tt),*) => { Atom::expr(&[ $( expr!($x) , )* ]) };
}

#[macro_export]
macro_rules! bind {
    ($($k:ident: $v:expr),*) => { vec![$( (VariableAtom::from(stringify!($k)), $v), )*]
        .iter().cloned().collect() };
}

// Symbol atom

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolAtom {
    name: String,
}

impl From<String> for SymbolAtom {
    fn from(name: String) -> Self {
        SymbolAtom{ name }
    }
}

impl From<&str> for SymbolAtom {
    fn from(name: &str) -> Self {
        SymbolAtom::from(name.to_string())
    }
}

impl SymbolAtom {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl Display for SymbolAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

// Expression atom

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionAtom {
    children: Vec<Atom>,
}

impl ExpressionAtom {
    pub fn is_plain(&self) -> bool {
        self.children.iter().all(|atom| ! matches!(atom, Atom::Expression(_)))
    }

    // Without lifetime annotations compiler makes lifetime elision incorrectly.
    // It deduces iter<'a>(&'a self) -> ExpressionAtomIter<'a>
    pub fn sub_expr_iter<'a, 'b>(&'a self) -> ExpressionAtomIter<'b> {
        ExpressionAtomIter::from(self)
    }

    pub fn children(&self) -> &Vec<Atom> {
        &self.children
    }

    pub fn children_mut(&mut self) -> &mut Vec<Atom> {
        &mut self.children
    }
}

impl From<Vec<Atom>> for ExpressionAtom {
    fn from(children: Vec<Atom>) -> Self {
        ExpressionAtom{ children }
    }
}

impl From<&[Atom]> for ExpressionAtom {
    fn from(children: &[Atom]) -> Self {
        ExpressionAtom::from(children.to_vec())
    }
}

impl Display for ExpressionAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(")
            .and_then(|_| self.children.iter().take(1).fold(Ok(()),
                |res, atom| res.and_then(|_| write!(f, "{}", atom))))
            .and_then(|_| self.children.iter().skip(1).fold(Ok(()),
                |res, atom| res.and_then(|_| write!(f, " {}", atom))))
            .and_then(|_| write!(f, ")"))
    }
}

struct ExpressionLevel<'a> {
    expr: &'a ExpressionAtom,
    idx: Option<usize>,
    iter: std::iter::Enumerate<std::slice::Iter<'a, Atom>>,
}

impl<'a> ExpressionLevel<'a> {
    fn from(expr: *const ExpressionAtom, idx: Option<usize>) -> Self {
        unsafe {
            let rexpr = &*expr as &ExpressionAtom;
            ExpressionLevel{ expr: rexpr, idx, iter: rexpr.children.iter().enumerate() }
        }
    }
}

pub struct ExpressionAtomIter<'a> {
    expr: *mut ExpressionAtom,
    levels: Vec<ExpressionLevel<'a>>,
}

impl Drop for ExpressionAtomIter<'_> {
    fn drop(&mut self) {
        unsafe {
            // free heap memory by wrapping by box and dropping it
            drop(Box::from_raw(self.expr));
        }
    }
}

impl<'a> ExpressionAtomIter<'a> {
    fn from(expr: &ExpressionAtom) -> Self {
        // get memory on heap under our control
        let ptr = Box::into_raw(Box::new(expr.clone()));
        Self{ expr: ptr, levels: vec![ExpressionLevel::from(ptr, None)] }
    }
}

impl<'a> Iterator for ExpressionAtomIter<'a> {
    type Item = (&'a ExpressionAtom, &'a ExpressionAtom, usize);

    fn next(&mut self) -> Option<Self::Item> {
        while !self.levels.is_empty() {
            let level = self.levels.last_mut().unwrap();
            let next = level.iter.next();
            if None == next {
                let expr = level.expr;
                let idx = level.idx;
                self.levels.pop();
                if !self.levels.is_empty() {
                    let parent = self.levels.last().unwrap().expr;
                    return Some((expr, parent, idx.unwrap()))
                }
            } else {
                let (idx, next) = next.unwrap();
                if let Atom::Expression(next) = next {
                    self.levels.push(ExpressionLevel::from(next, Some(idx)))
                }
            }
        }
        None
    }
}

// Variable atom

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct VariableAtom {
    name: String,
}

impl From<String> for VariableAtom {
    fn from(name: String) -> Self {
        VariableAtom{ name }
    }
}

impl From<&str> for VariableAtom {
    fn from(name: &str) -> Self {
        VariableAtom::from(name.to_string())
    }
}

impl VariableAtom {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl Display for VariableAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "${}", self.name)
    }
}

// Grounded atom

pub trait GroundedAtom : Display + mopa::Any {
    fn execute(&self, _ops: &mut Vec<Atom>, _data: &mut Vec<Atom>) -> Result<(), String> {
        Err(format!("{} is not executable", self))
    }
    fn eq_gnd(&self, other: &dyn GroundedAtom) -> bool;
    fn clone_gnd(&self) -> Box<dyn GroundedAtom>;
}

impl Debug for dyn GroundedAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

mopafy!(GroundedAtom);

// GroundedAtom implementation for all "regular" types
// to allow using them as GroundedAtoms
impl<T: 'static + Clone + PartialEq + Display> GroundedAtom for T {
    fn eq_gnd(&self, other: &dyn GroundedAtom) -> bool {
        match other.downcast_ref::<T>() {
            Some(o) => self.eq(o),
            None => false,
        }
    }

    fn clone_gnd(&self) -> Box<dyn GroundedAtom> {
        Box::new(self.clone())
    }
}

// Atom enum

#[derive(Debug)]
pub enum Atom {
    Symbol(SymbolAtom),
    Expression(ExpressionAtom),
    Variable(VariableAtom),
    // We need using Box here because:
    // - we cannot use GroundedAtom because trait size is not known at compile time
    // - reference to trait does not allow heap allocated values
    // - other smart pointers like Rc doesn't allow choosing to copy value or not
    Grounded(Box<dyn GroundedAtom>),
}

impl Atom {
    pub fn sym(name: &str) -> Self {
        Self::Symbol(SymbolAtom::from(name))
    }

    pub fn expr(children: &[Atom]) -> Self {
        Self::Expression(ExpressionAtom::from(children))
    }

    pub fn var(name: &str) -> Self {
        Self::Variable(VariableAtom::from(name))
    }

    pub fn gnd<T: GroundedAtom>(gnd: T) -> Atom {
        Self::Grounded(Box::new(gnd))
    }

    pub fn as_expr(&self) -> Option<&ExpressionAtom> {
        match self {
            Atom::Expression(ref expr) => Some(expr),
            _ => None,
        }
    }

    pub fn as_expr_mut(&mut self) -> Option<&mut ExpressionAtom> {
        match self {
            Atom::Expression(ref mut expr) => Some(expr),
            _ => None,
        }
    }
}

impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Atom::Symbol(sym1), Atom::Symbol(sym2)) => sym1 == sym2,
            (Atom::Expression(expr1), Atom::Expression(expr2)) => expr1 == expr2,
            (Atom::Variable(var1), Atom::Variable(var2)) => var1 == var2,
            (Atom::Grounded(gnd1), Atom::Grounded(gnd2)) => gnd1.eq_gnd(&**gnd2),
            _ => false,
        }
    }
}

impl Clone for Atom {
    fn clone(&self) -> Self {
        match self {
            Atom::Symbol(sym) => Atom::Symbol(sym.clone()),
            Atom::Expression(expr) => Atom::Expression(expr.clone()),
            Atom::Variable(var) => Atom::Variable(var.clone()),
            Atom::Grounded(gnd) => Atom::Grounded((*gnd).clone_gnd()),
        }
    }
}

impl Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Atom::Symbol(sym) => Display::fmt(sym, f),
            Atom::Expression(expr) => Display::fmt(expr, f),
            Atom::Variable(var) => Display::fmt(var, f),
            Atom::Grounded(gnd) => Display::fmt(gnd, f),
        }
    }
}

// Grounding space

pub type Bindings = HashMap<VariableAtom, Atom>;

// FIXME: clone should not be required but as interpret puts the GroundingSpace
// as Atom into data stack we cannot run interpret without moving into
// into interpreter.
#[derive(Clone)]
pub struct GroundingSpace {
    content: Vec<Atom>,
}

impl GroundingSpace {

    pub fn new() -> Self {
        Self{ content: Vec::new() }
    }
    
    pub fn add(&mut self, atom: Atom) {
        self.content.push(atom)
    }

    pub fn query(&self, pattern: &Atom) -> Vec<Bindings> {
        let mut result = Vec::new();
        for next in &self.content {
            match matcher::match_atoms(next, pattern) {
                Some((a_bindings, b_bindings)) => {
                    let bindings = matcher::apply_bindings_to_bindings(&a_bindings, &b_bindings);
                    result.push(bindings);
                },
                None => continue,
            }
        }
        result
    }

    pub fn interpret(&self, ops: &mut Vec<Atom>, data: &mut Vec<Atom>) -> Result<(), String> {
        let op = ops.pop();
        match op {
            Some(Atom::Grounded(atom)) => atom.execute(ops, data),
            Some(_) => Err("Ops stack contains non grounded atom".to_string()),
            None => Err("Ops stack is empty".to_string()),
        }
    }

    pub fn as_vec(&self) -> &Vec<Atom> {
        &self.content
    }

    pub fn atom_iter(&self) -> std::slice::Iter<Atom>{
        self.content.iter()
    }
}

impl PartialEq for GroundingSpace {
    fn eq(&self, other: &Self) -> bool {
        self.content == other.content
    }
}

impl Display for GroundingSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GroundingSpace")
    }
}

impl From<&SExprSpace> for GroundingSpace {
    fn from(other: &SExprSpace) -> Self {
        let mut space = GroundingSpace::new();
        other.into_grounding_space(&mut space);
        space
    }
}