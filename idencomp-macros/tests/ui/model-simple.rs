use idencomp_macros::model;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Acid;

#[derive(Debug)]
pub struct FastqQualityScore;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct GenericContextSpec<const A: usize, const B: usize, const C: usize>;

impl<const A: usize, const B: usize, const C: usize> GenericContextSpec<A, B, C> {
    pub fn new(_acids: [Acid; A], _q_scores: [FastqQualityScore; B]) -> Self {
        unimplemented!()
    }
}

pub trait ContextSpecGenerator {}

#[derive(Debug)]
pub struct GenericContextSpecGenerator<const A: usize, const B: usize, const C: usize>;

impl<const A: usize, const B: usize, const C: usize> GenericContextSpecGenerator<A, B, C> {
    pub fn new(_length: usize) -> Self {
        unimplemented!()
    }

    pub fn spec_num() -> u32 {
        unimplemented!()
    }
}

impl<const A: usize, const B: usize, const C: usize> ContextSpecGenerator
    for GenericContextSpecGenerator<A, B, C>
{
}

model! {
    generic(1, 0, 0),
    generic(4, 0, 1),
    generic(0, 1, 2),
}

fn main() {
    let _ctx_type = ContextSpecType::Generic4Acids0QScores1PosBits;
}
