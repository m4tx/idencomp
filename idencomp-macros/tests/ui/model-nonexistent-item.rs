#![allow(unused_imports)]

use idencomp_macros::model;
use serde::{Deserialize, Serialize};

pub trait ContextSpecGeneratorTrait {}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct GenericContextSpec<const A: usize, const B: usize>;

#[derive(Debug)]
pub struct GenericContextSpecGenerator<const A: usize, const B: usize>;

impl<const A: usize, const B: usize> GenericContextSpecGenerator<A, B> {
    pub fn new() -> Self {
        unimplemented!()
    }
}

impl<const A: usize, const B: usize> ContextSpecGeneratorTrait
    for GenericContextSpecGenerator<A, B>
{
}

model! {
    generic(1, 0, 0),
    nonexistent(4, 0, 0),
}

fn main() {}
