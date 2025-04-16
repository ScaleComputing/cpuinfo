//! Provide a means to work with and diff sets of facts
//!

use serde::{Deserialize, Serialize};
use std::cmp::Eq;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::rc::Rc;

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct GenericFact<T> {
    pub name: String,
    pub value: T,
}

impl<T> GenericFact<T> {
    pub fn new(name: String, value: T) -> Self {
        Self { name, value }
    }
    pub fn from<F: Into<T>>(other: GenericFact<F>) -> Self {
        Self {
            name: other.name,
            value: other.value.into(),
        }
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn add_path(&mut self, path: &str) -> &mut Self {
        self.name.insert(0, '/');
        self.name.insert_str(0, path);
        self
    }
}

impl<T: Display> Display for GenericFact<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{} = {}", self.name, self.value)
    }
}

impl<T> From<(String, T)> for GenericFact<T> {
    fn from(f: (String, T)) -> Self {
        GenericFact {
            name: f.0,
            value: f.1,
        }
    }
}

impl<T> From<(&str, T)> for GenericFact<T> {
    fn from(f: (&str, T)) -> Self {
        GenericFact {
            name: String::from(f.0),
            value: f.1,
        }
    }
}

pub trait Facter<T> {
    fn collect_facts(&self) -> Vec<T>;
}

pub struct FactSet<T> {
    backing: HashMap<String, Rc<GenericFact<T>>>,
    name_set: HashSet<String>,
}

pub struct NameIteration<'s, T, I: 's + Iterator> {
    iter: I,
    backing: &'s HashMap<String, Rc<GenericFact<T>>>,
}

impl<'s, T, I: Iterator<Item = &'s String> + 's> Iterator for NameIteration<'s, T, I> {
    type Item = &'s GenericFact<T>;
    fn next(&mut self) -> Option<Self::Item> {
        let key = self.iter.next()?;
        self.backing.get(key).map(|v| v.as_ref())
    }
}

pub struct ChangedIterator<'s, T, I: 's + Iterator> {
    iter: I,
    backing_from: &'s HashMap<String, Rc<GenericFact<T>>>,
    backing_to: &'s HashMap<String, Rc<GenericFact<T>>>,
}

impl<'s, T: PartialEq, I: Iterator<Item = &'s String> + 's> Iterator for ChangedIterator<'s, T, I> {
    type Item = (&'s GenericFact<T>, &'s GenericFact<T>);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.iter.next()?;

            let from = match self.backing_from.get(next) {
                Some(v) => v.as_ref(),
                None => continue,
            };
            let to = match self.backing_to.get(next) {
                Some(v) => v.as_ref(),
                None => continue,
            };

            if from != to {
                break Some((from, to));
            }
        }
    }
}

impl<T: PartialEq + Eq + Hash> FactSet<T> {
    /// Facts that are in to but not in self
    pub fn added_facts<'to>(
        &'to self,
        to: &'to Self,
    ) -> NameIteration<'to, T, impl Iterator<Item = &'to String>> {
        let name_iter = to.name_set.difference(&self.name_set);
        NameIteration {
            iter: name_iter,
            backing: &to.backing,
        }
    }

    /// Facts that are in self but not in to
    pub fn removed_facts<'to>(
        &'to self,
        to: &'to Self,
    ) -> NameIteration<'to, T, impl Iterator<Item = &'to String>> {
        let name_iter = self.name_set.difference(&to.name_set);
        NameIteration {
            iter: name_iter,
            backing: &self.backing,
        }
    }

    /// Facts that are in both self and to, but are different
    pub fn changed_facts<'to>(
        &'to self,
        to: &'to Self,
    ) -> ChangedIterator<'to, T, impl Iterator<Item = &'to String>> {
        let name_iter = self.backing.keys();
        ChangedIterator {
            iter: name_iter,
            backing_from: &self.backing,
            backing_to: &to.backing,
        }
    }
}

impl<T: PartialEq + Eq + Hash> From<Vec<GenericFact<T>>> for FactSet<T> {
    fn from(f: Vec<GenericFact<T>>) -> Self {
        let backing: HashMap<String, Rc<GenericFact<T>>> = f
            .into_iter()
            .map(|fact| (fact.name.clone(), Rc::new(fact)))
            .collect();
        let name_set = backing.keys().cloned().collect();
        Self { backing, name_set }
    }
}

#[cfg(test)]
mod fact_set_tests {
    use super::*;

    type FactTest = GenericFact<u16>;

    fn make_set_a() -> Vec<GenericFact<u16>> {
        vec![
            ("test/a", 0).into(),
            ("test/b", 1).into(),
            ("test/c", 3).into(),
            ("test/d", 3).into(),
            ("test/e", 3).into(),
        ]
    }
    fn make_set_b() -> Vec<GenericFact<u16>> {
        vec![
            ("test/c", 3).into(),
            ("test/d", 3).into(),
            ("test/e", 2).into(),
            ("test/f", 4).into(),
            ("test/g", 4).into(),
        ]
    }

    #[test]
    fn test_added() {
        let a: FactSet<u16> = make_set_a().into();
        let b: FactSet<u16> = make_set_b().into();
        let result: HashSet<&FactTest> = a.added_facts(&b).collect();
        assert_eq!(
            result,
            HashSet::from([&("test/f", 4u16).into(), &("test/g", 4u16).into(),])
        );
    }
    #[test]
    fn test_removed() {
        let a: FactSet<u16> = make_set_a().into();
        let b: FactSet<u16> = make_set_b().into();
        let result: HashSet<&FactTest> = a.removed_facts(&b).collect();
        assert_eq!(
            result,
            HashSet::from([&("test/a", 0).into(), &("test/b", 1).into(),])
        );
    }
    #[test]
    fn test_changed() {
        let a: FactSet<u16> = make_set_a().into();
        let b: FactSet<u16> = make_set_b().into();
        let result: HashSet<(&FactTest, &FactTest)> = a.changed_facts(&b).collect();
        assert_eq!(
            result,
            HashSet::from([(&("test/e", 3).into(), &("test/e", 2).into()),])
        );
    }
}
