use itertools::izip;
use log::{debug, trace};

use crate::compressor::{RansCompressor, RansDecContext, RansDecompressor, RansEncContext};
use crate::context::Context;
use crate::context_spec::{ContextSpec, ContextSpecGenerator, ContextSpecType};
use crate::fastq::{FastqQualityScore, FastqSequence};
use crate::model::{Model, ModelIdentifier};
use crate::sequence::Acid;
use crate::sequence::Symbol;

#[derive(Debug, Clone)]
pub struct RansEncModel<const SYMBOLS_NUM: usize> {
    identifier: ModelIdentifier,
    context_spec_type: ContextSpecType,
    contexts: Vec<RansEncContext<SYMBOLS_NUM>>,
    map: Vec<usize>,
}

impl<const SYMBOLS_NUM: usize> RansEncModel<SYMBOLS_NUM> {
    pub fn from_model(model: &Model, scale_bits: u8) -> Self {
        check_model(model);

        let mut contexts: Vec<RansEncContext<SYMBOLS_NUM>> =
            Vec::with_capacity(model.contexts().len() + 1);
        contexts.push(RansEncContext::from_context(
            &Context::dummy(SYMBOLS_NUM),
            scale_bits,
        ));
        contexts.extend(
            model
                .contexts()
                .iter()
                .map(|x| RansEncContext::from_context(x, scale_bits)),
        );

        let mut map = vec![0; model.context_spec_type().spec_num() as usize];
        for (k, &v) in model.map() {
            map[k.get() as usize] = v + 1;
        }

        Self {
            identifier: model.identifier().clone(),
            context_spec_type: model.context_spec_type(),
            contexts,
            map,
        }
    }

    #[must_use]
    pub fn identifier(&self) -> &ModelIdentifier {
        &self.identifier
    }

    #[must_use]
    pub fn context_spec_type(&self) -> ContextSpecType {
        self.context_spec_type
    }

    pub fn context_for(&self, spec: ContextSpec) -> &RansEncContext<SYMBOLS_NUM> {
        &self.contexts[self.map[spec.get() as usize]]
    }
}

pub type AcidRansEncModel = RansEncModel<{ Acid::SIZE }>;
pub type QScoreRansEncModel = RansEncModel<{ FastqQualityScore::SIZE }>;

#[derive(Debug)]
pub struct SequenceCompressor {
    compressor: RansCompressor<2>,
}

impl SequenceCompressor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            compressor: RansCompressor::new(),
        }
    }

    #[must_use]
    pub fn compress(
        &mut self,
        sequence: &FastqSequence,
        acid_model: &AcidRansEncModel,
        q_score_model: &QScoreRansEncModel,
    ) -> &[u8] {
        self.compressor.reset();

        let identifier = sequence.identifier().clone();

        let (acid_contexts, q_score_contexts) =
            Self::gen_contexts(sequence, acid_model, q_score_model);

        let acids = sequence.acids().iter().copied().rev();
        let q_scores = sequence.quality_scores().iter().copied().rev();
        let acid_contexts = acid_contexts.into_iter().rev();
        let q_score_contexts = q_score_contexts.into_iter().rev();

        trace!("Compressing sequence {}", identifier);
        trace!("Acids: {:?}", acids);
        trace!("Quality scores: {:?}", q_scores);
        for (acid, q_score, acid_spec, q_score_spec) in
            izip!(acids, q_scores, acid_contexts, q_score_contexts)
        {
            let acid_sym_num = acid as usize;
            let q_score_sym_num = q_score.get();

            trace!(
                "Putting {}, {}: acid_spec: `{}`; q_score_spec: `{}`; acid_sym_num: {}; q_score_sym_num: {}",
                acid, q_score,
                acid_spec, q_score_spec, acid_sym_num, q_score_sym_num
            );
            self.compressor.put(
                acid_model.context_for(acid_spec),
                acid_sym_num,
                q_score_model.context_for(q_score_spec),
                q_score_sym_num,
            );
        }
        self.compressor.flush();

        self.compressor.data()
    }

    fn gen_contexts(
        sequence: &FastqSequence,
        acid_model: &AcidRansEncModel,
        q_score_model: &QScoreRansEncModel,
    ) -> (Vec<ContextSpec>, Vec<ContextSpec>) {
        let mut acid_contexts = Vec::with_capacity(sequence.len());
        let mut q_score_contexts = Vec::with_capacity(sequence.len());

        let mut acid_spec_generator: Box<dyn ContextSpecGenerator> =
            acid_model.context_spec_type.generator(sequence.len());
        let mut q_score_spec_generator: Box<dyn ContextSpecGenerator> =
            q_score_model.context_spec_type.generator(sequence.len());

        for (&acid, &q_score) in sequence
            .acids()
            .iter()
            .zip(sequence.quality_scores().iter())
        {
            let acid_spec = acid_spec_generator.current_context();
            let q_score_spec = q_score_spec_generator.current_context();

            acid_contexts.push(acid_spec);
            q_score_contexts.push(q_score_spec);

            acid_spec_generator.update(acid, q_score);
            q_score_spec_generator.update(acid, q_score);
        }

        (acid_contexts, q_score_contexts)
    }
}

impl Default for SequenceCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct RansDecModel<const SYMBOLS_NUM: usize> {
    context_spec_type: ContextSpecType,
    contexts: Vec<RansDecContext<SYMBOLS_NUM>>,
    map: Vec<usize>,
}

pub type AcidRansDecModel = RansDecModel<{ Acid::SIZE }>;
pub type QScoreRansDecModel = RansDecModel<{ FastqQualityScore::SIZE }>;

impl<const SYMBOLS_NUM: usize> RansDecModel<SYMBOLS_NUM> {
    pub fn from_model(model: &Model, scale_bits: u8) -> Self {
        check_model(model);

        let mut contexts: Vec<RansDecContext<SYMBOLS_NUM>> =
            Vec::with_capacity(model.contexts().len() + 1);
        contexts.push(RansDecContext::from_context(
            &Context::dummy(SYMBOLS_NUM),
            scale_bits,
        ));
        contexts.extend(
            model
                .contexts()
                .iter()
                .map(|x| RansDecContext::from_context(x, scale_bits)),
        );

        let mut map = vec![0; model.context_spec_type().spec_num() as usize];
        for (k, &v) in model.map() {
            map[k.get() as usize] = v + 1;
        }

        Self {
            context_spec_type: model.context_spec_type(),
            contexts,
            map,
        }
    }

    pub fn context_for(&self, spec: ContextSpec) -> &RansDecContext<SYMBOLS_NUM> {
        &self.contexts[self.map[spec.get() as usize]]
    }
}

/// Checks the model before preprocessing to avoid using too much memory
fn check_model(model: &Model) {
    const MAX_CONTEXT_NUM: usize = 10_000;

    let context_num = model.len();
    if context_num > MAX_CONTEXT_NUM {
        panic!(
            "Model too large: context num {}, maximum {}",
            context_num, MAX_CONTEXT_NUM
        );
    }
}

#[derive(Debug)]
pub struct SequenceDecompressor {}

impl SequenceDecompressor {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    #[must_use]
    pub fn decompress(
        &mut self,
        data: &mut [u8],
        seq_length: usize,
        acid_model: &AcidRansDecModel,
        q_score_model: &QScoreRansDecModel,
    ) -> FastqSequence {
        debug!(
            "Decompressing sequence: data_len {}; seq_len {}",
            data.len(),
            seq_length
        );

        let mut acid_generator: Box<dyn ContextSpecGenerator> =
            acid_model.context_spec_type.generator(seq_length);
        let mut q_score_generator: Box<dyn ContextSpecGenerator> =
            q_score_model.context_spec_type.generator(seq_length);

        let mut decompressor: RansDecompressor<2> = RansDecompressor::new(data);

        let mut acids = Vec::with_capacity(seq_length);
        let mut q_scores = Vec::with_capacity(seq_length);
        for _ in 0..seq_length {
            let acid_spec: ContextSpec = acid_generator.current_context();
            let q_score_spec: ContextSpec = q_score_generator.current_context();

            let acid_ctx = acid_model.context_for(acid_spec);
            let q_score_ctx = q_score_model.context_for(q_score_spec);

            let (acid_symbol, q_score_symbol) = decompressor.get(acid_ctx, q_score_ctx);
            let acid = Acid::from_usize(acid_symbol);
            let q_score = FastqQualityScore::new(q_score_symbol as u8);

            trace!(
                "Got {}, {}: acid_spec: `{}`; q_score_spec: `{}`; acid_sym_num: {}; q_score_sym_num: {}",
                acid, q_score,
                acid_spec, q_score_spec, acid_symbol, q_score_symbol
            );

            acids.push(acid);
            q_scores.push(q_score);

            acid_generator.update(acid, q_score);
            q_score_generator.update(acid, q_score);
        }

        FastqSequence::new("", acids, q_scores)
    }
}

#[cfg(test)]
mod tests {

    use crate::_internal_test_data::{
        SHORT_TEST_SEQUENCE, SIMPLE_ACID_MODEL, SIMPLE_Q_SCORE_MODEL, SIMPLE_TEST_SEQUENCE,
    };
    use crate::fastq::FastqSequence;
    use crate::model::{Model, ModelType};
    use crate::sequence_compressor::{
        AcidRansDecModel, AcidRansEncModel, QScoreRansDecModel, QScoreRansEncModel,
        SequenceCompressor, SequenceDecompressor,
    };

    #[test]
    fn round_trip_empty_model_short_seq() {
        let acid_model = Model::empty(ModelType::Acids);
        let q_score_model = Model::empty(ModelType::QualityScores);
        let sequence = &*SHORT_TEST_SEQUENCE;

        let mut data = compress(sequence, &acid_model, &q_score_model);
        let decompressed_sequence = decompress(&mut data, 4, &acid_model, &q_score_model);

        assert_eq!(sequence, &decompressed_sequence);
    }

    #[test_log::test]
    fn round_trip_simple_model_short_seq() {
        let sequence = &*SHORT_TEST_SEQUENCE;

        let mut data = compress(sequence, &SIMPLE_ACID_MODEL, &SIMPLE_Q_SCORE_MODEL);
        let decompressed_sequence = decompress(
            &mut data,
            sequence.len(),
            &SIMPLE_ACID_MODEL,
            &SIMPLE_Q_SCORE_MODEL,
        );

        assert_eq!(sequence, &decompressed_sequence);
    }

    #[test_log::test]
    fn round_trip_simple_model_simple_seq() {
        let sequence = SIMPLE_TEST_SEQUENCE.clone().with_identifier_discarded();

        let mut data = compress(&sequence, &SIMPLE_ACID_MODEL, &SIMPLE_Q_SCORE_MODEL);
        let decompressed_sequence = decompress(
            &mut data,
            sequence.len(),
            &SIMPLE_ACID_MODEL,
            &SIMPLE_Q_SCORE_MODEL,
        );

        assert_eq!(sequence, decompressed_sequence);
    }

    const SCALE_BITS: u8 = 10;

    fn compress(sequence: &FastqSequence, acid_model: &Model, q_score_model: &Model) -> Vec<u8> {
        assert_eq!(acid_model.model_type(), ModelType::Acids);
        assert_eq!(q_score_model.model_type(), ModelType::QualityScores);

        let enc_acid_model = AcidRansEncModel::from_model(acid_model, SCALE_BITS);
        let enc_q_score_model = QScoreRansEncModel::from_model(q_score_model, SCALE_BITS);

        let mut compressor = SequenceCompressor::new();
        let data = compressor.compress(sequence, &enc_acid_model, &enc_q_score_model);

        data.to_owned()
    }

    fn decompress(
        data: &mut [u8],
        seq_length: usize,
        acid_model: &Model,
        q_score_model: &Model,
    ) -> FastqSequence {
        assert_eq!(acid_model.model_type(), ModelType::Acids);
        assert_eq!(q_score_model.model_type(), ModelType::QualityScores);

        let dec_acid_model = AcidRansDecModel::from_model(acid_model, SCALE_BITS);
        let dec_q_score_model = QScoreRansDecModel::from_model(q_score_model, SCALE_BITS);

        let mut decompressor = SequenceDecompressor::new();

        decompressor.decompress(data, seq_length, &dec_acid_model, &dec_q_score_model)
    }
}
