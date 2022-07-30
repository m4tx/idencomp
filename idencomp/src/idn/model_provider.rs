use std::collections::{HashMap, HashSet};
use std::fs::{DirEntry, File};
use std::ops::Index;
use std::path::Path;
use std::{fs, mem};

use log::debug;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::model::{Model, ModelIdentifier, ModelType};
use crate::model_serializer::SerializableModel;
use crate::sequence_compressor::{
    AcidRansDecModel, AcidRansEncModel, QScoreRansDecModel, QScoreRansEncModel,
};

#[derive(Debug, Clone)]
pub struct ModelProvider {
    models: Vec<Model>,
    index_map: HashMap<ModelIdentifier, usize>,

    compressor_models: Vec<CompressorModel>,
    decompressor_models: Vec<DecompressorModel>,
}

impl ModelProvider {
    #[must_use]
    pub fn new(models: Vec<Model>) -> Self {
        let model_num = models.len();

        let mut provider = Self {
            models,
            index_map: HashMap::with_capacity(model_num),
            compressor_models: Vec::new(),
            decompressor_models: Vec::new(),
        };
        provider.rebuild_index_map();
        provider
    }

    /// Creates a new `ModelProvider` instance containing an empty acid model
    /// and an empty quality score model.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    ///
    /// let provider = ModelProvider::with_empty_models();
    /// assert_eq!(provider.len(), 2);
    /// ```
    #[must_use]
    pub fn with_empty_models() -> Self {
        Self::new(vec![
            Model::empty(ModelType::Acids),
            Model::empty(ModelType::QualityScores),
        ])
    }

    pub fn from_directory(directory: &Path) -> Result<Self, anyhow::Error> {
        let paths = fs::read_dir(directory)?;
        let paths: Vec<Result<DirEntry, _>> = paths.collect();

        let models: Result<Vec<Model>, anyhow::Error> = paths
            .into_par_iter()
            .map(|dir_entry| {
                let dir_entry = dir_entry?;
                let path = &dir_entry.path();
                let file = File::open(path)?;
                let model = SerializableModel::read_model(file)?;

                debug!(
                    "Registering model {} with type {} from `{}`",
                    model.identifier(),
                    model.model_type(),
                    path.file_name().unwrap().to_string_lossy()
                );

                Ok(model)
            })
            .collect();

        Ok(Self::new(models?))
    }

    fn rebuild_index_map(&mut self) {
        self.index_map.clear();
        for (index, context) in self.models.iter().enumerate() {
            self.index_map.insert(context.identifier().clone(), index);
        }
    }

    #[must_use]
    pub fn index_of(&self, identifier: &ModelIdentifier) -> usize {
        self.index_map[identifier]
    }

    pub fn preprocess_compressor_models(&mut self) {
        self.compressor_models = self.models.par_iter().map(|x| x.into()).collect();
    }

    pub fn preprocess_decompressor_models(&mut self) {
        self.decompressor_models = self.models.par_iter().map(|x| x.into()).collect();
    }

    #[must_use]
    pub fn decompressor_models(&self) -> &[DecompressorModel] {
        &self.decompressor_models
    }

    pub fn acid_enc_models(&self) -> impl Iterator<Item = &AcidRansEncModel> + '_ {
        self.compressor_models
            .iter()
            .filter(|model| model.model_type() == ModelType::Acids)
            .map(|model| model.as_acid())
    }

    pub fn q_score_enc_models(&self) -> impl Iterator<Item = &QScoreRansEncModel> + '_ {
        self.compressor_models
            .iter()
            .filter(|model| model.model_type() == ModelType::QualityScores)
            .map(|model| model.as_quality_score())
    }

    pub fn has_all_models(&self, identifiers: &[ModelIdentifier]) -> Result<(), ModelIdentifier> {
        let mut all_identifiers = HashSet::new();
        all_identifiers.extend(self.identifiers());
        for identifier in identifiers {
            if !all_identifiers.contains(identifier) {
                return Err(identifier.clone());
            }
        }

        Ok(())
    }

    pub fn filter_by_identifiers(&mut self, identifiers: &[ModelIdentifier]) {
        self.has_all_models(identifiers).expect("Unknown model");

        let dummy_model = Model::empty(ModelType::Acids);

        let indices: Vec<usize> = identifiers
            .iter()
            .map(|identifier| self.index_of(identifier))
            .collect();

        self.models = indices
            .iter()
            .map(|&index| mem::replace(&mut self.models[index], dummy_model.clone()))
            .collect();

        if !self.compressor_models.is_empty() {
            let dummy_comp_model = CompressorModel::from(&dummy_model);
            self.compressor_models = indices
                .iter()
                .map(|&index| {
                    mem::replace(&mut self.compressor_models[index], dummy_comp_model.clone())
                })
                .collect();
        }

        if !self.decompressor_models.is_empty() {
            let dummy_decomp_model = DecompressorModel::from(&dummy_model);
            self.decompressor_models = indices
                .iter()
                .map(|&index| {
                    mem::replace(
                        &mut self.decompressor_models[index],
                        dummy_decomp_model.clone(),
                    )
                })
                .collect();
        }

        self.rebuild_index_map();
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    pub fn identifiers(&self) -> impl Iterator<Item = &ModelIdentifier> {
        self.models.iter().map(|model| model.identifier())
    }
}

impl Default for ModelProvider {
    fn default() -> Self {
        Self::with_empty_models()
    }
}

impl Index<usize> for ModelProvider {
    type Output = Model;

    fn index(&self, index: usize) -> &Self::Output {
        &self.models[index]
    }
}

#[derive(Debug, Clone)]
pub enum CoderModel<A, B> {
    Acid(A),
    QualityScore(B),
}

const SCALE_BITS: u8 = 14;

pub type CompressorModel = CoderModel<AcidRansEncModel, QScoreRansEncModel>;
pub type DecompressorModel = CoderModel<AcidRansDecModel, QScoreRansDecModel>;

impl From<&Model> for CompressorModel {
    fn from(model: &Model) -> Self {
        debug!(
            "Pre-processing model {} with type {} as a compressor model",
            model.identifier(),
            model.model_type(),
        );

        match model.model_type() {
            ModelType::Acids => Self::Acid(AcidRansEncModel::from_model(model, SCALE_BITS)),
            ModelType::QualityScores => {
                Self::QualityScore(QScoreRansEncModel::from_model(model, SCALE_BITS))
            }
        }
    }
}

impl From<&Model> for DecompressorModel {
    fn from(model: &Model) -> Self {
        debug!(
            "Pre-processing model {} with type {} as a decompressor model",
            model.identifier(),
            model.model_type(),
        );

        match model.model_type() {
            ModelType::Acids => Self::Acid(AcidRansDecModel::from_model(model, SCALE_BITS)),
            ModelType::QualityScores => {
                Self::QualityScore(QScoreRansDecModel::from_model(model, SCALE_BITS))
            }
        }
    }
}

impl<A, B> CoderModel<A, B> {
    pub fn model_type(&self) -> ModelType {
        match self {
            CoderModel::Acid(_) => ModelType::Acids,
            CoderModel::QualityScore(_) => ModelType::QualityScores,
        }
    }

    pub fn as_acid(&self) -> &A {
        match self {
            CoderModel::Acid(model) => model,
            _ => panic!("Expected Acid model"),
        }
    }

    pub fn as_quality_score(&self) -> &B {
        match self {
            CoderModel::QualityScore(model) => model,
            _ => panic!("Expected Quality Score model"),
        }
    }
}
