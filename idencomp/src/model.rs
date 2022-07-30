use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::hash::Hash;

use byteorder::{BigEndian, WriteBytesExt};
use derive_more::Deref;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

use crate::context::Context;
use crate::context_binning::ComplexContext;
use crate::context_spec::{ContextSpec, ContextSpecType};
use crate::fastq::FastqQualityScore;
use crate::sequence::{Acid, Symbol};

/// Compression rate of given model, expressed as bits per value (bpv) float.
#[derive(Deref, Copy, Debug, PartialOrd, Clone, Default)]
#[repr(transparent)]
pub struct CompressionRate(f32);

impl CompressionRate {
    /// `CompressionRate` with a value of `0.0`.
    pub const ZERO: CompressionRate = CompressionRate(0.0);

    const EQ_THRESHOLD: CompressionRate = CompressionRate(1e-6);

    /// Constructs new `CompressionRate`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model::CompressionRate;
    ///
    /// let rate = CompressionRate::new(3.45);
    /// assert_eq!(rate.to_string(), "3.4500bpv")
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

    /// Value of this `CompressionRate` object, as a float.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model::CompressionRate;
    ///
    /// let rate = CompressionRate::new(3.45);
    /// assert_eq!(rate.get(), 3.45);
    /// ```
    #[must_use]
    pub const fn get(&self) -> f32 {
        self.0
    }
}

impl PartialEq for CompressionRate {
    fn eq(&self, other: &Self) -> bool {
        (**self - **other).abs() <= *Self::EQ_THRESHOLD
    }
}

impl Eq for CompressionRate {}

impl Display for CompressionRate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}bpv", self.0)
    }
}

/// Type of model used to (de)compress genetic data (either acids, or quality
/// scores).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModelType {
    /// Type of model used to (de)compress nucleic acids.
    Acids,
    /// Type of model used to (de)compress quality scores.
    QualityScores,
}

impl Display for ModelType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelType::Acids => write!(f, "Acids"),
            ModelType::QualityScores => write!(f, "QualityScores"),
        }
    }
}

impl ModelType {
    #[must_use]
    fn symbols_num(&self) -> usize {
        match self {
            ModelType::Acids => Acid::SIZE,
            ModelType::QualityScores => FastqQualityScore::SIZE,
        }
    }
}

/// An automatically-generated identifier of a model.
///
/// The model identifier is an SHA-3 256-bit checksum of the entire model
/// contents. The identifier generation process starts with serialized by
/// storing the model type, context specifier type, model map sorted by keys
/// ascending, and then the contexts themselves. Then, the hash of such a blob
/// is calculated.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ModelIdentifier([u8; 32]);

impl ModelIdentifier {
    /// Creates a new instance of `ModelIdentifier`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model::ModelIdentifier;
    ///
    /// let identifier = ModelIdentifier::new([1; 32]);
    /// assert_eq!(identifier.to_string(), "01010101");
    /// ```
    #[must_use]
    pub fn new(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl From<ModelIdentifier> for [u8; 32] {
    fn from(model_identifier: ModelIdentifier) -> Self {
        model_identifier.0
    }
}

impl From<&ModelIdentifier> for [u8; 32] {
    fn from(model_identifier: &ModelIdentifier) -> Self {
        model_identifier.0
    }
}

impl From<[u8; 32]> for ModelIdentifier {
    fn from(value: [u8; 32]) -> Self {
        Self::new(value)
    }
}

impl From<&[u8; 32]> for ModelIdentifier {
    fn from(value: &[u8; 32]) -> Self {
        Self::new(*value)
    }
}

impl Display for ModelIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for val in &self.0[..4] {
            write!(f, "{:02x}", val)?;
        }
        Ok(())
    }
}

/// Statistics model that's used to compress and decompress nucleotide sequences
/// and quality scores.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Model {
    identifier: ModelIdentifier,
    model_type: ModelType,
    spec_type: ContextSpecType,
    contexts: Vec<Context>,
    map: HashMap<ContextSpec, usize>,
}

impl Model {
    #[must_use]
    fn new(
        model_type: ModelType,
        spec_type: ContextSpecType,
        contexts: Vec<Context>,
        map: HashMap<ContextSpec, usize>,
    ) -> Self {
        let identifier = Self::make_identifier(model_type, spec_type, &contexts, &map);

        Self {
            identifier,
            model_type,
            spec_type,
            contexts,
            map,
        }
    }

    #[must_use]
    pub fn with_model_and_spec_type<T: Into<Vec<ComplexContext>>>(
        model_type: ModelType,
        spec_type: ContextSpecType,
        contexts: T,
    ) -> Self {
        let (context_vec, map) = Self::map_contexts(contexts);

        assert!(context_vec
            .iter()
            .all(|x| x.symbol_num() == model_type.symbols_num()));

        Self::new(model_type, spec_type, context_vec, map)
    }

    fn map_contexts<T: Into<Vec<ComplexContext>>>(
        contexts: T,
    ) -> (Vec<Context>, HashMap<ContextSpec, usize>) {
        let mut contexts = contexts.into();
        contexts.sort(); // Ensure deterministic identifier
        let mut context_vec = Vec::new();
        let mut map = HashMap::new();

        for context in contexts {
            let (specs, context) = context.into_spec_and_context();

            let index = context_vec.len();
            context_vec.push(context);
            for spec in specs {
                map.insert(spec, index);
            }
        }

        (context_vec, map)
    }

    /// Constructs new [`Model`] instance that does not contain any contexts.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context_spec::ContextSpecType;
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// assert_eq!(model.model_type(), ModelType::Acids);
    /// assert_eq!(model.context_spec_type(), ContextSpecType::Dummy);
    /// assert_eq!(model.is_empty(), true);
    /// ```
    #[must_use]
    pub fn empty(model_type: ModelType) -> Self {
        Self::new(
            model_type,
            ContextSpecType::Dummy,
            Vec::new(),
            HashMap::new(),
        )
    }

    /// Returns the number of contexts in this [`Model`] instance.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// assert_eq!(model.len(), 0);
    /// ```
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    /// Returns `true` if this [`Model`] instance does not contain any contexts;
    /// `false` otherwise.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// assert_eq!(model.is_empty(), true);
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }

    /// Returns the identifier of this model.
    ///
    /// See the [`ModelIdentifier`] docs for more information on how the
    /// identifier is generated.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// assert_eq!(model.identifier().to_string(), "85989ce9");
    /// ```
    #[inline]
    #[must_use]
    pub fn identifier(&self) -> &ModelIdentifier {
        &self.identifier
    }

    /// Returns the model type of this [`Model`] instance.
    ///
    /// # Examples
    /// ```
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// assert_eq!(model.model_type(), ModelType::Acids);
    /// ```
    #[inline]
    #[must_use]
    pub fn model_type(&self) -> ModelType {
        self.model_type
    }

    /// Returns the context specifier type of this [`Model`] instance.
    ///
    /// # Examples
    /// ```
    /// use idencomp::context_spec::ContextSpecType;
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// assert_eq!(model.context_spec_type(), ContextSpecType::Dummy);
    /// ```
    #[inline]
    #[must_use]
    pub fn context_spec_type(&self) -> ContextSpecType {
        self.spec_type
    }

    #[inline]
    #[must_use]
    pub fn contexts(&self) -> &[Context] {
        &self.contexts
    }

    #[inline]
    #[must_use]
    pub fn map(&self) -> &HashMap<ContextSpec, usize> {
        &self.map
    }

    #[must_use]
    pub fn as_complex_contexts(&self) -> Vec<ComplexContext> {
        let mut specs = Vec::new();
        specs.resize(self.contexts.len(), Vec::new());

        for (&k, &v) in &self.map {
            specs[v].push(k);
        }

        self.contexts
            .iter()
            .zip(specs.into_iter())
            .map(|(context, specs)| ComplexContext::new(specs, context.clone()))
            .collect()
    }

    #[must_use]
    pub fn rate(&self) -> CompressionRate {
        CompressionRate::new(
            self.contexts
                .iter()
                .map(|ctx| ctx.context_prob.get() * *ctx.entropy())
                .sum(),
        )
    }

    fn make_identifier(
        model_type: ModelType,
        spec_type: ContextSpecType,
        contexts: &Vec<Context>,
        map: &HashMap<ContextSpec, usize>,
    ) -> ModelIdentifier {
        let mut hasher = Sha3_256::new();

        hasher.write_u8(model_type as u8).unwrap();
        hasher.update(spec_type.name().as_bytes());

        for context in contexts {
            for &prob in &context.symbol_prob {
                hasher.write_f32::<BigEndian>(prob.get()).unwrap();
            }
        }

        let entries = map.iter().sorted();
        for (&k, &v) in entries {
            hasher.write_u32::<BigEndian>(k.get()).unwrap();
            hasher.write_u32::<BigEndian>(v as u32).unwrap();
        }

        ModelIdentifier::new(hasher.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use itertools::Itertools;

    use crate::_internal_test_data::{SIMPLE_ACID_MODEL, SIMPLE_Q_SCORE_MODEL};
    use crate::context::Context;
    use crate::context_binning::ComplexContext;
    use crate::context_spec::{ContextSpec, ContextSpecType, GenericContextSpec};
    use crate::model::{CompressionRate, Model, ModelIdentifier, ModelType};
    use crate::sequence::Acid;

    #[test]
    fn test_empty_model() {
        let model = Model::empty(ModelType::Acids);

        assert_eq!(model.model_type(), ModelType::Acids);
        assert_eq!(model.context_spec_type(), ContextSpecType::Dummy);
        assert!(model.contexts().is_empty());
        assert!(model.map().is_empty());
        assert!(model.as_complex_contexts().is_empty());
        assert_eq!(model.rate(), CompressionRate::ZERO);
    }

    #[test]
    fn test_new_model() {
        let ctx1 = Context::new_from(0.25, [0.80, 0.10, 0.05, 0.05, 0.00]);
        let spec1: ContextSpec = GenericContextSpec::without_pos([Acid::A], []).into();
        let ctx2 = Context::new_from(0.25, [0.25, 0.50, 0.15, 0.10, 0.00]);
        let spec2: ContextSpec = GenericContextSpec::without_pos([Acid::C], []).into();
        let contexts = [
            ComplexContext::with_single_spec(spec1, ctx1.clone()),
            ComplexContext::with_single_spec(spec2, ctx2.clone()),
        ];

        let model = Model::with_model_and_spec_type(
            ModelType::Acids,
            ContextSpecType::Generic1Acids0QScores0PosBits,
            contexts.clone(),
        );

        assert_eq!(model.model_type(), ModelType::Acids);
        assert_eq!(
            model.context_spec_type(),
            ContextSpecType::Generic1Acids0QScores0PosBits
        );
        assert_eq!(model.contexts(), [ctx1, ctx2]);
        assert_eq!(
            model.map(),
            &HashMap::from([(spec1, 0_usize), (spec2, 1_usize),])
        );
        assert_eq!(model.as_complex_contexts(), contexts);
        assert_eq!(model.rate(), CompressionRate::new(0.6911664));
    }

    #[test]
    fn test_model_identifier_equal() {
        let ctx1 = Context::new_from(0.25, [0.80, 0.10, 0.05, 0.05, 0.00]);
        let spec1: ContextSpec = GenericContextSpec::without_pos([Acid::A], []).into();
        let ctx2 = Context::new_from(0.25, [0.25, 0.50, 0.15, 0.10, 0.00]);
        let spec2: ContextSpec = GenericContextSpec::without_pos([Acid::C], []).into();

        let contexts1 = [
            ComplexContext::with_single_spec(spec1, ctx1.clone()),
            ComplexContext::with_single_spec(spec2, ctx2.clone()),
        ];
        let model1 = Model::with_model_and_spec_type(
            ModelType::Acids,
            ContextSpecType::Generic1Acids0QScores0PosBits,
            contexts1,
        );
        let contexts2 = [
            ComplexContext::with_single_spec(spec2, ctx2),
            ComplexContext::with_single_spec(spec1, ctx1),
        ];
        let model2 = Model::with_model_and_spec_type(
            ModelType::Acids,
            ContextSpecType::Generic1Acids0QScores0PosBits,
            contexts2,
        );

        assert_eq!(model1.identifier(), model2.identifier());
    }
    #[test]
    fn test_model_identifier_display() {
        let identifier = ModelIdentifier::new(b"Tenacious D: The Pick of Destiny".to_owned());

        assert_eq!(identifier.to_string(), "54656e61");
    }

    #[test]
    fn test_model_identifier_unique() {
        let models = [
            Model::empty(ModelType::Acids),
            Model::empty(ModelType::QualityScores),
            SIMPLE_ACID_MODEL.clone(),
            SIMPLE_Q_SCORE_MODEL.clone(),
        ];

        assert!(models.iter().map(|model| model.identifier()).all_unique());
    }

    #[test]
    fn test_compression_rate_display() {
        assert_eq!(format!("{}", CompressionRate::new(0.0)), "0.0000bpv");
        assert_eq!(format!("{}", CompressionRate::new(1.2345)), "1.2345bpv");
        assert_eq!(
            format!("{}", CompressionRate::new(1.234_589_1)),
            "1.2346bpv"
        );
    }
}
