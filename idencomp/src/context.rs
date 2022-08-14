use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::ops::Add;

use derive_more::Deref;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

/// Probability, as a float between 0.0 and 1.0.
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
    ///
    /// # Examples
    /// ```
    /// use idencomp::context::Probability;
    ///
    /// let prob = Probability::new(0.5);
    /// assert_eq!(prob.get(), 0.5);
    /// ```
    #[must_use]
    pub fn new(value: f32) -> Self {
        assert!(value.is_finite());
        assert!(value == 0.0 || value.is_sign_positive());
        assert!(value <= 1.0);

        Self(value)
    }

    /// Value of this `Probability` object, as a float.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context::Probability;
    ///
    /// let prob = Probability::new(0.5);
    /// assert_eq!(prob.get(), 0.5);
    /// ```
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

/// Shannon Entropy, as a non-negative float representing the number of entropy
/// bits.
///
/// # See also
/// * [Entropy on Wikipedia](https://en.wikipedia.org/wiki/Entropy_%28information_theory%29)
#[derive(Deref, Copy, Debug, PartialEq, Clone, Default)]
#[repr(transparent)]
pub struct Entropy(f32);

impl Entropy {
    /// Creates a new `Entropy` object.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context::Entropy;
    ///
    /// let entropy = Entropy::new(0.5);
    /// assert_eq!(entropy.get(), 0.5);
    /// ```
    ///
    /// # Panics
    /// This function panics if the value is negative, or is not finite.
    #[must_use]
    pub fn new(value: f32) -> Self {
        assert!(value.is_finite());
        assert!(value == 0.0 || value.is_sign_positive());

        Self(value)
    }

    /// Value of this `Entropy` object, as a float.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context::Entropy;
    ///
    /// let entropy = Entropy::new(0.5);
    /// assert_eq!(entropy.get(), 0.5);
    /// ```
    #[must_use]
    pub fn get(&self) -> f32 {
        self.0
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

/// A statistical model for a single local situation ("context").
///
/// Contains the probabilities of each symbol, and the probability of
/// encountering this specific context as well.
#[derive(Debug, Clone)]
pub struct Context {
    /// The probability of encountering this specific context in a file. Useful
    /// for context binning and not used at all during compressing/decompressing
    /// data.
    pub context_prob: Probability,
    /// The probability of encountering each symbol after given context.
    pub symbol_prob: Vec<Probability>,

    entropy: Entropy,
}

impl Context {
    /// Creates new `Context` object.
    ///
    /// ## Examples
    /// ```
    /// use idencomp::context::{Context, Probability};
    ///
    /// let context = Context::new(Probability::ONE, [Probability::ZERO, Probability::ONE]);
    /// assert_eq!(context.symbol_num(), 2);
    /// assert_eq!(context.entropy().get(), 0.0);
    /// assert_eq!(context.context_prob.get(), 1.0);
    /// assert_eq!(context.symbol_prob[1].get(), 1.0);
    /// ```
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

    /// Creates new `Context` object, converting the passed values if needed.
    ///
    /// ## Examples
    /// ```
    /// use idencomp::context::{Context, Probability};
    ///
    /// let context = Context::new_from(1.0, [0.25, 0.25, 0.25, 0.25]);
    /// assert_eq!(context.symbol_num(), 4);
    /// assert_eq!(context.entropy().get(), 2.0);
    /// assert_eq!(context.context_prob.get(), 1.0);
    /// assert_eq!(context.symbol_prob[0].get(), 0.25);
    /// ```
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

    /// Creates a new "dummy" context that have the same probability of all symbols.
    ///
    /// ## Examples
    /// ```
    /// use idencomp::context::Context;
    ///
    /// let ctx = Context::dummy(4);
    /// let expected = Context::new_from(1.0, [0.25, 0.25, 0.25, 0.25]);
    /// assert_eq!(ctx, expected);
    /// ```
    #[inline]
    #[must_use]
    pub fn dummy(num_symbols: usize) -> Self {
        let mut symbol_prob = Vec::new();
        symbol_prob.resize(num_symbols, Probability::new(1.0 / num_symbols as f32));
        Self::new(Probability::ONE, symbol_prob)
    }

    /// Returns the number of symbols for this `Context` object.
    ///
    /// ## Examples
    /// ```
    /// use idencomp::context::{Context, Probability};
    ///
    /// let context = Context::new_from(1.0, [0.25, 0.25, 0.25, 0.25]);
    /// assert_eq!(context.symbol_num(), 4);
    /// ```
    #[must_use]
    pub fn symbol_num(&self) -> usize {
        self.symbol_prob.len()
    }

    /// Merge this context with another instance.
    ///
    /// ## Examples
    /// ```
    /// use idencomp::context::Context;
    ///
    /// let ctx_1 = Context::new_from(0.25, [1.0, 0.0, 0.0, 1.0]);
    /// let ctx_2 = Context::new_from(0.75, [0.0, 0.6666667, 1.0, 1.0]);
    /// let expected_ctx = Context::new_from(1.0, [0.25, 0.5, 0.75, 1.0]);
    /// assert_eq!(ctx_1.merge_with(&ctx_2), expected_ctx);
    /// ```
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

    /// Returns the entropy of this context.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context::Context;
    ///
    /// let context = Context::new_from(1.0, [0.25, 0.25, 0.25, 0.25]);
    /// assert_eq!(context.entropy().get(), 2.0);
    /// ```
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

/// The cost of merging two [`Context`]s together, as a float.
#[derive(Deref, Copy, Debug, Clone, Default)]
#[repr(transparent)]
pub struct ContextMergeCost(f32);

impl ContextMergeCost {
    /// `ContextMergeCost` with a value of `0.0`.
    pub const ZERO: ContextMergeCost = ContextMergeCost(0.0);
    const EQ_THRESHOLD: ContextMergeCost = ContextMergeCost(1e-6);

    /// Creates a new `ContextMergeCost` object.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context::ContextMergeCost;
    ///
    /// let cost = ContextMergeCost::new(0.5);
    /// assert_eq!(*cost, 0.5);
    /// ```
    ///
    /// # Panics
    /// This function panics if the is not finite.
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
