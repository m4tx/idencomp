use std::fmt::{Display, Formatter};

use idencomp_macros::model;
use serde::{Deserialize, Serialize};

use crate::fastq::FastqQualityScore;
use crate::int_queue::IntQueue;
use crate::sequence::{Acid, Symbol};

/// Context "specification", as a single number.
///
/// Context specification is a limited state at a specific point in
/// (de)compressing a sequence. For instance, context specification might be a
/// list of N prior acids and quality scores that were seen just before current
/// position. `ContextSpec` is just a convenient representation of that, used
/// for rapid lookup in encoder/decoder model context table.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ContextSpec(u32);

impl ContextSpec {
    /// Constructs new `ContextSpec`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context_spec::ContextSpec;
    ///
    /// let spec = ContextSpec::new(123);
    /// assert_eq!(spec.get(), 123);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Gets the integer value for this `ContextSpec`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context_spec::ContextSpec;
    ///
    /// let spec = ContextSpec::new(456);
    /// assert_eq!(spec.get(), 456);
    /// ```
    #[inline]
    #[must_use]
    pub fn get(&self) -> u32 {
        self.0
    }
}

impl Display for ContextSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Debug)]
pub struct GenericContextSpec<
    const ACID_ORDER: usize,
    const Q_SCORE_ORDER: usize,
    const POSITION_BITS: usize,
> {
    acids: [Acid; ACID_ORDER],
    q_scores: [FastqQualityScore; Q_SCORE_ORDER],
    position: u8,
}

impl<const ACID_ORDER: usize, const Q_SCORE_ORDER: usize, const POSITION_BITS: usize>
    GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>
{
    #[must_use]
    pub const fn new(
        acids: [Acid; ACID_ORDER],
        q_scores: [FastqQualityScore; Q_SCORE_ORDER],
        position: u8,
    ) -> Self {
        assert!(position < Self::max_position_value());

        Self {
            acids,
            q_scores,
            position,
        }
    }

    #[must_use]
    const fn max_position_value() -> u8 {
        1 << POSITION_BITS
    }
}

impl<const ACID_ORDER: usize, const Q_SCORE_ORDER: usize>
    GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, 0>
{
    #[must_use]
    pub const fn without_pos(
        acids: [Acid; ACID_ORDER],
        q_scores: [FastqQualityScore; Q_SCORE_ORDER],
    ) -> Self {
        Self {
            acids,
            q_scores,
            position: 0,
        }
    }
}

impl<const ACID_ORDER: usize, const Q_SCORE_ORDER: usize, const POSITION_BITS: usize>
    From<GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>> for ContextSpec
{
    fn from(repr: GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>) -> Self {
        GenericContextSpecGenerator::from_spec(&repr).current_context()
    }
}

impl<const ACID_ORDER: usize, const Q_SCORE_ORDER: usize, const POSITION_BITS: usize>
    From<&GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>> for ContextSpec
{
    fn from(repr: &GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>) -> Self {
        GenericContextSpecGenerator::from_spec(repr).current_context()
    }
}

impl<const ACID_ORDER: usize, const Q_SCORE_ORDER: usize, const POSITION_BITS: usize>
    From<ContextSpec> for GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>
{
    fn from(context_spec: ContextSpec) -> Self {
        GenericContextSpecGenerator::spec_to_repr(context_spec)
    }
}

impl<const ACID_ORDER: usize, const Q_SCORE_ORDER: usize, const POSITION_BITS: usize> Display
    for GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for acid in self.acids {
            write!(f, "{}", acid)?;
        }
        write!(f, ", ")?;
        for q_score in self.q_scores {
            write!(f, "{}", q_score)?;
        }
        write!(f, ", ")?;
        write!(f, "{}/{}", self.position, Self::max_position_value())?;

        Ok(())
    }
}

pub trait ContextSpecGenerator {
    #[must_use]
    fn current_context(&self) -> ContextSpec;

    fn update(&mut self, acid: Acid, q_score: FastqQualityScore);
}

#[derive(Debug)]
pub struct GenericContextSpecGenerator<
    const ACID_ORDER: usize,
    const Q_SCORE_ORDER: usize,
    const POSITION_BITS: usize,
> {
    acid_context: IntQueue<5, ACID_ORDER>,
    q_score_context: IntQueue<94, Q_SCORE_ORDER>,
    position: usize,
    length: usize,
}

impl<const ACID_ORDER: usize, const Q_SCORE_ORDER: usize, const POSITION_BITS: usize>
    GenericContextSpecGenerator<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>
{
    #[must_use]
    pub fn new(length: usize) -> Self {
        debug_assert!(Self::total_bits() < 32);

        Self {
            acid_context: IntQueue::with_default(Acid::default() as u32),
            q_score_context: IntQueue::with_default(FastqQualityScore::default().get() as u32),
            position: 0,
            length,
        }
    }

    #[must_use]
    const fn total_bits() -> u32 {
        Self::acid_bits() + Self::q_score_bits() + Self::position_bits()
    }

    #[must_use]
    pub const fn spec_num() -> u32 {
        1 << Self::total_bits()
    }

    #[must_use]
    const fn acid_bits() -> u32 {
        IntQueue::<5, ACID_ORDER>::num_bits()
    }

    #[must_use]
    const fn q_score_bits() -> u32 {
        IntQueue::<94, Q_SCORE_ORDER>::num_bits()
    }

    #[must_use]
    const fn position_bits() -> u32 {
        POSITION_BITS as u32
    }

    fn push_acid(&mut self, acid: Acid) {
        self.acid_context = self.acid_context.with_pushed_back(acid as u32);
    }

    fn push_q_score(&mut self, q_score: FastqQualityScore) {
        self.q_score_context = self.q_score_context.with_pushed_back(q_score.get() as u32);
    }

    #[must_use]
    fn pop_acid(&mut self) -> Acid {
        let val = self.acid_context.back();
        self.acid_context = self.acid_context.with_popped_back();
        Acid::from_usize(val as usize)
    }

    #[must_use]
    fn pop_q_score(&mut self) -> FastqQualityScore {
        let val = self.q_score_context.back();
        self.q_score_context = self.q_score_context.with_popped_back();
        FastqQualityScore::new(val as u8)
    }

    #[inline]
    fn position(&self) -> u32 {
        self.position as u32 * Self::max_position_value() / self.length as u32
    }

    #[must_use]
    const fn max_position_value() -> u32 {
        1 << POSITION_BITS
    }

    #[must_use]
    fn from_spec(
        context_spec: &GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>,
    ) -> Self {
        let mut gen = Self::new(Self::max_position_value() as usize);
        for acid in context_spec.acids {
            gen.push_acid(acid);
        }
        for q_score in context_spec.q_scores {
            gen.push_q_score(q_score);
        }
        gen.position = context_spec.position as usize;

        gen
    }

    #[must_use]
    fn spec_to_repr(
        context: ContextSpec,
    ) -> GenericContextSpec<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS> {
        let val = context.get();
        let position = val & (Self::max_position_value() - 1);

        let val = context.get() >> POSITION_BITS;
        let acid_context = val & IntQueue::<5, ACID_ORDER>::mask();

        let val = val >> IntQueue::<5, ACID_ORDER>::num_bits();
        let q_score_context = val & IntQueue::<94, Q_SCORE_ORDER>::mask();

        let mut gen = Self {
            acid_context: IntQueue::with_state(acid_context),
            q_score_context: IntQueue::with_state(q_score_context),
            position: position as usize,
            length: Self::max_position_value() as usize,
        };

        let mut acids = [Acid::default(); ACID_ORDER];
        let mut q_scores = [FastqQualityScore::default(); Q_SCORE_ORDER];
        for acid in &mut acids {
            *acid = gen.pop_acid();
        }
        for q_score in &mut q_scores {
            *q_score = gen.pop_q_score();
        }
        acids.reverse();
        q_scores.reverse();

        GenericContextSpec::new(acids, q_scores, position as u8)
    }
}

impl<const ACID_ORDER: usize, const Q_SCORE_ORDER: usize, const POSITION_BITS: usize>
    ContextSpecGenerator for GenericContextSpecGenerator<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS>
{
    fn current_context(&self) -> ContextSpec {
        let mut val = self.q_score_context.get();
        val = (val << Self::acid_bits()) | self.acid_context.get();
        val = (val << POSITION_BITS) | self.position();

        ContextSpec::new(val)
    }

    fn update(&mut self, acid: Acid, q_score: FastqQualityScore) {
        self.push_acid(acid);
        self.push_q_score(q_score);
        self.position += 1;
    }
}

#[derive(Debug)]
pub struct LightContextSpecGenerator<
    const ACID_ORDER: usize,
    const Q_SCORE_ORDER: usize,
    const POSITION_BITS: usize,
    const Q_SCORE_MAX: u32,
> {
    acid_context: IntQueue<4, ACID_ORDER>,
    q_score_context: IntQueue<Q_SCORE_MAX, Q_SCORE_ORDER>,
    position: usize,
    length: usize,
}

impl<
        const ACID_ORDER: usize,
        const Q_SCORE_ORDER: usize,
        const POSITION_BITS: usize,
        const Q_SCORE_MAX: u32,
    > LightContextSpecGenerator<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS, Q_SCORE_MAX>
{
    #[must_use]
    pub fn new(length: usize) -> Self {
        debug_assert!(Self::total_bits() < 32);

        Self {
            acid_context: IntQueue::with_default(0),
            q_score_context: IntQueue::with_default(0),
            position: 0,
            length,
        }
    }

    #[must_use]
    const fn total_bits() -> u32 {
        Self::acid_bits() + Self::q_score_bits() + Self::position_bits()
    }

    #[must_use]
    pub const fn spec_num() -> u32 {
        1 << Self::total_bits()
    }

    #[must_use]
    const fn acid_bits() -> u32 {
        IntQueue::<4, ACID_ORDER>::num_bits()
    }

    #[must_use]
    const fn q_score_bits() -> u32 {
        IntQueue::<Q_SCORE_MAX, Q_SCORE_ORDER>::num_bits()
    }

    #[must_use]
    const fn max_q_score_value() -> u32 {
        Q_SCORE_MAX
    }

    #[must_use]
    const fn position_bits() -> u32 {
        POSITION_BITS as u32
    }

    fn push_acid(&mut self, acid: u32) {
        self.acid_context = self.acid_context.with_pushed_back(acid);
    }

    fn push_q_score(&mut self, q_score: u32) {
        self.q_score_context = self.q_score_context.with_pushed_back(q_score);
    }

    #[inline]
    fn position(&self) -> u32 {
        self.position as u32 * Self::max_position_value() / self.length as u32
    }

    #[must_use]
    const fn max_position_value() -> u32 {
        1 << POSITION_BITS
    }
}

impl<
        const ACID_ORDER: usize,
        const Q_SCORE_ORDER: usize,
        const POSITION_BITS: usize,
        const Q_SCORE_MAX: u32,
    > ContextSpecGenerator
    for LightContextSpecGenerator<ACID_ORDER, Q_SCORE_ORDER, POSITION_BITS, Q_SCORE_MAX>
{
    fn current_context(&self) -> ContextSpec {
        let mut val = self.q_score_context.get();
        val = (val << Self::acid_bits()) | self.acid_context.get();
        val = (val << POSITION_BITS) | self.position();

        ContextSpec::new(val)
    }

    fn update(&mut self, acid: Acid, q_score: FastqQualityScore) {
        let (acid, q_score) = if acid == Acid::N || q_score == FastqQualityScore::ZERO {
            (0, 0)
        } else {
            (
                acid.to_usize() - 1,
                q_score.get() * Self::max_q_score_value() as usize / FastqQualityScore::SIZE,
            )
        };

        self.push_acid(acid as u32);
        self.push_q_score(q_score as u32);
        self.position += 1;
    }
}

model! {
    // # Dummy
    dummy(),
    // # Generic
    // ## Acids
    generic(1, 0, 0),
    generic(2, 0, 0),
    generic(4, 0, 0),
    generic(8, 0, 0),
    // ## Quality Scores
    generic(0, 1, 0),
    generic(0, 2, 0),
    generic(0, 3, 0),
    // ## Positions
    generic(0, 0, 2),
    generic(0, 0, 4),
    generic(0, 0, 8),
    // ## Middle
    generic(4, 1, 2),
    generic(1, 3, 2),
    generic(2, 1, 6),
    // ## Acids & Quality Scores
    generic(6, 2, 0),
    generic(3, 3, 0),
    // ## Acids & Positions
    generic(8, 0, 4),
    generic(4, 0, 3),
    generic(4, 0, 6),
    // ## Quality Scores & Positions
    generic(0, 2, 6),
    generic(0, 3, 3),
    // ## Big
    generic(4, 2, 6),
    generic(5, 2, 4),
    generic(3, 3, 4),
    // # Light
    // ## Acids
    light(4, 1, 2, 16),
    light(8, 1, 2, 16),
    light(8, 0, 0, 1),
    // ## Quality Scores
    light(0, 3, 3, 8),
    light(0, 3, 3, 16),
    light(0, 4, 3, 8),
    light(0, 4, 3, 16),
    light(0, 4, 0, 8),
    light(0, 4, 0, 16),
    light(3, 3, 0, 8),
    light(3, 3, 0, 16),
    light(2, 3, 2, 8),
    light(0, 4, 2, 8),
    light(2, 3, 2, 16),
    light(0, 4, 2, 16),
    // ## Middle
    light(2, 4, 2, 8),
    light(4, 3, 4, 16),
    light(4, 3, 2, 8),
    // ## Different Q Score precision
    light(0, 3, 0, 4),
    light(0, 3, 0, 8),
    light(0, 3, 0, 16),
    light(0, 3, 0, 32),
    // ## Big
    light(4, 4, 4, 8),
    light(4, 4, 4, 16),
    light(5, 4, 4, 16),
    light(3, 5, 4, 16),
}

#[cfg(test)]
mod tests {
    use crate::context_spec::{
        ContextSpec, ContextSpecGenerator, GenericContextSpec, GenericContextSpecGenerator,
        LightContextSpecGenerator,
    };
    use crate::fastq::FastqQualityScore;
    use crate::sequence::Acid;

    #[test]
    fn test_context_spec_display() {
        let spec = ContextSpec::new(21_374_269);
        assert_eq!(spec.to_string(), "0146253D");
    }

    #[test]
    fn test_index() {
        let spec_1 = GenericContextSpec::without_pos(
            [Acid::C, Acid::G, Acid::A, Acid::T],
            [FastqQualityScore::new(35), FastqQualityScore::new(42)],
        );
        let context = ContextSpec::from(&spec_1);
        let spec_2 = GenericContextSpec::<4, 2, 0>::from(context);

        assert_eq!(spec_1, spec_2);
    }

    #[test]
    fn test_display_generic_context_spec() {
        let context_spec = GenericContextSpec::<5, 3, 2>::new(
            [Acid::A, Acid::C, Acid::G, Acid::T, Acid::N],
            [
                FastqQualityScore::new(0),
                FastqQualityScore::new(15),
                FastqQualityScore::new(93),
            ],
            3,
        );

        assert_eq!(format!("{}", context_spec), "ACGTN, !0~, 3/4");
    }

    #[test]
    fn test_context_spec_dummy() {
        let generic_spec = GenericContextSpec::<0, 0, 0>::new([], [], 0);

        let spec = ContextSpec::from(generic_spec);

        assert_eq!(spec, ContextSpec::new(0));
    }

    #[test]
    fn test_context_spec_generic_no_pos() {
        let generic_spec =
            GenericContextSpec::without_pos([Acid::C, Acid::G], [FastqQualityScore::new(92)]);

        let spec = ContextSpec::from(generic_spec);

        assert_eq!(spec, ContextSpec::new(0xB8E));
    }

    #[test]
    fn test_context_spec_generic() {
        let generic_spec =
            GenericContextSpec::<2, 1, 3>::new([Acid::C, Acid::G], [FastqQualityScore::new(92)], 5);

        let spec = ContextSpec::from(generic_spec);

        assert_eq!(spec, ContextSpec::new(0x5C75));
    }

    #[test]
    fn test_generator_position() {
        let mut generator = GenericContextSpecGenerator::<0, 0, 2>::new(7);

        assert_eq!(generator.current_context(), ContextSpec::new(0));
        generator.update(Acid::default(), FastqQualityScore::default());
        assert_eq!(generator.current_context(), ContextSpec::new(0));
        generator.update(Acid::default(), FastqQualityScore::default());
        assert_eq!(generator.current_context(), ContextSpec::new(1));
        generator.update(Acid::default(), FastqQualityScore::default());
        assert_eq!(generator.current_context(), ContextSpec::new(1));
        generator.update(Acid::default(), FastqQualityScore::default());
        assert_eq!(generator.current_context(), ContextSpec::new(2));
        generator.update(Acid::default(), FastqQualityScore::default());
        assert_eq!(generator.current_context(), ContextSpec::new(2));
        generator.update(Acid::default(), FastqQualityScore::default());
        assert_eq!(generator.current_context(), ContextSpec::new(3));
    }

    #[test]
    fn test_generator_spec_num() {
        assert_eq!(GenericContextSpecGenerator::<1, 0, 0>::spec_num(), 8);
    }

    #[test]
    fn test_light_context_spec_generator() {
        let mut generator = LightContextSpecGenerator::<2, 2, 4, 16>::new(8);
        assert_eq!(generator.current_context(), ContextSpec::new(0x00000000));

        generator.update(Acid::A, FastqQualityScore::new(0));
        assert_eq!(generator.current_context(), ContextSpec::new(0x00000002));

        generator.update(Acid::N, FastqQualityScore::new(0));
        assert_eq!(generator.current_context(), ContextSpec::new(0x00000004));

        generator.update(Acid::A, FastqQualityScore::new(93));
        assert_eq!(generator.current_context(), ContextSpec::new(0x00000F06));

        generator.update(Acid::A, FastqQualityScore::new(93));
        assert_eq!(generator.current_context(), ContextSpec::new(0x0000FF08));

        generator.update(Acid::C, FastqQualityScore::new(93));
        assert_eq!(generator.current_context(), ContextSpec::new(0x0000FF1A));

        generator.update(Acid::C, FastqQualityScore::new(93));
        assert_eq!(generator.current_context(), ContextSpec::new(0x0000FF5C));
    }
}
