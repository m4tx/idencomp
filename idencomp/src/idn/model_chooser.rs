use itertools::Itertools;
use log::debug;

use crate::clustering::{ClusterCostCalculator, Clustering};
use crate::compressor::RansCompressor;
use crate::context_spec::ContextSpecGenerator;
use crate::fastq::{FastqQualityScore, FastqSequence};
use crate::idn::compressor::{CompressionQuality, IdnCompressorOptions};
use crate::model::ModelIdentifier;
use crate::sequence::{Acid, Symbol};
use crate::sequence_compressor::{AcidRansEncModel, QScoreRansEncModel, RansEncModel};

#[derive(Debug)]
pub(super) struct ModelChooser {
    model_tester: ModelTester,
    clustering: Clustering,
}

impl ModelChooser {
    #[must_use]
    pub fn new() -> Self {
        Self {
            model_tester: ModelTester::new(),
            clustering: Clustering::new(),
        }
    }

    pub fn get_best_acid_models<'a>(
        &mut self,
        sequences: &[FastqSequence],
        options: &'a IdnCompressorOptions,
        model_num: usize,
    ) -> Vec<ModelIdentifier> {
        let models: Vec<&AcidRansEncModel> = options.model_provider.acid_enc_models().collect();
        debug_assert!(!models.is_empty());

        if models.len() == 1 {
            debug!("Only one acid model registered");
            return vec![models[0].identifier().clone()];
        }

        debug!("Calculating the best acid models for this file");
        if Self::use_clustering(options) {
            self.cluster_models(&models, sequences, model_num)
        } else {
            self.get_model_ranking(&models, sequences, model_num)
        }
    }

    pub fn get_best_q_score_models<'a>(
        &mut self,
        sequences: &[FastqSequence],
        options: &'a IdnCompressorOptions,
        model_num: usize,
    ) -> Vec<ModelIdentifier> {
        let models: Vec<&QScoreRansEncModel> =
            options.model_provider.q_score_enc_models().collect();
        debug_assert!(!models.is_empty());

        if models.len() == 1 {
            debug!("Only one quality score model registered");
            return vec![models[0].identifier().clone()];
        }

        debug!("Calculating the best quality score models for this file");
        if Self::use_clustering(options) {
            self.cluster_models(&models, sequences, model_num)
        } else {
            self.get_model_ranking(&models, sequences, model_num)
        }
    }

    const CLUSTERING_THRESHOLD: CompressionQuality = CompressionQuality::new(2);
    fn use_clustering(options: &IdnCompressorOptions) -> bool {
        options.quality >= Self::CLUSTERING_THRESHOLD
    }

    fn cluster_models<'a, const SYMBOLS_NUM: usize>(
        &mut self,
        models: &[&'a RansEncModel<SYMBOLS_NUM>],
        sequences: &[FastqSequence],
        model_num: usize,
    ) -> Vec<ModelIdentifier> {
        let clusters =
            self.clustering
                .make_clusters(&mut self.model_tester, models, sequences, model_num);

        clusters
            .into_iter()
            .map(|cluster| {
                let model_identifier = models[cluster.centroid].identifier().clone();
                debug!(
                    "Cluster {} has {} sequences",
                    model_identifier,
                    cluster.values.len()
                );

                model_identifier
            })
            .collect()
    }

    fn get_model_ranking<'a, const SYMBOLS_NUM: usize>(
        &mut self,
        models: &[&'a RansEncModel<SYMBOLS_NUM>],
        sequences: &[FastqSequence],
        model_num: usize,
    ) -> Vec<ModelIdentifier> {
        let mut model_scores: Vec<u32> = vec![0; models.len()];

        for sequence in sequences {
            let lengths = models
                .iter()
                .map(|model| self.model_tester.compute_size(sequence, model));
            let ranking_for_seq = lengths
                .enumerate()
                .sorted_by_key(|(_, len)| *len)
                .map(|(model_index, _)| model_index);
            for (i, model_index) in ranking_for_seq.enumerate() {
                model_scores[model_index] += i as u32 + 1;
            }
        }

        let ranking_sorted = model_scores
            .into_iter()
            .enumerate()
            .sorted_by_key(|(_model_index, score)| *score)
            .inspect(|(model_index, score)| {
                debug!(
                    "Score for model {}: {}",
                    models[*model_index].identifier(),
                    score
                );
            })
            .map(|(model_index, _score)| models[model_index].identifier().clone())
            .take(model_num);
        ranking_sorted.collect()
    }

    pub fn get_best_acid_model_for<'a>(
        &mut self,
        sequence: &FastqSequence,
        options: &'a IdnCompressorOptions,
        current_model: Option<&ModelIdentifier>,
    ) -> (usize, &'a AcidRansEncModel) {
        debug!(
            "Calculating the best acid model for `{}`",
            sequence.identifier()
        );
        let models = options.model_provider.acid_enc_models();
        self.get_best_model_for(sequence, models, current_model)
    }

    pub fn get_best_q_score_model_for<'a>(
        &mut self,
        sequence: &FastqSequence,
        options: &'a IdnCompressorOptions,
        current_model: Option<&ModelIdentifier>,
    ) -> (usize, &'a QScoreRansEncModel) {
        debug!(
            "Calculating the best quality score model for `{}`",
            sequence.identifier()
        );
        let models = options.model_provider.q_score_enc_models();
        self.get_best_model_for(sequence, models, current_model)
    }

    fn get_best_model_for<'a, const SYMBOLS_NUM: usize, T>(
        &mut self,
        sequence: &FastqSequence,
        models: T,
        current_model: Option<&ModelIdentifier>,
    ) -> (usize, &'a RansEncModel<SYMBOLS_NUM>)
    where
        T: Iterator<Item = &'a RansEncModel<SYMBOLS_NUM>>,
    {
        const SWITCH_MODEL_PENALTY: usize = 2;

        models
            .map(|model| {
                let len = self.model_tester.compute_size(sequence, model);
                let penalty = if Some(model.identifier()) != current_model {
                    SWITCH_MODEL_PENALTY
                } else {
                    0
                };
                debug!(
                    "Length with model {}: {} + {} (penalty)",
                    model.identifier(),
                    len,
                    penalty
                );

                (len + penalty, model)
            })
            .min_by(|(len_1, _), (len_2, _)| len_1.cmp(len_2))
            .expect("No quality models provided")
    }
}

#[derive(Debug)]
struct ModelTester {
    compressor: RansCompressor<1>,
}

impl ModelTester {
    #[must_use]
    fn new() -> Self {
        Self {
            compressor: RansCompressor::new(),
        }
    }

    #[must_use]
    fn compute_size<const SYMBOLS_NUM: usize>(
        &mut self,
        sequence: &FastqSequence,
        model: &RansEncModel<SYMBOLS_NUM>,
    ) -> usize {
        self.compressor.reset();

        let acids = sequence.acids().iter().cloned();
        let q_scores = sequence.quality_scores().iter().cloned();

        let mut spec_generator: Box<dyn ContextSpecGenerator> =
            model.context_spec_type().generator(sequence.len());

        for (acid, q_score) in acids.zip(q_scores) {
            let spec = spec_generator.current_context();
            let symbol_num = match SYMBOLS_NUM {
                Acid::SIZE => acid as usize,
                FastqQualityScore::SIZE => q_score.get(),
                _ => unimplemented!(),
            };

            self.compressor.put(model.context_for(spec), symbol_num);

            spec_generator.update(acid, q_score);
        }
        self.compressor.flush();

        self.compressor.data().len()
    }
}

impl<const SYMBOLS_NUM: usize> ClusterCostCalculator<FastqSequence, &RansEncModel<SYMBOLS_NUM>>
    for &mut ModelTester
{
    fn cost_for(&mut self, value: &FastqSequence, centroid: &&RansEncModel<SYMBOLS_NUM>) -> u32 {
        self.compute_size(value, centroid) as u32
    }
}
