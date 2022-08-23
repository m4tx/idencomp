use std::collections::HashMap;
use std::marker::PhantomData;

use crate::context::{Context, Probability};
use crate::context_binning::ComplexContext;
use crate::context_spec::ContextSpec;
use crate::sequence::Symbol;

/// An object that helps generating statistic models out of nucleotide
/// sequences.
#[derive(Debug)]
pub struct ModelGenerator<T> {
    map: HashMap<ContextSpec, ContextCounter<T>>,
    count: usize,
}

impl<T: Symbol> ModelGenerator<T> {
    /// Creates a new `ModelGenerator` instance.
    ///
    /// # Example
    /// ```
    /// use idencomp::context_spec::ContextSpec;
    /// use idencomp::model_generator::ModelGenerator;
    /// use idencomp::sequence::Acid;
    ///
    /// let mut generator = ModelGenerator::<Acid>::new();
    /// generator.add(ContextSpec::new(123), Acid::A);
    /// let _contexts = generator.complex_contexts();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            count: 0,
        }
    }

    /// Adds a new value associated with a context specifier.
    ///
    /// # Example
    /// ```
    /// use idencomp::context_spec::ContextSpec;
    /// use idencomp::model_generator::ModelGenerator;
    /// use idencomp::sequence::Acid;
    ///
    /// let mut generator = ModelGenerator::<Acid>::new();
    /// generator.add(ContextSpec::new(123), Acid::A);
    /// assert_eq!(generator.len(), 1);
    /// ```
    pub fn add(&mut self, context_spec: ContextSpec, value: T) {
        self.map
            .entry(context_spec)
            .or_insert_with(|| ContextCounter::new())
            .add(value);
        self.count += 1;
    }

    /// Returns the number of distinct context specifiers encountered so far.
    ///
    /// # Example
    /// ```
    /// use idencomp::context_spec::ContextSpec;
    /// use idencomp::model_generator::ModelGenerator;
    /// use idencomp::sequence::Acid;
    ///
    /// let mut generator = ModelGenerator::<Acid>::new();
    /// generator.add(ContextSpec::new(123), Acid::A);
    /// generator.add(ContextSpec::new(123), Acid::G);
    /// generator.add(ContextSpec::new(423), Acid::A);
    /// assert_eq!(generator.len(), 2);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns whether nothing has been added to this `ModelGenerator`.
    ///
    /// # Example
    /// ```
    /// use idencomp::context_spec::ContextSpec;
    /// use idencomp::model_generator::ModelGenerator;
    /// use idencomp::sequence::Acid;
    ///
    /// let mut generator = ModelGenerator::<Acid>::new();
    /// assert_eq!(generator.is_empty(), true);
    /// generator.add(ContextSpec::new(123), Acid::A);
    /// assert_eq!(generator.is_empty(), false);
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the list of [`ComplexContext`] instances, which then can be used
    /// to create a model.
    ///
    /// # Example
    /// ```
    /// use idencomp::context::Context;
    /// use idencomp::context_binning::ComplexContext;
    /// use idencomp::context_spec::ContextSpec;
    /// use idencomp::model_generator::ModelGenerator;
    /// use idencomp::sequence::Acid;
    ///
    /// let mut generator = ModelGenerator::<Acid>::new();
    /// generator.add(ContextSpec::new(123), Acid::A);
    /// let contexts = generator.complex_contexts();
    /// assert_eq!(contexts.len(), 1);
    /// assert_eq!(
    ///     contexts[0],
    ///     ComplexContext::with_single_spec(
    ///         ContextSpec::new(123),
    ///         Context::new_from(1.0, [0.0, 1.0, 0.0, 0.0, 0.0])
    ///     )
    /// );
    /// ```
    #[must_use]
    pub fn complex_contexts(&self) -> Vec<ComplexContext> {
        self.map
            .keys()
            .map(|&key| ComplexContext::with_single_spec(key, self.context(key)))
            .collect()
    }

    #[must_use]
    fn context(&self, spec: ContextSpec) -> Context {
        let counter = &self.map[&spec];

        let context_prob = Probability::new(counter.count() as f32 / self.count as f32);
        let symbol_prob: Vec<Probability> = (0..T::SIZE)
            .map(|x| counter.percentage(T::from_usize(x)))
            .map(Probability::new)
            .collect();

        Context::new(context_prob, symbol_prob)
    }
}

impl<T: Symbol> Default for ModelGenerator<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// A counter for symbols. Allows to calculate percentage how often does a
/// certain symbol occur in a sequence.
#[derive(Debug)]
pub struct ContextCounter<T> {
    counts: Vec<usize>,
    _phantom: PhantomData<T>,
}

impl<T: Symbol> ContextCounter<T> {
    /// Crates a new `ContextCounter` instance.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model_generator::ContextCounter;
    /// use idencomp::sequence::Acid;
    ///
    /// let _counter = ContextCounter::<Acid>::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            counts: vec![0; T::SIZE],
            _phantom: PhantomData,
        }
    }

    /// Adds a symbol to the counter.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model_generator::ContextCounter;
    /// use idencomp::sequence::Acid;
    ///
    /// let mut counter = ContextCounter::<Acid>::new();
    /// counter.add(Acid::A);
    /// ```
    pub fn add(&mut self, value: T) {
        self.counts[value.to_usize()] += 1;
    }

    /// Gets the percentage probability of a certain symbol occurring in a
    /// sequence.
    ///
    /// # Examples
    /// ```
    /// use approx::assert_abs_diff_eq;
    /// use idencomp::model_generator::ContextCounter;
    /// use idencomp::sequence::Acid;
    ///
    /// let mut counter = ContextCounter::<Acid>::new();
    /// counter.add(Acid::A);
    /// counter.add(Acid::A);
    /// counter.add(Acid::C);
    /// assert_abs_diff_eq!(counter.percentage(Acid::A), 0.66666667);
    /// assert_abs_diff_eq!(counter.percentage(Acid::C), 0.33333333);
    /// ```
    #[must_use]
    pub fn percentage(&self, value: T) -> f32 {
        if self.count() == 0 {
            return 0.0;
        }
        self.counts[value.to_usize()] as f32 / self.count() as f32
    }

    /// Returns the total number of symbols added so far.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model_generator::ContextCounter;
    /// use idencomp::sequence::Acid;
    ///
    /// let mut counter = ContextCounter::<Acid>::new();
    /// counter.add(Acid::A);
    /// counter.add(Acid::A);
    /// counter.add(Acid::C);
    /// assert_eq!(counter.count(), 3);
    /// ```
    #[must_use]
    pub fn count(&self) -> usize {
        self.counts.iter().sum()
    }
}

impl<T: Symbol> Default for ContextCounter<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::context::Context;
    use crate::context_binning::ComplexContext;
    use crate::context_spec::ContextSpec;
    use crate::model_generator::ModelGenerator;
    use crate::sequence::Symbol;

    #[derive(Copy, Clone, PartialEq, Eq, Hash)]
    struct TestSymbol(usize);

    impl Symbol for TestSymbol {
        const SIZE: usize = 3;

        fn to_usize(&self) -> usize {
            self.0
        }

        fn from_usize(value: usize) -> Self {
            Self(value)
        }
    }

    #[test]
    fn test_model_generator() {
        let spec_1 = ContextSpec::new(0);
        let spec_2 = ContextSpec::new(1);
        let symbol_1 = TestSymbol(0);
        let symbol_2 = TestSymbol(1);
        let _symbol_3 = TestSymbol(2);

        let mut gen = ModelGenerator::<TestSymbol>::new();
        gen.add(spec_1, symbol_1);
        gen.add(spec_1, symbol_1);
        gen.add(spec_2, symbol_1);
        gen.add(spec_2, symbol_2);
        gen.add(spec_2, symbol_2);
        let mut contexts = gen.complex_contexts();
        contexts.sort();

        let ctx_1 =
            ComplexContext::with_single_spec(spec_1, Context::new_from(0.4, [1.0, 0.0, 0.0]));
        let ctx_2 = ComplexContext::with_single_spec(
            spec_2,
            Context::new_from(0.6, [0.33333334, 0.6666667, 0.0]),
        );
        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0], ctx_1);
        assert_eq!(contexts[1], ctx_2);
    }
}
