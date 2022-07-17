use std::hash::Hash;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::mem;
use std::sync::Arc;

use binrw::BinRead;
use flate2::read::DeflateDecoder;
use log::debug;

use crate::fastq::FastqSequence;
use crate::idn::data::{
    IdnIdentifierCompression, IdnIdentifiersHeader, IdnSequenceHeader, IdnSliceHeader,
    IdnSwitchModelHeader,
};
use crate::idn::decompressor::{
    IdnDecompressResult, IdnDecompressorError, IdnDecompressorOutState, IdnDecompressorParams,
};
use crate::model::ModelType;
use crate::progress::ByteNum;
use crate::sequence_compressor::{AcidRansDecModel, QScoreRansDecModel, SequenceDecompressor};

#[derive(Debug)]
pub(super) struct IdnBlockDecompressor {
    block_index: u32,
    data: Cursor<Vec<u8>>,
    out_state: Arc<IdnDecompressorOutState>,
    seq_checksum: u32,
    options: Arc<IdnDecompressorParams>,

    last_pos: usize,
    decompressor: SequenceDecompressor,
    identifiers: Vec<String>,
    hasher: crc32fast::Hasher,
    current_acid_model: Option<u8>,
    current_q_score_model: Option<u8>,
}

impl IdnBlockDecompressor {
    #[must_use]
    pub fn new(
        block_index: u32,
        data: Vec<u8>,
        out_state: Arc<IdnDecompressorOutState>,
        seq_checksum: u32,
        options: Arc<IdnDecompressorParams>,
    ) -> Self {
        Self {
            block_index,
            data: Cursor::new(data),
            out_state,
            seq_checksum,
            options,

            last_pos: 0,
            decompressor: SequenceDecompressor::new(),
            identifiers: Vec::new(),
            hasher: crc32fast::Hasher::new(),
            current_acid_model: None,
            current_q_score_model: None,
        }
    }

    fn remaining(data: &Cursor<Vec<u8>>) -> &[u8] {
        let pos = data.position() as usize;
        &data.get_ref()[pos..]
    }

    fn remaining_mut(data: &mut Cursor<Vec<u8>>) -> &mut [u8] {
        let pos = data.position() as usize;
        &mut data.get_mut()[pos..]
    }

    fn is_empty(&self) -> bool {
        Self::remaining(&self.data).is_empty()
    }

    pub fn process(mut self) -> IdnDecompressResult<()> {
        let mut sequences = Vec::new();
        while let Some(sequence) = self.next_sequence()? {
            sequences.push(sequence);
        }

        let _guard = self.out_state.block_lock().lock(self.block_index);
        self.out_state.data_queue().add_all(sequences);
        Ok(())
    }

    fn next_sequence(&mut self) -> IdnDecompressResult<Option<FastqSequence>> {
        let sequence_result = self.next_sequence_internal()?;

        let current_pos = self.data.position() as usize;
        let processed = current_pos - self.last_pos;
        self.last_pos = current_pos;
        self.options
            .progress_notifier
            .processed_bytes(ByteNum::new(processed));

        match &sequence_result {
            Some(sequence) => {
                sequence.hash(&mut self.hasher);
            }
            None => self.check_checksum()?,
        }
        Ok(sequence_result)
    }

    fn next_sequence_internal(&mut self) -> IdnDecompressResult<Option<FastqSequence>> {
        loop {
            if self.is_empty() {
                return Ok(None);
            }

            let header: IdnSliceHeader = IdnSliceHeader::read(&mut self.data)?;
            debug!("Read block slice header: {:?}", header);
            match header {
                IdnSliceHeader::Identifiers(header) => self.handle_identifiers_slice(header)?,
                IdnSliceHeader::SwitchModel(header) => self.handle_switch_model_slice(header)?,
                IdnSliceHeader::Sequence(header) => return self.handle_sequence_slice(header),
            }
        }
    }

    fn check_checksum(&mut self) -> IdnDecompressResult<()> {
        let hasher = mem::take(&mut self.hasher);
        let computed_checksum = hasher.finalize();
        let expected_checksum = self.seq_checksum;

        if computed_checksum != expected_checksum {
            return Err(IdnDecompressorError::block_checksum_mismatch(
                computed_checksum,
                expected_checksum,
            ));
        }

        Ok(())
    }

    fn handle_identifiers_slice(
        &mut self,
        header: IdnIdentifiersHeader,
    ) -> IdnDecompressResult<()> {
        let data_len = header.length as usize;
        let data = &Self::remaining(&self.data)[..data_len];

        let identifiers = match header.compression {
            IdnIdentifierCompression::Brotli => Self::handle_identifiers_slice_brotli(data)?,
            IdnIdentifierCompression::Deflate => Self::handle_identifiers_slice_deflate(data)?,
        };
        self.identifiers = identifiers;

        self.data.seek(SeekFrom::Current(data_len as i64))?;
        Ok(())
    }

    fn handle_identifiers_slice_brotli(data: &[u8]) -> IdnDecompressResult<Vec<String>> {
        let identifier_data = {
            let mut identifier_data = Vec::new();
            let mut reader = brotli::Decompressor::new(data, 4096);
            reader.read_to_end(&mut identifier_data)?;
            identifier_data
        };

        Self::identifiers_from_lines(identifier_data)
    }

    fn handle_identifiers_slice_deflate(data: &[u8]) -> IdnDecompressResult<Vec<String>> {
        let identifier_data = {
            let mut identifier_data = Vec::new();
            let mut reader = DeflateDecoder::new(data);
            reader.read_to_end(&mut identifier_data)?;
            identifier_data
        };

        Self::identifiers_from_lines(identifier_data)
    }

    fn identifiers_from_lines(identifier_data: Vec<u8>) -> IdnDecompressResult<Vec<String>> {
        let identifiers = String::from_utf8(identifier_data)?;
        let mut identifiers: Vec<String> =
            identifiers.lines().map(|line| line.to_owned()).collect();
        identifiers.reverse();

        Ok(identifiers)
    }

    fn handle_switch_model_slice(
        &mut self,
        header: IdnSwitchModelHeader,
    ) -> IdnDecompressResult<()> {
        let model_index = header.model_index as usize;
        let num_models = self.options.model_provider.len();
        if model_index >= num_models {
            return Err(IdnDecompressorError::invalid_model_index(
                model_index as u8,
                num_models as u8,
            ));
        }

        let model = &self.options.model_provider[model_index];
        match model.model_type() {
            ModelType::Acids => self.current_acid_model = Some(model_index as u8),
            ModelType::QualityScores => self.current_q_score_model = Some(model_index as u8),
        }

        Ok(())
    }

    fn handle_sequence_slice(
        &mut self,
        header: IdnSequenceHeader,
    ) -> IdnDecompressResult<Option<FastqSequence>> {
        let data_len = header.length as usize;
        let seq_len = header.seq_len as usize;

        let options = self.options.clone();
        let acid_model = self.get_current_acid_model(&options)?;
        let q_score_model = self.get_current_q_score_model(&options)?;
        let data = &mut Self::remaining_mut(&mut self.data)[..data_len];

        let sequence = self
            .decompressor
            .decompress(data, seq_len, acid_model, q_score_model);
        let sequence = if let Some(identifer) = self.identifiers.pop() {
            sequence.with_identifier(identifer)
        } else {
            sequence
        };

        self.data.seek(SeekFrom::Current(data_len as i64))?;
        Ok(Some(sequence))
    }

    fn get_current_acid_model<'a>(
        &self,
        options: &'a IdnDecompressorParams,
    ) -> IdnDecompressResult<&'a AcidRansDecModel> {
        let index = self
            .current_acid_model
            .ok_or_else(|| IdnDecompressorError::no_active_model(ModelType::Acids))?;

        Ok(options.model_provider.decompressor_models()[index as usize].as_acid())
    }

    fn get_current_q_score_model<'a>(
        &self,
        options: &'a IdnDecompressorParams,
    ) -> IdnDecompressResult<&'a QScoreRansDecModel> {
        let index = self
            .current_q_score_model
            .ok_or_else(|| IdnDecompressorError::no_active_model(ModelType::QualityScores))?;

        Ok(options.model_provider.decompressor_models()[index as usize].as_quality_score())
    }
}
