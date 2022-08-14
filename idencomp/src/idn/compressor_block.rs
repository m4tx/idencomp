use std::io::Write;
use std::mem;
use std::sync::Arc;

use flate2::write::DeflateEncoder;
use itertools::Itertools;
use log::debug;

use crate::fastq::FastqSequence;
use crate::idn::compressor::{
    CompressionQuality, CompressionStats, IdnCompressorOptions, IdnCompressorOutState,
    IdnCompressResult,
};
use crate::idn::data::IdnIdentifierCompression;
use crate::idn::model_chooser::ModelChooser;
use crate::idn::writer_block::BlockWriter;
use crate::progress::ByteNum;
use crate::sequence_compressor::{AcidRansEncModel, QScoreRansEncModel, SequenceCompressor};

pub(super) struct IdnBlockCompressor<W> {
    options: Arc<IdnCompressorOptions>,
    out_state: Arc<IdnCompressorOutState<W>>,
    block_index: u32,
    sequences: Vec<FastqSequence>,
    stats: Arc<CompressionStats>,

    block_writer: BlockWriter,
    compressor: SequenceCompressor,
    current_acid_model: Option<u8>,
    current_q_score_model: Option<u8>,
    model_chooser: ModelChooser,

    // Stats
    in_bytes: ByteNum,
    in_symbols: usize,
    in_identifier_bytes: usize,
    out_identifier_bytes: usize,
    out_acid_bytes: usize,
    out_q_score_bytes: usize,
    acid_model_switches: usize,
    q_score_model_switches: usize,
}

impl<W: Write> IdnBlockCompressor<W> {
    pub fn new(
        options: Arc<IdnCompressorOptions>,
        out_state: Arc<IdnCompressorOutState<W>>,
        block_index: u32,
        sequences: Vec<FastqSequence>,
        stats: Arc<CompressionStats>,
    ) -> Self {
        Self {
            options,
            out_state,
            block_index,
            sequences,
            stats,

            block_writer: BlockWriter::new(),
            compressor: SequenceCompressor::new(),
            current_acid_model: None,
            current_q_score_model: None,
            model_chooser: ModelChooser::new(),

            in_bytes: ByteNum::ZERO,
            in_symbols: 0,
            in_identifier_bytes: 0,
            out_identifier_bytes: 0,
            out_acid_bytes: 0,
            out_q_score_bytes: 0,
            acid_model_switches: 0,
            q_score_model_switches: 0,
        }
    }

    pub fn process(mut self) -> IdnCompressResult<()> {
        self.prepare_to_write()?;
        self.write()?;

        Ok(())
    }

    fn prepare_to_write(&mut self) -> IdnCompressResult<()> {
        if self.sequences.is_empty() {
            return Ok(());
        }

        let sequences = mem::take(&mut self.sequences);
        let options = self.options.clone();

        if options.include_identifiers {
            self.write_identifiers(&sequences, &options)?;
        }

        if options.fast {
            assert_eq!(self.options.model_provider.len(), 2);
            self.block_writer.write_switch_model(0)?;
            self.block_writer.write_switch_model(1)?;
        }
        let default_acid_model = options.model_provider.acid_enc_models().next().unwrap();
        let default_q_score_model = options.model_provider.q_score_enc_models().next().unwrap();

        for sequence in sequences.iter() {
            let (acid_model, q_score_model) = if options.fast {
                (default_acid_model, default_q_score_model)
            } else {
                let acid_model = self.switch_to_best_acid_model_for(sequence, &options)?;
                let q_score_model = self.switch_to_best_q_score_model_for(sequence, &options)?;
                (acid_model, q_score_model)
            };

            self.in_bytes += sequence.size();
            self.in_symbols += sequence.len();
            self.in_identifier_bytes += sequence.identifier().len();

            self.write_sequence(sequence, acid_model, q_score_model, &options)?;
        }

        Ok(())
    }

    fn write(self) -> IdnCompressResult<()> {
        let _guard = self.out_state.block_lock().lock(self.block_index);
        let mut writer_guard = self.out_state.writer();
        let mut w = writer_guard.writer_for_block();

        self.block_writer.write_to(&mut w)?;
        w.flush()?;

        self.stats.add_in_bytes(self.in_bytes);
        self.stats.add_in_identifier_bytes(self.in_identifier_bytes);
        self.stats.add_in_symbols(self.in_symbols);
        self.stats.set_out_bytes(w.position() as usize);
        self.stats
            .add_out_identifier_bytes(self.out_identifier_bytes);
        self.stats.add_out_acid_bytes(self.out_acid_bytes);
        self.stats.add_out_q_score_bytes(self.out_q_score_bytes);
        self.stats.inc_blocks();
        self.stats.add_acid_model_switches(self.acid_model_switches);
        self.stats
            .add_q_score_model_switches(self.q_score_model_switches);

        Ok(())
    }

    const BROTLI_THRESHOLD: CompressionQuality = CompressionQuality::new(8);
    fn write_identifiers(
        &mut self,
        sequences: &[FastqSequence],
        options: &IdnCompressorOptions,
    ) -> IdnCompressResult<()> {
        if options.quality >= Self::BROTLI_THRESHOLD {
            let data = Self::compress_identifiers_brotli(sequences)?;
            self.out_identifier_bytes += data.len();
            self.block_writer
                .write_identifiers(IdnIdentifierCompression::Brotli, &data)
        } else {
            let data = Self::compress_identifiers_deflate(sequences)?;
            self.out_identifier_bytes += data.len();
            self.block_writer
                .write_identifiers(IdnIdentifierCompression::Deflate, &data)
        }
    }

    fn compress_identifiers_brotli(sequences: &[FastqSequence]) -> IdnCompressResult<Vec<u8>> {
        let identifiers = Self::identifiers_as_lines(sequences);

        let mut data = Vec::new();
        {
            let mut br_writer = brotli::enc::writer::CompressorWriter::new(&mut data, 4096, 11, 20);
            br_writer.write_all(identifiers.as_bytes())?;
        }

        debug!(
            "Compressed {} bytes of identifiers into {} bytes with Brotli",
            identifiers.len(),
            data.len()
        );

        Ok(data)
    }

    fn compress_identifiers_deflate(sequences: &[FastqSequence]) -> IdnCompressResult<Vec<u8>> {
        let identifiers = Self::identifiers_as_lines(sequences);

        let mut encoder = DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(identifiers.as_bytes())?;
        let data = encoder.finish()?;

        debug!(
            "Compressed {} bytes of identifiers into {} bytes with Deflate",
            identifiers.len(),
            data.len()
        );

        Ok(data)
    }

    fn identifiers_as_lines(sequences: &[FastqSequence]) -> String {
        let identifiers = sequences
            .iter()
            .map(|sequence| sequence.identifier().str())
            .join("\n");

        identifiers
    }

    pub fn write_sequence(
        &mut self,
        sequence: &FastqSequence,
        acid_model: &AcidRansEncModel,
        q_score_model: &QScoreRansEncModel,
        options: &IdnCompressorOptions,
    ) -> IdnCompressResult<()> {
        let seq_len = sequence.len();
        let seq_identifier = sequence.identifier().clone();
        let data = self
            .compressor
            .compress(sequence, acid_model, q_score_model);
        debug!(
            "Encoded sequence `{}` (length: {}) with {} bytes",
            seq_identifier,
            seq_len,
            data.len()
        );

        self.block_writer.write_sequence(sequence, data)?;
        options.progress_notifier.processed_bytes(sequence.size());
        Ok(())
    }

    fn switch_to_best_acid_model_for<'a>(
        &mut self,
        sequence: &FastqSequence,
        options: &'a IdnCompressorOptions,
    ) -> IdnCompressResult<&'a AcidRansEncModel> {
        let current_identifier = self
            .current_acid_model
            .map(|index| self.options.model_provider[index as usize].identifier());
        let (bytes, model) =
            self.model_chooser
                .get_best_acid_model_for(sequence, options, current_identifier);
        let index = options.model_provider.index_of(model.identifier()) as u8;

        if self.current_acid_model != Some(index) {
            self.block_writer.write_switch_model(index)?;
            self.current_acid_model = Some(index);

            debug!("Switching to acid model: {}", model.identifier());
            self.acid_model_switches += 1;
        }

        self.out_acid_bytes += bytes;
        Ok(model)
    }

    fn switch_to_best_q_score_model_for<'a>(
        &mut self,
        sequence: &FastqSequence,
        options: &'a IdnCompressorOptions,
    ) -> IdnCompressResult<&'a QScoreRansEncModel> {
        let current_identifier = self
            .current_q_score_model
            .map(|index| self.options.model_provider[index as usize].identifier());
        let (bytes, model) =
            self.model_chooser
                .get_best_q_score_model_for(sequence, options, current_identifier);
        let index = options.model_provider.index_of(model.identifier()) as u8;

        if self.current_q_score_model != Some(index) {
            self.block_writer.write_switch_model(index)?;
            self.current_q_score_model = Some(index);

            debug!("Switching to quality score model: {}", model.identifier());
            self.q_score_model_switches += 1;
        }

        self.out_q_score_bytes += bytes;
        Ok(model)
    }
}
