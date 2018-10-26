use std::str::pattern::{Pattern, SearchStep, Searcher};
use std::io::prelude::*;
use std::fs::File;

use serde::{Serialize, de::DeserializeOwned};

pub fn encode<T: Serialize>(obj: &T, name: &str) -> Result<(), String> {
    let toml = toml::to_string(obj).map_err(|e| format!("toml encode failed: {}", e))?;
    let mut f = File::create(name).map_err(|x| format!("could not open {}: {}", name, x))?;
    f.write_all(toml.as_bytes()).map(|_| ()).map_err(|x| format!("could not write to {}: {}", name, x))
}

pub fn decode<T: DeserializeOwned>(name: &str) -> Result<T, String> {
    let mut f = File::open(name).map_err(|x| format!("could not open {}: {}", name, x))?;
    let mut s = String::new();
    f.read_to_string(&mut s).map_err(|x| format!("could not read {}: {}", name, x))?;

    toml::from_str(&s).map_err(|x| format!("could not parse {}: {:?}", name, x))
}

pub fn ignore<T, U>(_: Result<T, U>) {}

pub struct LengthSearcher<'a> {
    haystack: &'a str,
    split_bef: Vec<usize>,
    current_split: usize,
    is_match: bool
}

pub struct LengthPattern(pub usize);

impl<'a> Pattern<'a> for LengthPattern {
    type Searcher = LengthSearcher<'a>;

    fn into_searcher(self, haystack: &'a str) -> Self::Searcher {
        let mut split_bef = vec![];
        let mut idx = haystack.char_indices();
        idx.next();
        while let Some((idx, _)) = idx.nth(self.0 - 1) {
            split_bef.push(idx);
        }
        LengthSearcher::new(haystack, split_bef)
    }
}

impl<'a> LengthSearcher<'a> {
    pub fn new(haystack: &'a str, split_bef: Vec<usize>) -> Self {
        LengthSearcher { haystack: haystack, split_bef: split_bef, current_split: 0, is_match: false }
    }
}

unsafe impl<'a> Searcher<'a> for LengthSearcher<'a> {
    fn haystack(&self) -> &'a str {
        self.haystack
    }

    fn next(&mut self) -> SearchStep {
        if self.current_split == self.split_bef.len() {
            SearchStep::Done
        } else if self.is_match {
            self.is_match = !self.is_match;
            self.current_split += 1;
            SearchStep::Match(self.split_bef[self.current_split - 1], self.split_bef[self.current_split - 1])
        } else {
            self.is_match = !self.is_match;
            SearchStep::Reject(if self.current_split == 0 {
                                   0
                               } else {
                                   self.split_bef[self.current_split - 1]
                               },
                               self.split_bef[self.current_split])
        }
    }
}
