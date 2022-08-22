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

/// A store for [`Model`]s that can be used with
/// [`IdnCompressor`](crate::idn::compressor::IdnCompressor) and
/// [`IdnDecompressor`](crate::idn::decompressor::IdnDecompressor). Can be
/// constructed with a list of models or by using a directory, where the models
/// are loaded from.
///
/// `ModelProvider` makes it possible to get model by its identifier. It can
/// also make a new instance by filtering the models inside by a list of
/// identifiers. It also can internally convert [`Model`]s to
/// [`RansEncModel`](crate::sequence_compressor::RansEncModel)s
/// and [`RansDecModel`](crate::sequence_compressor::RansDecModel)s.
#[derive(Debug, Clone)]
pub struct ModelProvider {
    models: Vec<Model>,
    index_map: HashMap<ModelIdentifier, usize>,

    compressor_models: Vec<CompressorModel>,
    decompressor_models: Vec<DecompressorModel>,
}

impl ModelProvider {
    /// Creates a new `ModelProvider` instance containing given collection of
    /// models.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let provider = ModelProvider::new(vec![
    ///     Model::empty(ModelType::Acids),
    ///     Model::empty(ModelType::QualityScores),
    /// ]);
    /// assert_eq!(provider.len(), 2);
    /// ```
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

    /// Creates a new `ModelProvider` instance containing all models loaded from
    /// a directory given by path.
    ///
    /// This functions tries to load *all* files as models and uses
    /// [`SerializableModel::read_model`] function to deserialize them.
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

    /// Returns the index of a model given by an identifier.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// let identifier = model.identifier().clone();
    /// let model_provider = ModelProvider::new(vec![model]);
    ///
    /// assert_eq!(model_provider.index_of(&identifier), 0);
    /// ```
    ///
    /// # Panics
    /// Panics if there is no model with given identifier in this provider.
    #[must_use]
    pub fn index_of(&self, identifier: &ModelIdentifier) -> usize {
        self.index_map[identifier]
    }

    /// Converts [`Model`]s inside this `ModelProvider` to
    /// [`RansEncModel`](crate::sequence_compressor::RansEncModel)s so they can
    /// be obtained with [`Self::acid_enc_models()`] and
    /// [`Self::q_score_enc_models()`].
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    ///
    /// let mut model_provider = ModelProvider::with_empty_models();
    /// assert!(model_provider.acid_enc_models().next().is_none());
    /// model_provider.preprocess_compressor_models();
    /// assert!(model_provider.acid_enc_models().next().is_some());
    /// ```
    pub fn preprocess_compressor_models(&mut self) {
        self.compressor_models = self.models.par_iter().map(|x| x.into()).collect();
    }

    /// Converts [`Model`]s inside this `ModelProvider` to
    /// [`RansDecModel`](crate::sequence_compressor::RansDecModel)s so they can
    /// be obtained with [`Self::decompressor_models()`].
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    ///
    /// let mut model_provider = ModelProvider::with_empty_models();
    /// assert_eq!(model_provider.decompressor_models().len(), 0);
    /// model_provider.preprocess_decompressor_models();
    /// assert_eq!(model_provider.decompressor_models().len(), 2);
    /// ```
    pub fn preprocess_decompressor_models(&mut self) {
        self.decompressor_models = self.models.par_iter().map(|x| x.into()).collect();
    }

    /// Returns a slice of all decoder models of this `ModelProvider`.
    ///
    /// Please note that [`Self::preprocess_decompressor_models()`] has to be
    /// called before using this function, or otherwise it will always return an
    /// empty slice.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    ///
    /// let mut model_provider = ModelProvider::with_empty_models();
    /// assert_eq!(model_provider.decompressor_models().len(), 0);
    /// model_provider.preprocess_decompressor_models();
    /// assert_eq!(model_provider.decompressor_models().len(), 2);
    /// ```
    #[must_use]
    pub fn decompressor_models(&self) -> &[DecompressorModel] {
        &self.decompressor_models
    }

    /// Returns an iterator of all Acid encoder models of this `ModelProvider`.
    ///
    /// Please note that [`Self::preprocess_compressor_models()`] has to be
    /// called before using this function, or otherwise it will always return an
    /// empty iterator.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    ///
    /// let mut model_provider = ModelProvider::with_empty_models();
    /// assert!(model_provider.acid_enc_models().next().is_none());
    /// model_provider.preprocess_compressor_models();
    /// assert!(model_provider.acid_enc_models().next().is_some());
    /// ```
    pub fn acid_enc_models(&self) -> impl Iterator<Item = &AcidRansEncModel> + '_ {
        self.compressor_models
            .iter()
            .filter(|model| model.model_type() == ModelType::Acids)
            .map(|model| model.as_acid())
    }

    /// Returns an iterator of all Quality Score encoder models of this
    /// `ModelProvider`.
    ///
    /// Please note that [`Self::preprocess_compressor_models()`] has to be
    /// called before using this function, or otherwise it will always return an
    /// empty iterator.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    ///
    /// let mut model_provider = ModelProvider::with_empty_models();
    /// assert!(model_provider.q_score_enc_models().next().is_none());
    /// model_provider.preprocess_compressor_models();
    /// assert!(model_provider.q_score_enc_models().next().is_some());
    /// ```
    pub fn q_score_enc_models(&self) -> impl Iterator<Item = &QScoreRansEncModel> + '_ {
        self.compressor_models
            .iter()
            .filter(|model| model.model_type() == ModelType::QualityScores)
            .map(|model| model.as_quality_score())
    }

    /// Returns `Ok` if this `ModelProvider` contains models with all given
    /// identifiers; `Err` (with missing identifier) otherwise.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    /// use idencomp::model::{Model, ModelIdentifier, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// let identifier = model.identifier().clone();
    /// let model_provider = ModelProvider::new(vec![model]);
    ///
    /// assert!(model_provider.has_all_models(&[]).is_ok());
    /// assert!(model_provider.has_all_models(&[identifier]).is_ok());
    /// assert!(model_provider
    ///     .has_all_models(&[ModelIdentifier::new([1; 32])])
    ///     .is_err());
    /// ```
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

    /// Modifies `ModelProvider` in-place so that it only contains models with
    /// given identifiers.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    /// use idencomp::model::{Model, ModelIdentifier, ModelType};
    ///
    /// let model_1 = Model::empty(ModelType::Acids);
    /// let model_2 = Model::empty(ModelType::QualityScores);
    /// let identifier = model_1.identifier().clone();
    /// let mut model_provider = ModelProvider::new(vec![model_1, model_2]);
    ///
    /// assert_eq!(model_provider.len(), 2);
    /// model_provider.filter_by_identifiers(&[identifier]);
    /// assert_eq!(model_provider.len(), 1);
    /// ```
    ///
    /// # Panics
    /// Panics if any of given identifiers is missing in this `ModelProvider`.
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

    /// Returns the number of [`Model`]s this `ModelProvider` contains.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    ///
    /// let provider = ModelProvider::with_empty_models();
    /// assert_eq!(provider.len(), 2);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Returns `true` if this `ModelProvider` does not contain any [`Model`]s.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    ///
    /// let provider = ModelProvider::new(vec![]);
    /// assert_eq!(provider.is_empty(), true);
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// Returns an iterator of identifiers of all models in this
    /// `ModelProvider`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::ModelProvider;
    /// use idencomp::model::{Model, ModelIdentifier, ModelType};
    ///
    /// let model_1 = Model::empty(ModelType::Acids);
    /// let model_2 = Model::empty(ModelType::QualityScores);
    /// let identifier_1 = model_1.identifier().clone();
    /// let identifier_2 = model_2.identifier().clone();
    /// let mut model_provider = ModelProvider::new(vec![model_1, model_2]);
    ///
    /// assert_eq!(
    ///     model_provider.identifiers().collect::<Vec<_>>(),
    ///     vec![&identifier_1, &identifier_2]
    /// );
    /// ```
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

/// Common interface for Acid and Quality Score rANS compressor/decompressor
/// models.
#[derive(Debug, Clone)]
pub enum CoderModel<A, B> {
    /// Acid model variant.
    Acid(A),
    /// Quality Score model variant.
    QualityScore(B),
}

const SCALE_BITS: u8 = 14;

/// rANS compressor model for acids or quality scores.
pub type CompressorModel = CoderModel<AcidRansEncModel, QScoreRansEncModel>;
/// rANS decompressor model for acids or quality scores.
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
    /// Returns [`ModelType`] for this `CoderModel`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::CompressorModel;
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// let compressor_model = CompressorModel::from(&model);
    /// assert_eq!(compressor_model.model_type(), ModelType::Acids);
    /// ```
    #[must_use]
    pub fn model_type(&self) -> ModelType {
        match self {
            CoderModel::Acid(_) => ModelType::Acids,
            CoderModel::QualityScore(_) => ModelType::QualityScores,
        }
    }

    /// Returns the rANS coder model for this `CoderModel`, if this instance has
    /// the type of `ModelType::Acids`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::CompressorModel;
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::Acids);
    /// let identifier = model.identifier().clone();
    /// let compressor_model = CompressorModel::from(&model);
    /// assert_eq!(compressor_model.as_acid().identifier(), &identifier);
    /// ```
    ///
    /// # Panics
    /// Panics if [`Self::model_type()`] is not `ModelType::Acids`.
    #[must_use]
    pub fn as_acid(&self) -> &A {
        match self {
            CoderModel::Acid(model) => model,
            _ => panic!("Expected Acid model"),
        }
    }

    /// Returns the rANS coder model for this `CoderModel`, if this instance has
    /// the type of `ModelType::QualityScores`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::model_provider::CompressorModel;
    /// use idencomp::model::{Model, ModelType};
    ///
    /// let model = Model::empty(ModelType::QualityScores);
    /// let identifier = model.identifier().clone();
    /// let compressor_model = CompressorModel::from(&model);
    /// assert_eq!(
    ///     compressor_model.as_quality_score().identifier(),
    ///     &identifier
    /// );
    /// ```
    ///
    /// # Panics
    /// Panics if [`Self::model_type()`] is not `ModelType::QualityScores`.
    #[must_use]
    pub fn as_quality_score(&self) -> &B {
        match self {
            CoderModel::QualityScore(model) => model,
            _ => panic!("Expected Quality Score model"),
        }
    }
}
