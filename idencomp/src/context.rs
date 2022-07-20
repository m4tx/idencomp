use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::ops::Add;

use derive_more::Deref;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::context_spec::ContextSpec;

/// Probability, as a float between 0.0 and 1.0
#[derive(Copy, Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Probability(f32);

impl Probability {
    /// Zero (impossible) probability
    pub const ZERO: Probability = Probability(0.0);
    /// Half (50/50) probability
    pub const HALF: Probability = Probability(0.5);
    /// One (certain) probability
    pub const ONE: Probability = Probability(1.0);
    const ZERO_THRESHOLD: Probability = Probability(1e-6);
    const EQ_THRESHOLD: Probability = Probability(1e-6);

    /// Creates a new `Probability` object.
    #[must_use]
    pub fn new(value: f32) -> Self {
        assert!(value.is_finite());
        assert!(value == 0.0 || value.is_sign_positive());
        assert!(value <= 1.0);

        Self(value)
    }

    /// Value of this Probability object, as a float
    #[must_use]
    pub fn get(&self) -> f32 {
        self.0
    }
}

impl PartialEq for Probability {
    fn eq(&self, other: &Self) -> bool {
        (self.get() - other.get()).abs() <= Self::EQ_THRESHOLD.get()
    }
}

impl Eq for Probability {}

impl From<f32> for Probability {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}

impl PartialOrd for Probability {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for Probability {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Deref, Copy, Debug, PartialEq, Clone, Default)]
#[repr(transparent)]
pub struct Entropy(f32);

impl Entropy {
    #[must_use]
    pub fn new(value: f32) -> Self {
        assert!(value.is_finite());
        assert!(value == 0.0 || value.is_sign_positive());

        Self(value)
    }
}

impl Add for Entropy {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(*self + *rhs)
    }
}

impl From<f32> for Entropy {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}

#[derive(Deref, Copy, Debug, Clone, Default)]
#[repr(transparent)]
pub struct ContextMergeCost(f32);

impl ContextMergeCost {
    pub const ZERO: ContextMergeCost = ContextMergeCost(0.0);
    const EQ_THRESHOLD: ContextMergeCost = ContextMergeCost(1e-6);

    #[must_use]
    pub fn new(value: f32) -> Self {
        assert!(value.is_finite());

        Self(value)
    }
}

impl Display for ContextMergeCost {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<f32> for ContextMergeCost {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}

impl PartialEq for ContextMergeCost {
    fn eq(&self, other: &Self) -> bool {
        (**self - **other).abs() <= *ContextMergeCost::EQ_THRESHOLD
    }
}

impl Eq for ContextMergeCost {}

impl PartialOrd for ContextMergeCost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for ContextMergeCost {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    pub context_prob: Probability,
    pub symbol_prob: Vec<Probability>,

    entropy: Entropy,
}

impl Context {
    #[must_use]
    pub fn new<U: Into<Vec<Probability>>>(context_prob: Probability, symbol_prob: U) -> Self {
        let symbol_prob = symbol_prob.into();
        let entropy = Self::calc_entropy(&symbol_prob);

        Self {
            context_prob,
            symbol_prob,
            entropy,
        }
    }

    #[must_use]
    pub fn new_from<T: Into<Probability>, U: Into<Probability>, I>(
        context_prob: T,
        symbol_prob: I,
    ) -> Self
    where
        I: IntoIterator<Item = U>,
    {
        Self::new(
            context_prob.into(),
            symbol_prob
                .into_iter()
                .map(|x| x.into())
                .collect::<Vec<Probability>>(),
        )
    }

    #[inline]
    #[must_use]
    pub fn dummy(num_symbols: usize) -> Self {
        let mut symbol_prob = Vec::new();
        symbol_prob.resize(num_symbols, Probability::new(1.0 / num_symbols as f32));
        Self::new(Probability::ONE, symbol_prob)
    }

    #[must_use]
    pub fn symbol_num(&self) -> usize {
        self.symbol_prob.len()
    }

    #[must_use]
    pub fn merge_with(&self, other: &Self) -> Self {
        assert_eq!(self.symbol_num(), other.symbol_num());

        let context_prob_val = self.context_prob.get() + other.context_prob.get();
        let context_prob = Probability::new(context_prob_val.min(1.0));
        let symbol_prob: Vec<Probability> = self
            .symbol_prob
            .iter()
            .zip(other.symbol_prob.iter())
            .map(|(&x, &y)| {
                let prob = (self.context_prob.get() * x.get() + other.context_prob.get() * y.get())
                    / context_prob.get();
                if prob.is_nan() {
                    Probability::new(0.0)
                } else {
                    Probability::new(prob.min(1.0))
                }
            })
            .collect();

        Self::new(context_prob, symbol_prob)
    }

    #[must_use]
    pub fn entropy(&self) -> Entropy {
        self.entropy
    }

    #[must_use]
    fn calc_entropy(symbol_prob: &[Probability]) -> Entropy {
        symbol_prob
            .iter()
            .filter(|&&x| x >= Probability::ZERO_THRESHOLD)
            .map(|&x| Entropy::new(-x.get() * x.get().log2()))
            .reduce(|x, y| x + y)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn merge_cost(merged: &Self, left: &Self, right: &Self) -> ContextMergeCost {
        let cost: f32 = merged.context_prob.get() * *merged.entropy()
            - (left.context_prob.get() * *left.entropy()
                + right.context_prob.get() * *right.entropy());

        ContextMergeCost::new(cost)
    }

    #[must_use]
    pub fn as_integer_cum_freqs(&self, scale_bits: u8) -> Vec<u32> {
        let symbols_num = self.symbol_num();
        let total: u32 = 1 << scale_bits;
        assert!(total > symbols_num as u32);

        let mut result = self
            .symbol_prob
            .iter()
            .map(|&x| x.get() * total as f32)
            .scan(0.0_f32, |acc, x| {
                let val = *acc;
                *acc += x;
                Some(val)
            })
            .map(|x| x.round() as u32)
            .collect();

        Self::cum_freq_to_freq(&mut result, total);
        Self::fix_zero_freqs(&mut result);
        Self::freq_to_cum_freq(&mut result);

        assert!(result.iter().all_unique());
        assert!(result.last().copied().unwrap() < total);

        result
    }

    fn fix_zero_freqs(result: &mut Vec<u32>) {
        let mut zero_count = 0;
        for freq in result.iter_mut() {
            if *freq == 0 {
                *freq = 1;
                zero_count += 1;
            }
        }

        let mut i: usize = 0;
        while zero_count > 0 {
            if result[i] > 1 {
                result[i] -= 1;
                zero_count -= 1;
            }

            i += 1;
            if i >= result.len() {
                i = 0;
            }
        }
    }

    pub fn cum_freq_to_freq(cum_freq: &mut Vec<u32>, total: u32) {
        for i in 0..cum_freq.len() - 1 {
            cum_freq[i] = cum_freq[i + 1] - cum_freq[i];
        }
        let last = cum_freq.last_mut().unwrap();
        *last = total - *last;
    }

    pub fn freq_to_cum_freq(freq: &mut Vec<u32>) {
        let mut acc: u32 = 0;
        for val in freq {
            let old_val = *val;
            *val = acc;
            acc += old_val;
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new(Probability::ZERO, Vec::new())
    }
}

impl Eq for Context {}

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        self.context_prob == other.context_prob && self.symbol_prob == other.symbol_prob
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextNode {
    Leaf {
        specs: Vec<ContextSpec>,
        context: Context,
    },
    Node {
        context: Context,
        merge_cost: ContextMergeCost,
        left_child: usize,
        right_child: usize,
    },
}

impl ContextNode {
    #[must_use]
    pub(crate) fn new_from_merge(
        left: &Context,
        right: &Context,
        left_index: usize,
        right_index: usize,
    ) -> Self {
        let context = left.merge_with(right);
        let merge_cost = Context::merge_cost(&context, left, right);

        Self::new_node(context, merge_cost, left_index, right_index)
    }

    #[must_use]
    pub(crate) fn new_leaf(spec: ContextSpec, context: Context) -> Self {
        Self::Leaf {
            specs: vec![spec],
            context,
        }
    }

    #[must_use]
    pub(crate) fn new_leaf_multi<T>(specs: T, context: Context) -> Self
    where
        T: Into<Vec<ContextSpec>>,
    {
        Self::Leaf {
            specs: specs.into(),
            context,
        }
    }

    #[must_use]
    pub(crate) fn new_node(
        context: Context,
        merge_cost: ContextMergeCost,
        left_child: usize,
        right_child: usize,
    ) -> Self {
        Self::Node {
            context,
            merge_cost,
            left_child,
            right_child,
        }
    }

    #[must_use]
    pub fn is_leaf(&self) -> bool {
        match self {
            ContextNode::Leaf { .. } => true,
            ContextNode::Node { .. } => false,
        }
    }

    #[must_use]
    pub fn is_node(&self) -> bool {
        !self.is_leaf()
    }

    #[must_use]
    pub fn context(&self) -> &Context {
        match self {
            ContextNode::Leaf { context, .. } => context,
            ContextNode::Node { context, .. } => context,
        }
    }

    #[must_use]
    pub fn merge_cost(&self) -> ContextMergeCost {
        match self {
            ContextNode::Leaf { .. } => ContextMergeCost::ZERO,
            ContextNode::Node { merge_cost, .. } => *merge_cost,
        }
    }

    #[must_use]
    pub fn children(&self) -> (usize, usize) {
        match self {
            ContextNode::Leaf { .. } => panic!("called children() on a leaf node"),
            ContextNode::Node {
                left_child,
                right_child,
                ..
            } => (*left_child, *right_child),
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use crate::context::{Context, Probability};

    #[test]
    fn should_merge_contexts_with_prob_1() {
        let ctx1 = Context::new_from(1.0, [0.0, 0.5, 0.3, 0.2]);
        let ctx2 = Context::new_from(0.0, [0.5, 0.1, 0.1, 0.3]);

        let ctx_merged = ctx1.merge_with(&ctx2);

        assert_abs_diff_eq!(ctx_merged.context_prob.get(), 1.0);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[0].get(), 0.0);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[1].get(), 0.5);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[2].get(), 0.3);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[3].get(), 0.2);
    }

    #[test]
    fn merge_contexts_with_prob_0() {
        let ctx1 = Context::new_from(0.0, [0.0, 0.5, 0.3, 0.2]);
        let ctx2 = Context::new_from(0.0, [0.5, 0.1, 0.1, 0.3]);

        let ctx_merged = ctx1.merge_with(&ctx2);

        assert_abs_diff_eq!(ctx_merged.context_prob.get(), 0.0);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[0].get(), 0.0);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[1].get(), 0.0);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[2].get(), 0.0);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[3].get(), 0.0);
    }

    #[test]
    fn should_merge_identical_contexts() {
        let ctx1 = Context::new_from(0.25, [0.0, 0.5, 0.3, 0.2]);
        let ctx2 = ctx1.clone();

        let ctx_merged = ctx1.merge_with(&ctx2);

        assert_abs_diff_eq!(ctx_merged.context_prob.get(), 0.5);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[0].get(), 0.0);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[1].get(), 0.5);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[2].get(), 0.3);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[3].get(), 0.2);
    }

    #[test]
    fn should_merge_distinct_contexts() {
        let ctx1 = Context::new_from(0.75, [0.0, 0.5, 0.3, 0.2]);
        let ctx2 = Context::new_from(0.25, [0.5, 0.1, 0.1, 0.3]);

        let ctx_merged = ctx1.merge_with(&ctx2);

        assert_abs_diff_eq!(ctx_merged.context_prob.get(), 1.0);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[0].get(), 0.125);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[1].get(), 0.4);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[2].get(), 0.25);
        assert_abs_diff_eq!(ctx_merged.symbol_prob[3].get(), 0.225);
    }

    #[test]
    fn should_calculate_entropy_zero() {
        let context = Context::new(Probability::ONE, [Probability::ZERO, Probability::ONE]);

        assert_abs_diff_eq!(*context.entropy(), 0.0);
    }

    #[test]
    fn should_calculate_entropy_bit() {
        let context = Context::new(Probability::ONE, [Probability::HALF, Probability::HALF]);

        assert_abs_diff_eq!(*context.entropy(), 1.0);
    }

    #[test]
    fn should_calculate_entropy_bigger_context() {
        let context = Context::new_from(1.0, [0.25, 0.25, 0.125, 0.375]);

        assert_abs_diff_eq!(*context.entropy(), 1.905639);
    }

    #[test]
    fn context_to_cum_freq_simple() {
        let context = Context::new_from(1.0, [0.25, 0.25, 0.25, 0.25]);

        let cum_freqs = context.as_integer_cum_freqs(4);

        assert_eq!(cum_freqs, [0, 4, 8, 12]);
    }

    #[test]
    fn context_to_cum_freq_bigger() {
        let context = Context::new_from(
            1.0,
            [0.05, 0.10, 0.125, 0.125, 0.30, 0.03, 0.07, 0.05, 0.12, 0.03],
        );

        let cum_freqs = context.as_integer_cum_freqs(10);

        assert_eq!(cum_freqs, [0, 51, 154, 282, 410, 717, 748, 819, 870, 993]);
    }

    #[test]
    fn context_to_cum_freq_low_freq() {
        let context = Context::new_from(1.0, [0.01, 0.01, 0.49, 0.49]);

        let cum_freqs = context.as_integer_cum_freqs(4);

        assert_eq!(cum_freqs, [0, 1, 2, 9]);
    }
}
