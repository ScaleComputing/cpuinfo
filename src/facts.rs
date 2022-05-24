//! Provide a means to work with and diff sets of facts
//!

use serde::{Deserialize, Serialize};
use std::cmp::Eq;
use std::fmt::{Display, Formatter};
use std::hash::Hash;

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq)]
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
    pub fn add_path<'a>(&mut self, path: &'a str) {
        self.name.insert_str(0, path);
        self.name.insert(0, '/');
    }
}

impl<T: Display> Display for GenericFact<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{} = {}", self.name, self.value)
    }
}

pub trait Facter<T> {
    fn collect_facts(&self) -> Vec<T>;
}
