use std::io::{Seek, Write};

use log::debug;

use crate::fastq::FastqSequence;
use crate::idn::compressor::{IdnCompressorOptions, IdnWriteResult};
use crate::idn::model_chooser::ModelChooser;
use crate::idn::writer_idn::IdnWriter;
use crate::model::ModelIdentifier;

pub(super) struct CompressorInitializer<'a, W> {
    writer: &'a mut IdnWriter<W>,
    options: &'a mut IdnCompressorOptions,
    sequences: &'a [FastqSequence],
    model_chooser: ModelChooser,
}

impl<'a, W: Write + Seek> CompressorInitializer<'a, W> {
    #[must_use]
    pub fn new(
        writer: &'a mut IdnWriter<W>,
        options: &'a mut IdnCompressorOptions,
        initial_sequences: &'a [FastqSequence],
    ) -> Self {
        Self {
            writer,
            options,
            sequences: initial_sequences,
            model_chooser: ModelChooser::new(),
        }
    }

    pub fn initialize(mut self) -> IdnWriteResult<()> {
        self.writer.write_header(1)?;
        self.retain_best_models();
        self.write_metadata()?;

        Ok(())
    }

    fn write_metadata(&mut self) -> IdnWriteResult<()> {
        self.add_models_metadata();
        self.writer.write_metadata()?;

        Ok(())
    }

    fn add_models_metadata(&mut self) {
        let identifiers: Vec<_> = self.options.model_provider.identifiers().cloned().collect();
        self.writer.add_models_metadata(&identifiers);
    }

    fn retain_best_models(&mut self) {
        self.options.model_provider.preprocess_compressor_models();

        let acid_models = self
            .model_chooser
            .get_best_acid_models(self.sequences, self.options, 3)
            .into_iter();
        let q_score_models = self
            .model_chooser
            .get_best_q_score_models(self.sequences, self.options, 3)
            .into_iter();
        let identifiers: Vec<ModelIdentifier> = acid_models.chain(q_score_models).collect();
        debug!("Model identifiers:");
        for (index, identifier) in identifiers.iter().enumerate() {
            debug!("[{}] {}", index, identifier);
        }

        self.options
            .model_provider
            .filter_by_identifiers(&identifiers);
    }
}
