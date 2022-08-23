use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::io::Read;
use std::string::FromUtf8Error;
use std::sync::Arc;
use std::time::Instant;

use binrw::BinRead;
use itertools::Itertools;
use log::{debug, info, trace};

use super::no_seek::NoSeek;
use crate::fastq::FastqSequence;
use crate::idn::common::{format_stats, DataQueue, IdnBlockLock};
use crate::idn::data::{
    IdnBlockHeader, IdnHeader, IdnMetadataHeader, IdnMetadataItem, IdnModelsMetadata,
};
use crate::idn::decompressor_block::IdnBlockDecompressor;
use crate::idn::model_provider::ModelProvider;
use crate::idn::thread_pool::ThreadPool;
use crate::model::{ModelIdentifier, ModelType};
use crate::progress::{ByteNum, DummyProgressNotifier, ProgressNotifier};

/// Error occurring during decompression of an IDN file.
#[derive(Debug, Default)]
pub enum IdnDecompressorError {
    /// Invalid decompressor state.
    #[default]
    InvalidState,
    /// I/O error occurred when reading the IDN file.
    IoError(std::io::Error),
    /// Error decoding UTF-8 string data.
    Utf8Error(FromUtf8Error),
    /// File structure invalid.
    SerializeError(binrw::Error),
    /// Unknown IDN file format version.
    InvalidVersion(u8),
    /// The calculated and saved block content checksums are not equal.
    BlockChecksumMismatch(u32, u32),
    /// The model index requested in a switch is greater than the total number
    /// of models.
    InvalidModelIndex(u8, u8),
    /// Sequence slice occurred without prior acid/quality score "switch model"
    /// slice.
    NoActiveModel(ModelType),
    /// Unknown model identifier occurred in the file metadata.
    UnknownModel(ModelIdentifier),
}

impl IdnDecompressorError {
    #[must_use]
    pub(super) fn block_checksum_mismatch(actual: u32, expected: u32) -> Self {
        Self::BlockChecksumMismatch(actual, expected)
    }

    #[must_use]
    pub(super) fn invalid_model_index(index: u8, num_models: u8) -> Self {
        Self::InvalidModelIndex(index, num_models)
    }

    #[must_use]
    pub(super) fn no_active_model(model_type: ModelType) -> Self {
        Self::NoActiveModel(model_type)
    }

    #[must_use]
    pub(super) fn unknown_model(model_identifier: ModelIdentifier) -> Self {
        Self::UnknownModel(model_identifier)
    }
}

impl From<std::io::Error> for IdnDecompressorError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<FromUtf8Error> for IdnDecompressorError {
    fn from(e: FromUtf8Error) -> Self {
        Self::Utf8Error(e)
    }
}

impl From<binrw::Error> for IdnDecompressorError {
    fn from(e: binrw::Error) -> Self {
        Self::SerializeError(e)
    }
}

impl Display for IdnDecompressorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IdnDecompressorError::InvalidState => write!(f, "Invalid decompressor state"),
            IdnDecompressorError::IoError(e) => write!(f, "IO error: {}", e),
            IdnDecompressorError::Utf8Error(e) => write!(f, "UTF-8 error: {}", e),
            IdnDecompressorError::SerializeError(e) => write!(f, "Serialize error: {}", e),
            IdnDecompressorError::InvalidVersion(ver) => {
                write!(f, "Invalid IDN file version: {}", ver)
            }
            IdnDecompressorError::BlockChecksumMismatch(actual, expected) => write!(
                f,
                "Invalid block checksum (actual: {:08X}, expected: {:08X})",
                actual, expected
            ),
            IdnDecompressorError::InvalidModelIndex(model_index, num_models) => write!(
                f,
                "Invalid model index (read: {}, number of active models: {})",
                model_index, num_models
            ),
            IdnDecompressorError::NoActiveModel(model_type) => write!(
                f,
                "No active {} model set, but read has been requested",
                model_type
            ),
            IdnDecompressorError::UnknownModel(model_identifier) => {
                write!(f, "Unknown model {} used by the file", model_identifier)
            }
        }
    }
}

impl Error for IdnDecompressorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            IdnDecompressorError::IoError(e) => Some(e),
            IdnDecompressorError::Utf8Error(e) => Some(e),
            IdnDecompressorError::SerializeError(e) => Some(e),
            _ => None,
        }
    }
}

/// The result of decompressing IDN.
pub type IdnDecompressResult<T> = Result<T, IdnDecompressorError>;

/// IDN decompression parameters that can be set by user.
#[derive(Debug, Clone)]
pub struct IdnDecompressorParams {
    pub(super) model_provider: ModelProvider,
    pub(super) progress_notifier: Arc<dyn ProgressNotifier>,
    pub(super) thread_num: usize,
}

impl IdnDecompressorParams {
    /// Returns a new instance of a builder for `IdnDecompressorParams`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::decompressor::IdnDecompressorParams;
    ///
    /// let _params: IdnDecompressorParams = IdnDecompressorParams::builder().build();
    /// ```
    #[must_use]
    pub fn builder() -> IdnDecompressorParamsBuilder {
        IdnDecompressorParamsBuilder::new()
    }
}

impl Default for IdnDecompressorParams {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// The builder for [`IdnDecompressorParams`].
#[derive(Debug, Clone)]
pub struct IdnDecompressorParamsBuilder {
    model_provider: ModelProvider,
    progress_notifier: Arc<dyn ProgressNotifier>,
    thread_num: usize,
}

impl IdnDecompressorParamsBuilder {
    /// Returns a new instance of `IdnDecompressorParamsBuilder`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::decompressor::{IdnDecompressorParams, IdnDecompressorParamsBuilder};
    ///
    /// let _params: IdnDecompressorParams = IdnDecompressorParamsBuilder::new().build();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            model_provider: ModelProvider::default(),
            progress_notifier: Arc::new(DummyProgressNotifier),
            thread_num: 0,
        }
    }

    /// Sets the model provider instance to be used for decompression.
    pub fn model_provider(&mut self, model_provider: ModelProvider) -> &mut Self {
        let mut new = self;
        new.model_provider = model_provider;
        new
    }

    /// Sets the progress notifier instance.
    pub fn progress_notifier(&mut self, progress_notifier: Arc<dyn ProgressNotifier>) -> &mut Self {
        let mut new = self;
        new.progress_notifier = progress_notifier;
        new
    }

    /// Sets the maximum number of additional CPU threads to be spawned during
    /// decompression.
    pub fn thread_num(&mut self, thread_num: usize) -> &mut Self {
        let mut new = self;
        new.thread_num = thread_num;
        new
    }

    /// Builds the `IdnDecompressorParams`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::decompressor::{IdnDecompressorParams, IdnDecompressorParamsBuilder};
    ///
    /// let _params: IdnDecompressorParams = IdnDecompressorParamsBuilder::new().build();
    /// ```
    #[must_use]
    pub fn build(&mut self) -> IdnDecompressorParams {
        IdnDecompressorParams {
            model_provider: self.model_provider.clone(),
            progress_notifier: self.progress_notifier.clone(),
            thread_num: self.thread_num,
        }
    }
}

impl Default for IdnDecompressorParamsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub(super) struct IdnDecompressorOutState {
    data_queue: DataQueue<FastqSequence>,
    block_lock: IdnBlockLock,
}

impl IdnDecompressorOutState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data_queue: DataQueue::new(),
            block_lock: IdnBlockLock::new(),
        }
    }

    pub fn data_queue(&self) -> &DataQueue<FastqSequence> {
        &self.data_queue
    }

    pub fn block_lock(&self) -> &IdnBlockLock {
        &self.block_lock
    }
}

#[derive(Debug, Eq, PartialEq)]
enum IdnDecompressorState {
    Uninitialized,
    Reading,
    LastBlockReached,
}

impl IdnDecompressorState {
    pub fn not_finished(&self) -> bool {
        *self != Self::LastBlockReached
    }
}

#[derive(Debug)]
struct IdnDecompressorInner<R> {
    reader: NoSeek<R>,
    options: Arc<IdnDecompressorParams>,
    out_state: Arc<IdnDecompressorOutState>,
    thread_pool: ThreadPool<IdnDecompressorError>,

    state: IdnDecompressorState,
    current_block: u32,
}

impl<R: Read> IdnDecompressorInner<R> {
    #[must_use]
    fn new(
        reader: R,
        params: IdnDecompressorParams,
        state: Arc<IdnDecompressorOutState>,
        thread_pool: ThreadPool<IdnDecompressorError>,
    ) -> Self {
        Self {
            reader: NoSeek::new(reader),
            options: Arc::new(params),
            out_state: state,
            thread_pool,

            state: IdnDecompressorState::Uninitialized,
            current_block: 0,
        }
    }

    fn initialize(&mut self) -> IdnDecompressResult<()> {
        assert_eq!(self.state, IdnDecompressorState::Uninitialized);

        self.read_header()?;
        self.read_metadata()?;
        self.state = IdnDecompressorState::Reading;

        Ok(())
    }

    fn read_header(&mut self) -> IdnDecompressResult<()> {
        let header = IdnHeader::read(&mut self.reader)?;
        debug!("Read IDN header: {:?}", header);
        if header.version != 1 {
            return Err(IdnDecompressorError::InvalidVersion(header.version));
        }

        Ok(())
    }

    fn read_metadata(&mut self) -> IdnDecompressResult<()> {
        let header = IdnMetadataHeader::read(&mut self.reader)?;
        debug!("Read metadata header: {:?}", header);
        for _ in 0..header.item_num {
            self.read_metadata_item()?;
        }

        let bytes_read = self.reader.position();
        self.options
            .progress_notifier
            .processed_bytes(ByteNum::new(bytes_read as usize));

        Ok(())
    }

    fn read_metadata_item(&mut self) -> IdnDecompressResult<()> {
        let item: IdnMetadataItem = IdnMetadataItem::read(&mut self.reader)?;
        debug!("Read metadata item: {:?}", item);
        match item {
            IdnMetadataItem::Models(models_metadata) => {
                self.handle_models_metadata(models_metadata)?
            }
        }

        Ok(())
    }

    fn handle_models_metadata(
        &mut self,
        models_metadata: IdnModelsMetadata,
    ) -> IdnDecompressResult<()> {
        let identifiers: Vec<ModelIdentifier> = models_metadata
            .model_identifiers
            .into_iter()
            .map_into()
            .collect();
        let options =
            Arc::get_mut(&mut self.options).expect("IdnReaderOptions unexpectedly cloned");
        options
            .model_provider
            .has_all_models(&identifiers)
            .map_err(IdnDecompressorError::unknown_model)?;
        options.model_provider.filter_by_identifiers(&identifiers);
        debug!("Model identifiers:");
        for (index, identifier) in identifiers.iter().enumerate() {
            debug!("[{}] {}", index, identifier);
        }
        options.model_provider.preprocess_decompressor_models();

        Ok(())
    }

    fn read_all(&mut self) -> IdnDecompressResult<()> {
        while self.state.not_finished() {
            let result = self.read_next_block();
            if result.is_err() {
                self.out_state.data_queue.set_finished();
            }
            result?;
        }
        Ok(())
    }

    fn read_next_block(&mut self) -> IdnDecompressResult<()> {
        match self.state {
            IdnDecompressorState::Uninitialized => self.initialize()?,
            IdnDecompressorState::Reading => {}
            IdnDecompressorState::LastBlockReached => return Ok(()),
        }

        trace!("Reading next block");
        let header = IdnBlockHeader::read(&mut self.reader)?;
        let data_len = header.length as usize;
        trace!("Reading block with length {}", data_len);

        {
            let mut data = vec![0; data_len];
            self.reader.read_exact(&mut data)?;

            let current_block = self.current_block;
            let out_state = self.out_state.clone();
            let seq_checksum = header.seq_checksum;
            let options = self.options.clone();

            self.thread_pool.execute(move || {
                let block = IdnBlockDecompressor::new(
                    current_block,
                    data,
                    out_state,
                    seq_checksum,
                    options,
                );
                block.process()?;
                Ok(())
            })?;
        }

        self.current_block += 1;
        if data_len == 0 {
            self.state = IdnDecompressorState::LastBlockReached;
            debug!("End of file block reached");
        }

        Ok(())
    }
}

/// IDN file format decompressor.
#[derive(Debug)]
pub struct IdnDecompressor<R> {
    out_state: Arc<IdnDecompressorOutState>,
    start_time: Instant,
    bytes_decompressed: ByteNum,
    thread_pool: ThreadPool<IdnDecompressorError>,
    sequences_to_get: Vec<FastqSequence>,
    eof_reached: bool,
    inner: Option<IdnDecompressorInner<R>>,
}

impl<R: Read + Send> IdnDecompressor<R> {
    /// Creates a new `IdnDecompressor` instance.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::decompressor::IdnDecompressor;
    ///
    /// let vec = Vec::new();
    /// let mut decompressor = IdnDecompressor::new(vec.as_slice());
    /// assert_eq!(decompressor.next_sequence().is_err(), true);
    /// ```
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self::with_params(reader, IdnDecompressorParams::default())
    }

    /// Creates a new `IdnDecompressor` instance with given params.
    ///
    /// # Examples
    /// ```
    /// use idencomp::idn::decompressor::{IdnDecompressor, IdnDecompressorParams};
    ///
    /// let vec = Vec::new();
    /// let params = IdnDecompressorParams::builder().thread_num(5).build();
    /// let mut decompressor = IdnDecompressor::with_params(vec.as_slice(), params);
    /// assert_eq!(decompressor.next_sequence().is_err(), true);
    /// ```
    #[must_use]
    pub fn with_params(reader: R, params: IdnDecompressorParams) -> Self {
        let start_time = Instant::now();
        let out_state = Arc::new(IdnDecompressorOutState::new());
        let thread_pool = ThreadPool::new(params.thread_num, "idn-decompressor");

        let inner =
            IdnDecompressorInner::new(reader, params, out_state.clone(), thread_pool.make_child());

        let inner = if thread_pool.is_foreground() {
            Some(inner)
        } else {
            let mut inner = inner;
            thread_pool
                .execute(move || {
                    inner.read_all()?;
                    Ok(())
                })
                .expect("Unexpected Thread Pool error");

            None
        };

        Self {
            out_state,
            start_time,
            bytes_decompressed: ByteNum::ZERO,
            thread_pool,
            sequences_to_get: Vec::new(),
            eof_reached: false,
            inner,
        }
    }

    /// Reads and returns next sequence in the file. Returns `Ok(None)` if the
    /// end of file has been reached.
    pub fn next_sequence(&mut self) -> IdnDecompressResult<Option<FastqSequence>> {
        if self.eof_reached {
            return Ok(None);
        }

        let result = self.next_sequence_internal();

        if let Ok(Some(seq)) = &result {
            self.bytes_decompressed += seq.size();
        } else {
            self.eof_reached = true;
            self.thread_pool.join()?;
        }

        result
    }

    fn next_sequence_internal(&mut self) -> IdnDecompressResult<Option<FastqSequence>> {
        if self.sequences_to_get.is_empty() {
            if let Some(inner) = self.inner.as_mut() {
                inner.read_next_block()?;
            }

            self.sequences_to_get = self.out_state.data_queue.retrieve_all();
            if self.sequences_to_get.is_empty() {
                return Ok(None);
            }
            self.sequences_to_get.reverse();
        }

        Ok(Some(self.sequences_to_get.pop().unwrap()))
    }
}

impl<R: Read + Send> IntoIterator for IdnDecompressor<R> {
    type Item = IdnDecompressResult<FastqSequence>;
    type IntoIter = IdnDecompressorIterator<R>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter { decompressor: self }
    }
}

/// Iterable object for [`IdnDecompressor`], returning [`Result`]s of
/// [`FastqSequence`]s.
#[derive(Debug)]
pub struct IdnDecompressorIterator<R> {
    decompressor: IdnDecompressor<R>,
}

impl<R: Read + Send> Iterator for IdnDecompressorIterator<R> {
    type Item = IdnDecompressResult<FastqSequence>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.decompressor.next_sequence();
        match result {
            Ok(val) => val.map(Ok),
            Err(val) => Some(Err(val)),
        }
    }
}

impl<R> IdnDecompressor<R> {
    fn print_stats(&self) {
        info!(
            "Decompressed {}",
            format_stats(self.start_time, self.bytes_decompressed)
        );
    }
}

impl<R> Drop for IdnDecompressor<R> {
    fn drop(&mut self) {
        self.print_stats();

        if !self.eof_reached {
            panic!("Cannot drop IdnDecompressor while still reading");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;
    use std::io::ErrorKind::NotFound;

    use crate::idn::decompressor::IdnDecompressorError;

    #[test]
    fn test_error_display() {
        assert_eq!(
            IdnDecompressorError::InvalidState.to_string(),
            "Invalid decompressor state"
        );
        assert_eq!(
            IdnDecompressorError::from(io::Error::from(NotFound)).to_string(),
            "IO error: entity not found"
        );
        assert_eq!(
            IdnDecompressorError::from(binrw::Error::NoVariantMatch { pos: 0 }).to_string(),
            "Serialize error: no variants matched at 0x0"
        );
        assert_eq!(
            IdnDecompressorError::InvalidVersion(255).to_string(),
            "Invalid IDN file version: 255"
        );
        assert_eq!(
            IdnDecompressorError::block_checksum_mismatch(123, 456).to_string(),
            "Invalid block checksum (actual: 0000007B, expected: 000001C8)"
        );
        assert_eq!(
            IdnDecompressorError::invalid_model_index(12, 5).to_string(),
            "Invalid model index (read: 12, number of active models: 5)"
        );
    }

    #[test]
    fn test_error_source() {
        assert!(IdnDecompressorError::InvalidState.source().is_none());
    }
}
