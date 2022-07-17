use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::mem;

use derive_more::Deref;
use serde::{Deserialize, Serialize};

use crate::fastq::{FastqQualityScore, FASTQ_QUALITY_SCORE_CHARS};
use crate::progress::ByteNum;

pub trait Symbol: PartialEq + Eq + Hash + Copy {
    const SIZE: usize;

    fn to_usize(&self) -> usize;
    fn from_usize(value: usize) -> Self;

    fn values() -> Vec<Self> {
        (0..Self::SIZE)
            .into_iter()
            .map(|value| Self::from_usize(value))
            .collect()
    }
}

/// Identifier (title/name) of a nucleotide sequence.
#[derive(Debug, Eq, PartialEq, Hash, Clone, Default)]
pub struct NucleotideSequenceIdentifier(pub String);

impl NucleotideSequenceIdentifier {
    /// Empty identifier.
    pub const EMPTY: NucleotideSequenceIdentifier = NucleotideSequenceIdentifier(String::new());

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns this identifier as string.
    #[inline]
    #[must_use]
    pub fn str(&self) -> &str {
        &self.0
    }
}

impl Display for NucleotideSequenceIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for NucleotideSequenceIdentifier {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for NucleotideSequenceIdentifier {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Nucleotide sequence, containing both acids and the corresponding quality
/// scores.
#[derive(Clone, Debug, Eq)]
pub struct NucleotideSequence<const Q_END: usize> {
    identifier: NucleotideSequenceIdentifier,
    acids: Vec<Acid>,
    quality_scores: Vec<QualityScore<Q_END>>,
    size: ByteNum,
}

impl<const Q_END: usize> NucleotideSequence<Q_END> {
    /// Creates a new instance of `NucleotideSequence`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::sequence::{Acid, NucleotideSequence, QualityScore};
    ///
    /// let seq: NucleotideSequence<20> = NucleotideSequence::new(
    ///     "SEQ_1",
    ///     [Acid::A, Acid::C, Acid::G],
    ///     [
    ///         QualityScore::<20>::new(5),
    ///         QualityScore::<20>::new(10),
    ///         QualityScore::<20>::new(15),
    ///     ],
    /// );
    /// ```
    ///
    /// # Panics
    /// This function panics if the number of acids is not equal to the number
    /// of quality scores.
    #[must_use]
    pub fn new<T, U, V>(identifier: T, acids: U, quality_scores: V) -> Self
    where
        T: Into<NucleotideSequenceIdentifier>,
        U: Into<Vec<Acid>>,
        V: Into<Vec<QualityScore<Q_END>>>,
    {
        let identifier = identifier.into();
        let acids = acids.into();
        let quality_scores = quality_scores.into();

        const FASTQ_BOILERPLATE_LEN: usize = "@\n\n+\n\n".len();
        let approximate_size =
            identifier.len() + acids.len() + quality_scores.len() + FASTQ_BOILERPLATE_LEN;

        Self::with_size(
            identifier,
            acids,
            quality_scores,
            ByteNum::new(approximate_size),
        )
    }

    #[must_use]
    pub fn with_size<T, U, V>(identifier: T, acids: U, quality_scores: V, size: ByteNum) -> Self
    where
        T: Into<NucleotideSequenceIdentifier>,
        U: Into<Vec<Acid>>,
        V: Into<Vec<QualityScore<Q_END>>>,
    {
        let acids = acids.into();
        let quality_scores = quality_scores.into();
        assert_eq!(acids.len(), quality_scores.len());

        Self {
            identifier: identifier.into(),
            acids,
            quality_scores,
            size,
        }
    }

    /// Returns the identifier of this sequence.
    ///
    /// # Examples
    /// ```
    /// use idencomp::sequence::{
    ///     Acid, NucleotideSequence, NucleotideSequenceIdentifier, QualityScore,
    /// };
    ///
    /// let seq: NucleotideSequence<20> = NucleotideSequence::new("SEQ_1", [], []);
    /// assert_eq!(
    ///     seq.identifier(),
    ///     &NucleotideSequenceIdentifier::from("SEQ_1")
    /// );
    /// ```
    #[must_use]
    pub fn identifier(&self) -> &NucleotideSequenceIdentifier {
        &self.identifier
    }

    /// Returns the list of acids of this sequence.
    ///
    /// # Examples
    /// ```
    /// use idencomp::sequence::{
    ///     Acid, NucleotideSequence, NucleotideSequenceIdentifier, QualityScore,
    /// };
    ///
    /// let seq: NucleotideSequence<20> =
    ///     NucleotideSequence::new("", [Acid::A], [QualityScore::default()]);
    /// assert_eq!(seq.acids(), &[Acid::A]);
    /// ```
    #[must_use]
    pub fn acids(&self) -> &[Acid] {
        &self.acids
    }

    /// Returns the list of quality scores of this sequence.
    ///
    /// # Examples
    /// ```
    /// use idencomp::sequence::{
    ///     Acid, NucleotideSequence, NucleotideSequenceIdentifier, QualityScore,
    /// };
    ///
    /// let seq: NucleotideSequence<20> =
    ///     NucleotideSequence::new("", [Acid::A], [QualityScore::new(5)]);
    /// assert_eq!(seq.quality_scores(), &[QualityScore::new(5)]);
    /// ```
    #[must_use]
    pub fn quality_scores(&self) -> &[QualityScore<Q_END>] {
        &self.quality_scores
    }

    /// Returns a new instance of `NucleotideSequence`, identical as `self`, but
    /// with an empty identifier.
    #[must_use]
    pub fn with_identifier_discarded(self) -> Self {
        Self::with_size(
            NucleotideSequenceIdentifier::EMPTY,
            self.acids,
            self.quality_scores,
            self.size,
        )
    }

    /// Returns a new instance of `NucleotideSequence`, identical as `self`, but
    /// with given identifier.
    #[must_use]
    pub fn with_identifier<T>(self, identifier: T) -> Self
    where
        T: Into<NucleotideSequenceIdentifier>,
    {
        Self::new(identifier, self.acids, self.quality_scores)
    }

    /// Consumes this sequence and returns a vector of acids and quality scores.
    #[must_use]
    pub fn into_data(self) -> (Vec<Acid>, Vec<QualityScore<Q_END>>) {
        (self.acids, self.quality_scores)
    }

    /// Returns the length (i.e. number of acids/quality scores) of the
    /// sequence.
    ///
    /// # Examples
    /// ```
    /// use idencomp::sequence::{
    ///     Acid, NucleotideSequence, NucleotideSequenceIdentifier, QualityScore,
    /// };
    ///
    /// let seq: NucleotideSequence<20> =
    ///     NucleotideSequence::new("", [Acid::A], [QualityScore::default()]);
    /// assert_eq!(seq.len(), 1);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.acids.len()
    }

    #[must_use]
    pub fn size(&self) -> ByteNum {
        self.size
    }

    /// Returns `true` if the sequence contains no acids/quality scores.
    ///
    /// # Examples
    /// ```
    /// use idencomp::sequence::{
    ///     Acid, NucleotideSequence, NucleotideSequenceIdentifier, QualityScore,
    /// };
    ///
    /// let seq: NucleotideSequence<20> = NucleotideSequence::new("", [], []);
    /// assert_eq!(seq.is_empty(), true);
    /// let seq: NucleotideSequence<20> =
    ///     NucleotideSequence::new("", [Acid::A], [QualityScore::new(5)]);
    /// assert_eq!(seq.is_empty(), false);
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.acids.is_empty()
    }
}

impl<const Q_END: usize> PartialEq for NucleotideSequence<Q_END> {
    fn eq(&self, other: &Self) -> bool {
        if self.identifier != other.identifier {
            return false;
        }
        if self.acids != other.acids {
            return false;
        }
        if self.quality_scores != other.quality_scores {
            return false;
        }
        true
    }
}

impl<const Q_END: usize> Hash for NucleotideSequence<Q_END> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.identifier.0.as_bytes());

        let acids = self.acids.as_slice();
        let acids: &[u8] = unsafe { mem::transmute(acids) };
        state.write(acids);

        let q_scores = self.quality_scores.as_slice();
        let q_scores: &[u8] = unsafe { mem::transmute(q_scores) };
        state.write(q_scores);
    }
}

/// Nucleic acid.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum Acid {
    #[default]
    /// Invalid nucleic acid.
    N,
    /// Adenine.
    A,
    /// Cytosine.
    C,
    /// Thymine.
    T,
    /// Guanine.
    G,
}

impl Symbol for Acid {
    const SIZE: usize = 5;

    #[inline]
    fn to_usize(&self) -> usize {
        *self as usize
    }

    #[inline]
    fn from_usize(value: usize) -> Self {
        match value {
            0 => Acid::N,
            1 => Acid::A,
            2 => Acid::C,
            3 => Acid::T,
            4 => Acid::G,
            _ => unimplemented!(),
        }
    }
}

impl Display for Acid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Acid::A => 'A',
            Acid::C => 'C',
            Acid::G => 'G',
            Acid::T => 'T',
            Acid::N => 'N',
        };

        write!(f, "{}", str)
    }
}

/// Quality score (how certain a specific read is) for a read.
#[derive(Deref, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy)]
#[repr(transparent)]
pub struct QualityScore<const Q_END: usize>(u8);

impl<const Q_END: usize> QualityScore<Q_END> {
    pub const ZERO: QualityScore<Q_END> = Self(0);

    /// Constructs a new QualityScore instance.
    ///
    /// # Panics
    /// This function panics if `value` > `Q_END`.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        assert!((value as usize) < Q_END);

        Self(value)
    }

    /// Return the integer value of this `QualityScore` instance.
    #[must_use]
    pub fn get(&self) -> usize {
        self.0 as usize
    }
}

impl FastqQualityScore {
    #[must_use]
    pub fn as_fastq_char(&self) -> char {
        FASTQ_QUALITY_SCORE_CHARS
            .clone()
            .nth(self.get())
            .expect("Quality score not valid")
    }
}

impl Display for FastqQualityScore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_fastq_char())
    }
}

impl<const Q_END: usize> From<u8> for QualityScore<Q_END> {
    fn from(value: u8) -> Self {
        Self::new(value)
    }
}

impl<const Q_END: usize> Symbol for QualityScore<Q_END> {
    const SIZE: usize = Q_END;

    #[inline]
    fn to_usize(&self) -> usize {
        self.get() as usize
    }

    #[inline]
    fn from_usize(value: usize) -> Self {
        Self::new(value as u8)
    }
}

#[cfg(test)]
mod tests {
    use crate::fastq::FastqQualityScore;
    use crate::sequence::{
        Acid, NucleotideSequence, NucleotideSequenceIdentifier, QualityScore, Symbol,
    };

    #[test]
    fn test_sequence_new() {
        let identifier = "TEST";
        let acids = [Acid::A, Acid::G];
        let q_scores = [QualityScore::<10>::new(0), QualityScore::<10>::new(1)];

        let seq = NucleotideSequence::new(identifier, acids, q_scores);

        assert_eq!(
            seq.identifier(),
            &NucleotideSequenceIdentifier::from(identifier)
        );
        assert_eq!(seq.acids(), acids);
        assert_eq!(seq.quality_scores(), q_scores);
        assert_eq!(seq.len(), 2);
        let (ret_acids, ret_q_scores) = seq.into_data();
        assert_eq!(acids.as_slice(), ret_acids);
        assert_eq!(q_scores.as_slice(), ret_q_scores);
    }

    #[test]
    fn test_sequence_identifier_modification() {
        let identifier = "TEST";
        let acids = [Acid::A, Acid::G];
        let q_scores = [QualityScore::<10>::new(0), QualityScore::<10>::new(1)];

        let seq_1 = NucleotideSequence::new(identifier, acids, q_scores);
        let seq_2 = NucleotideSequence::new("", acids, q_scores);

        assert_eq!(seq_1.clone().with_identifier_discarded(), seq_2);
        assert_eq!(seq_2.with_identifier(identifier), seq_1);
    }

    #[test]
    fn test_acid_display() {
        assert_eq!(format!("{}", Acid::A), "A");
        assert_eq!(format!("{}", Acid::C), "C");
        assert_eq!(format!("{}", Acid::T), "T");
        assert_eq!(format!("{}", Acid::G), "G");
        assert_eq!(format!("{}", Acid::N), "N");
    }

    #[test]
    fn test_q_score_str() {
        let s = "!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
        for i in 0..94 {
            let q_score = FastqQualityScore::new(i as u8);
            assert_eq!(q_score.as_fastq_char(), s.chars().nth(i).unwrap());
            assert_eq!(format!("{}", q_score), s[i..i + 1]);
        }
    }

    #[test]
    fn test_q_score_symbol() {
        let q_score = QualityScore::<10>::new(5);
        assert_eq!(q_score.to_usize(), 5);

        let q_score = QualityScore::<10>::from_usize(7);
        assert_eq!(q_score, QualityScore::<10>::new(7));
    }
}
