#![allow(warnings)]

use super::{
    lang::{
        *,
        InnerTerm::*
    },
    eval::*
};
use std::{
    collections::{
        HashSet,
        hash_map::{
            HashMap,
            IntoIter
        }
    },
    iter::*
};

#[derive(Copy, Clone, Debug, PartialEq, Hash, Eq)]
pub enum ContextEntryKind {
    Dec,
    Def
}

#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub struct ContextEntry {
    pub dec: Option<Term>,
    pub def: Option<Term>
}

#[derive(Clone, Debug)]
pub struct Context(HashMap<usize, ContextEntry>);

use ContextEntryKind::*;

impl Context {
    pub fn new() -> Self {
        Context(HashMap::new())
    }

    pub fn exists_dec(&self, index: usize) -> bool {
        if let Some(ContextEntry { dec: Some(_), def: _ }) = self.0.get(&index) {
            true
        } else {
            false
        }
    }

    pub fn exists_def(&self, index: usize) -> bool {
        if let Some(ContextEntry { dec: _, def: Some(_) }) = self.0.get(&index) {
            true
        } else {
            false
        }
    }

    pub fn get(&self, index: usize) -> Option<ContextEntry> {
        match self.0.get(&index) {
            Some(term) => Some(term.clone()),
            None => None
        }
    }

    pub fn get_dec(&self, index: usize) -> Option<Term> {
        self.get(index)?.dec
    }

    pub fn get_def(&self, index: usize) -> Option<Term> {
        self.get(index)?.def
    }

    pub fn inc(self, amount: isize) -> Self {
        let new = Context(self.0.into_iter().map(|(k, v)| (((k as isize) + amount) as usize, v)).collect());
        new
    }

    pub fn with(mut self, index: usize, kind: ContextEntryKind, entry: Term) -> Self {
        if let Some(ContextEntry { ref mut dec, ref mut def }) = self.0.get_mut(&index) {
            match (kind, &dec, &def) {
                (Dec, None, _) => *dec = Some(entry),
                (Def, _, None) => *def = Some(entry),
                _ => panic!("BUG: duplicate var")
            }
        } else {
            let old = match kind {
                Dec => self.0.insert(index, ContextEntry { dec: Some(entry), def: None }),
                Def => self.0.insert(index, ContextEntry { dec: None, def: Some(entry) })
            };
        }
        Context(self.0)
    }

    pub fn with_dec(self, index: usize, entry: Term) -> Self {
        self.with(index, Dec, entry)
    }

    pub fn with_def(self, index: usize, entry: Term) -> Self {
        self.with(index, Def, entry)
    }

    pub fn without(mut self, index: usize) -> Self {
        self.0.remove(&index);
        self
    }

    pub fn indices(&self) -> HashSet<usize> {
        let mut set = HashSet::new();
        for key in self.0.keys() {
            set.insert(*key);
        }
        set
    }

    pub fn shift(self, amount: isize) -> Self {
        Context(self.0.into_iter().map(|(k, v)| {
            (k,
            ContextEntry {
                dec: match v.dec {
                    Some(dec) => Some(shift(dec, HashSet::new(), amount)),
                    None => None
                },
                def: match v.def {
                    Some(def) => Some(shift(def, HashSet::new(), amount)),
                    None => None
                }})}).collect())
    }

    pub fn update(self, index: usize, val: Term) -> Self {
        Context(self.0.into_iter().map(|(k, v)|
            (k,
            ContextEntry {
                dec: match v.dec {
                    Some(dec) => Some(normalize(dec, Context::new().with_def(index + k, val.clone()))),
                    None => None
                },
                def: match v.def {
                    Some(def) => Some(normalize(def, Context::new().with_def(index + k, val.clone()))),
                    None => None
                }})).collect())
    }

    pub fn inc_and_shift(self, amount: isize) -> Self {
        self.inc(amount).shift(amount as isize)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn into_iter(self) -> ContextIterator {
        ContextIterator::new(self)
    }

    pub fn to_caps_list(self, exp_level: usize) -> Term {
        let core_nil =
            Term::new(
                Box::new(CapturesListIntro(List::Nil)),
                Some(Box::new(Term::new_with_note(
                    Note("generated 1".to_string()),
                    Box::new(CapturesListTypeIntro(exp_level)),
                    Some(Box::new(Term::new(
                        Box::new(TypeTypeIntro(exp_level, Usage::Unique)),
                        None)))))));
        let list = self.into_iter().fold(core_nil, |curr_list, entry| {
            Term::new(
                Box::new(CapturesListIntro(List::Cons(entry.1.dec.unwrap(), curr_list))),
                Some(Box::new(Term::new_with_note(
                    Note("generated 2".to_string()),
                    Box::new(CapturesListTypeIntro(exp_level)),
                    Some(Box::new(Term::new(
                        Box::new(TypeTypeIntro(exp_level, Usage::Unique)),
                        None)))))))});
        list
    }
}

pub struct ContextIterator {
    inner_iter: IntoIter<usize, ContextEntry>,
}

impl ContextIterator {
    fn new(context: Context) -> ContextIterator {
        ContextIterator { inner_iter: context.0.into_iter() }
    }
}

impl Iterator for ContextIterator {
    type Item = (usize, ContextEntry);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter.next()
    }
}

// impl DoubleEndedIterator for ContextIterator {
//     fn next_back(&mut self) -> Option<Self::Item> {
//         self.inner_iter.next_back()
//     }
// }