use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use log::info;

use crate::fastq::FastqSequence;
use crate::idn::common::{format_stats, DataQueue, IdnBlockLock};
use crate::idn::compressor_block::IdnBlockCompressor;
use crate::idn::compressor_initializer::CompressorInitializer;
use crate::idn::model_provider::ModelProvider;
use crate::idn::no_seek::NoSeek;
use crate::idn::thread_pool::ThreadPool;
use crate::idn::writer_idn::IdnWriter;
use crate::progress::{ByteNum, DummyProgressNotifier, ProgressNotifier};

/// Error occurring during compression of an IDN file.
#[derive(Debug, Default)]
pub enum IdnCompressorError {
    /// Invalid compressor state.
    #[default]
    InvalidState,
    /// I/O error occurred when writing the IDN file.
    IoError(std::io::Error),
    /// Error occurred trying to serialize the headers and metadata.
    SerializeError(binrw::Error),
    /// Requested to compress a sequence longer than the configured limit.
    SequenceTooLong(usize, usize),
}

impl IdnCompressorError {
    pub(super) fn sequence_too_long(sequence_len: usize, max_len: usize) -> Self {
        Self::SequenceTooLong(sequence_len, max_len)
    }
}

impl From<std::io::Error> for IdnCompressorError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<binrw::Error> for IdnCompressorError {
    fn from(e: binrw::Error) -> Self {
        Self::SerializeError(e)
    }
}

impl Display for IdnCompressorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IdnCompressorError::InvalidState => write!(f, "Invalid compressor state"),
            IdnCompressorError::IoError(e) => write!(f, "IO error: {}", e),
            IdnCompressorError::SerializeError(e) => write!(f, "Serialize error: {}", e),
            IdnCompressorError::SequenceTooLong(sequence_len, max_len) => write!(
                f,
                "Sequence too long (sequence length: {}, limit: {})",
                sequence_len, max_len
            ),
        }
    }
}

impl Error for IdnCompressorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            IdnCompressorError::IoError(e) => Some(e),
            IdnCompressorError::SerializeError(e) => Some(e),
            _ => None,
        }
    }
}

/// The result of compressing IDN.
pub type IdnCompressResult<T> = Result<T, IdnCompressorError>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct CompressionQuality(u8);

impl CompressionQuality {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        assert!(value >= 1);
        assert!(value <= 9);

        Self(value)
    }

    #[must_use]
    pub const fn get(&self) -> u8 {
        self.0
    }
}

impl Default for CompressionQuality {
    fn default() -> Self {
        Self(7)
    }
}

#[derive(Debug, Clone)]
pub struct IdnCompressorParams {
    model_provider: ModelProvider,
    max_block_total_len: usize,
    progress_notifier: Arc<dyn ProgressNotifier>,
    thread_num: usize,
    include_identifiers: bool,
    quality: CompressionQuality,
    fast: bool,
}

impl IdnCompressorParams {
    pub fn builder() -> IdnCompressorParamsBuilder {
        IdnCompressorParamsBuilder::new()
    }
}

impl Default for IdnCompressorParams {
    fn default() -> Self {
        Self::builder().build()
    }
}

#[derive(Debug, Clone)]
pub struct IdnCompressorParamsBuilder {
    model_provider: ModelProvider,
    max_block_total_len: usize,
    progress_notifier: Arc<dyn ProgressNotifier>,
    thread_num: usize,
    include_identifiers: bool,
    quality: CompressionQuality,
    fast: bool,
}

impl IdnCompressorParamsBuilder {
    pub fn new() -> Self {
        Self {
            model_provider: ModelProvider::default(),
            max_block_total_len: 4 * 1024 * 1024,
            progress_notifier: Arc::new(DummyProgressNotifier),
            thread_num: 0,
            include_identifiers: true,
            quality: CompressionQuality::default(),
            fast: false,
        }
    }

    pub fn model_provider(&mut self, model_provider: ModelProvider) -> &mut Self {
        let mut new = self;
        new.model_provider = model_provider;
        new
    }

    pub fn max_block_total_len(&mut self, max_block_total_len: usize) -> &mut Self {
        let mut new = self;
        new.max_block_total_len = max_block_total_len;
        new
    }

    pub fn progress_notifier(&mut self, progress_notifier: Arc<dyn ProgressNotifier>) -> &mut Self {
        let mut new = self;
        new.progress_notifier = progress_notifier;
        new
    }

    pub fn thread_num(&mut self, thread_num: usize) -> &mut Self {
        let mut new = self;
        new.thread_num = thread_num;
        new
    }

    pub fn include_identifiers(&mut self, include_identifiers: bool) -> &mut Self {
        let mut new = self;
        new.include_identifiers = include_identifiers;
        new
    }

    pub fn quality(&mut self, quality: CompressionQuality) -> &mut Self {
        let mut new = self;
        new.quality = quality;
        new
    }

    pub fn fast(&mut self, fast: bool) -> &mut Self {
        let mut new = self;
        new.fast = fast;
        if fast {
            new.quality = CompressionQuality::new(1);
        }
        new
    }

    pub fn build(&mut self) -> IdnCompressorParams {
        IdnCompressorParams {
            model_provider: self.model_provider.clone(),
            max_block_total_len: self.max_block_total_len,
            progress_notifier: self.progress_notifier.clone(),
            thread_num: self.thread_num,
            include_identifiers: self.include_identifiers,
            quality: self.quality,
            fast: self.fast,
        }
    }
}

impl Default for IdnCompressorParamsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct IdnCompressorOptions {
    pub(super) model_provider: ModelProvider,
    pub(super) progress_notifier: Arc<dyn ProgressNotifier>,
    pub(super) include_identifiers: bool,
    pub(super) quality: CompressionQuality,
    pub(super) fast: bool,
}

impl From<IdnCompressorParams> for IdnCompressorOptions {
    fn from(params: IdnCompressorParams) -> Self {
        Self {
            model_provider: params.model_provider,
            progress_notifier: params.progress_notifier,
            include_identifiers: params.include_identifiers,
            quality: params.quality,
            fast: params.fast,
        }
    }
}

#[derive(Debug)]
pub(super) struct IdnCompressorOutState<W> {
    writer: Mutex<IdnWriter<NoSeek<W>>>,
    block_lock: IdnBlockLock,
}

impl<W: Write> IdnCompressorOutState<W> {
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(IdnWriter::new(NoSeek::new(writer))),
            block_lock: IdnBlockLock::new(),
        }
    }

    pub fn writer(&self) -> MutexGuard<'_, IdnWriter<NoSeek<W>>> {
        self.writer.lock().expect("Could not acquire writer lock")
    }

    pub fn block_lock(&self) -> &IdnBlockLock {
        &self.block_lock
    }
}

type SequenceBlock = Vec<FastqSequence>;

#[derive(Debug)]
struct IdnCompressorInner<W> {
    state: Arc<IdnCompressorOutState<W>>,
    options: Arc<IdnCompressorOptions>,
    current_block: u32,
    initialized: bool,
    thread_pool: ThreadPool<IdnCompressorError>,
    data_queue: Arc<DataQueue<SequenceBlock>>,
    stats: Arc<CompressionStats>,
}

impl<W: Write + Send> IdnCompressorInner<W> {
    #[must_use]
    fn new(
        writer: W,
        params: IdnCompressorParams,
        thread_pool: ThreadPool<IdnCompressorError>,
        data_queue: Arc<DataQueue<SequenceBlock>>,
        stats: Arc<CompressionStats>,
    ) -> Self {
        Self {
            state: Arc::new(IdnCompressorOutState::new(writer)),
            options: Arc::new(params.into()),
            current_block: 0,
            initialized: false,
            thread_pool,
            data_queue,
            stats,
        }
    }

    fn initialize(&mut self, first_block: &SequenceBlock) -> IdnCompressResult<()> {
        let mut writer = self.state.writer();
        let options = Arc::get_mut(&mut self.options).unwrap();
        let initializer = CompressorInitializer::new(&mut writer, options, first_block);
        initializer.initialize()?;
        self.initialized = true;

        Ok(())
    }

    fn write_all_blocks(&mut self) -> IdnCompressResult<()> {
        loop {
            let blocks = self.data_queue.retrieve_all();
            if blocks.is_empty() {
                return Ok(());
            }

            for block in blocks {
                self.write_block(block)?;
            }
        }
    }

    fn write_current_blocks(&mut self) -> IdnCompressResult<()> {
        let blocks = self.data_queue.retrieve_all();

        for block in blocks {
            self.write_block(block)?;
        }

        Ok(())
    }

    fn write_block(&mut self, block: SequenceBlock) -> IdnCompressResult<()> {
        if !self.initialized {
            self.initialize(&block)?;
        }

        {
            let options = self.options.clone();
            let state = self.state.clone();
            let current_block = self.current_block;
            let stats = self.stats.clone();
            self.thread_pool.execute(move || {
                let block = IdnBlockCompressor::new(options, state, current_block, block, stats);
                block.process()?;
                Ok(())
            })?;
        }

        self.current_block += 1;
        Ok(())
    }
}

#[derive(Debug)]
pub struct IdnCompressor<W> {
    // Inner communication
    inner: Option<IdnCompressorInner<W>>,
    thread_pool: ThreadPool<IdnCompressorError>,
    data_queue: Arc<DataQueue<SequenceBlock>>,

    // Options
    max_block_total_len: usize,
    include_identifiers: bool,

    // Current block
    block: SequenceBlock,
    block_length: usize,
}

impl<W: Write + Send> IdnCompressor<W> {
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self::with_params(writer, IdnCompressorParams::default())
    }

    #[must_use]
    pub fn with_params(writer: W, params: IdnCompressorParams) -> Self {
        let max_block_total_len = params.max_block_total_len;
        let include_identifiers = params.include_identifiers;

        let thread_pool = ThreadPool::new(params.thread_num, "idn-compressor");
        let data_queue = Arc::new(DataQueue::new());

        let inner = IdnCompressorInner::new(
            writer,
            params,
            thread_pool.make_child(),
            data_queue.clone(),
            Arc::new(CompressionStats::new()),
        );
        let inner = if thread_pool.is_foreground() {
            Some(inner)
        } else {
            let mut inner = inner;
            thread_pool
                .execute(move || {
                    inner.write_all_blocks()?;
                    Ok(())
                })
                .expect("Unexpected Thread Pool error");

            None
        };

        Self {
            inner,
            thread_pool,
            data_queue,

            max_block_total_len,
            include_identifiers,

            block: SequenceBlock::new(),
            block_length: 0,
        }
    }

    pub fn add_sequence(&mut self, sequence: FastqSequence) -> IdnCompressResult<()> {
        let seq_len = sequence.len();
        if seq_len > self.max_seq_len() {
            return Err(IdnCompressorError::sequence_too_long(
                seq_len,
                self.max_seq_len(),
            ));
        }

        if self.block_length + seq_len > self.max_block_total_len {
            self.make_block()?;
        }

        let sequence = if self.include_identifiers {
            sequence
        } else {
            sequence.with_identifier_discarded()
        };

        self.block.push(sequence);
        self.block_length += seq_len;

        Ok(())
    }

    fn max_seq_len(&self) -> usize {
        self.max_block_total_len / 2
    }

    fn make_block(&mut self) -> IdnCompressResult<()> {
        self.thread_pool.get_status()?;

        let block = mem::take(&mut self.block);
        self.block_length = 0;

        self.data_queue.add(block);

        if let Some(inner) = &mut self.inner {
            inner.write_current_blocks()?;
        }

        Ok(())
    }

    pub fn finish(mut self) -> IdnCompressResult<()> {
        if !self.block.is_empty() {
            self.make_block()?;
        }
        self.make_block()?;

        self.data_queue.set_finished();
        self.thread_pool.join()?;

        Ok(())
    }
}

impl<W> Drop for IdnCompressor<W> {
    fn drop(&mut self) {
        self.thread_pool
            .join()
            .expect("Could not wait for the thread pool to finish");
    }
}

#[derive(Debug)]
pub(super) struct CompressionStats {
    start_time: Instant,

    in_bytes: AtomicUsize,
    in_identifier_bytes: AtomicUsize,
    in_symbols: AtomicUsize,

    out_bytes: AtomicUsize,
    out_identifier_bytes: AtomicUsize,
    out_acid_bytes: AtomicUsize,
    out_q_score_bytes: AtomicUsize,

    blocks: AtomicUsize,
    acid_model_switches: AtomicUsize,
    q_score_model_switches: AtomicUsize,
}

impl CompressionStats {
    #[must_use]
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),

            in_bytes: AtomicUsize::new(0),
            in_identifier_bytes: AtomicUsize::new(0),
            in_symbols: AtomicUsize::new(0),

            out_bytes: AtomicUsize::new(0),
            out_identifier_bytes: AtomicUsize::new(0),
            out_acid_bytes: AtomicUsize::new(0),
            out_q_score_bytes: AtomicUsize::new(0),

            blocks: AtomicUsize::new(0),
            acid_model_switches: AtomicUsize::new(0),
            q_score_model_switches: AtomicUsize::new(0),
        }
    }

    pub fn add_in_bytes(&self, bytes: ByteNum) {
        self.in_bytes.fetch_add(bytes.get(), Ordering::Relaxed);
    }

    pub fn add_in_identifier_bytes(&self, num: usize) {
        self.in_identifier_bytes.fetch_add(num, Ordering::Relaxed);
    }

    pub fn add_in_symbols(&self, num: usize) {
        self.in_symbols.fetch_add(num, Ordering::Relaxed);
    }

    pub fn set_out_bytes(&self, num: usize) {
        self.out_bytes.store(num, Ordering::SeqCst);
    }

    pub fn add_out_identifier_bytes(&self, num: usize) {
        self.out_identifier_bytes.fetch_add(num, Ordering::Relaxed);
    }

    pub fn add_out_acid_bytes(&self, num: usize) {
        self.out_acid_bytes.fetch_add(num, Ordering::Relaxed);
    }

    pub fn add_out_q_score_bytes(&self, num: usize) {
        self.out_q_score_bytes.fetch_add(num, Ordering::Relaxed);
    }

    pub fn inc_blocks(&self) {
        self.blocks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_acid_model_switches(&self, num: usize) {
        self.acid_model_switches.fetch_add(num, Ordering::Relaxed);
    }

    pub fn add_q_score_model_switches(&self, num: usize) {
        self.q_score_model_switches
            .fetch_add(num, Ordering::Relaxed);
    }

    fn print_stats(&self) {
        let in_bytes = self.in_bytes.load(Ordering::SeqCst);
        let in_identifier_bytes = self.in_identifier_bytes.load(Ordering::SeqCst);
        let in_symbols = self.in_symbols.load(Ordering::SeqCst);

        let out_bytes = self.out_bytes.load(Ordering::SeqCst);
        let out_identifier_bytes = self.out_identifier_bytes.load(Ordering::SeqCst);
        let out_acid_bytes = self.out_acid_bytes.load(Ordering::SeqCst);
        let out_q_score_bytes = self.out_q_score_bytes.load(Ordering::SeqCst);

        let blocks = self.blocks.load(Ordering::SeqCst);
        let acid_model_switches = self.acid_model_switches.load(Ordering::SeqCst);
        let q_score_model_switches = self.q_score_model_switches.load(Ordering::SeqCst);

        info!(
            "Compressed {}",
            format_stats(self.start_time, ByteNum::new(in_bytes))
        );
        info!("{} symbols", in_symbols);

        let rate = out_bytes as f32 / in_bytes as f32 * 100.0;
        info!("File: {:>9} -> {:>9} ({:>7.3}%)", in_bytes, out_bytes, rate);

        let header_bytes = out_bytes - out_identifier_bytes - out_acid_bytes - out_q_score_bytes;
        let header_rate = header_bytes as f32 / out_bytes as f32 * 100.0;
        info!(
            "Hder: {:>9} -> {:>9} ({:>7.3}%)",
            out_bytes, header_bytes, header_rate
        );

        let ident_rate = out_identifier_bytes as f32 / in_identifier_bytes as f32 * 100.0;
        let ident_bpv = out_identifier_bytes as f32 * 8.0 / in_identifier_bytes as f32;
        info!(
            "Iden: {:>9} -> {:>9} ({:>7.3}%, {:.3} bpv)",
            in_identifier_bytes, out_identifier_bytes, ident_rate, ident_bpv
        );

        let acid_rate = out_acid_bytes as f32 / in_symbols as f32 * 100.0;
        let acid_bpv = out_acid_bytes as f32 * 8.0 / in_symbols as f32;
        info!(
            "Acid: {:>9} -> {:>9} ({:>7.3}%, {:.3} bpv)",
            in_symbols, out_acid_bytes, acid_rate, acid_bpv
        );

        let q_score_rate = out_q_score_bytes as f32 / in_symbols as f32 * 100.0;
        let q_score_bpv = out_q_score_bytes as f32 * 8.0 / in_symbols as f32;
        info!(
            "QScr: {:>9} -> {:>9} ({:>7.3}%, {:.3} bpv)",
            in_symbols, out_q_score_bytes, q_score_rate, q_score_bpv
        );

        info!("{} blocks", blocks);
        info!("{} acid model switches", acid_model_switches);
        info!("{} q score model switches", q_score_model_switches);
    }
}

impl Drop for CompressionStats {
    fn drop(&mut self) {
        self.print_stats();
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;
    use std::io::ErrorKind::NotFound;

    use crate::_internal_test_data::SHORT_TEST_SEQUENCE;
    use crate::idn::compressor::{IdnCompressor, IdnCompressorError, IdnCompressorParams};

    #[test]
    fn test_sequence_too_long() {
        let options = IdnCompressorParams::builder()
            .max_block_total_len(1)
            .build();

        let mut data = Vec::new();
        let mut writer = IdnCompressor::with_params(&mut data, options);

        let error = writer
            .add_sequence(SHORT_TEST_SEQUENCE.clone())
            .unwrap_err();
        writer.finish().unwrap();

        assert!(matches!(error, IdnCompressorError::SequenceTooLong(4, _)));
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            format!("{}", IdnCompressorError::InvalidState),
            "Invalid compressor state"
        );
        assert_eq!(
            format!("{}", IdnCompressorError::from(io::Error::from(NotFound))),
            "IO error: entity not found"
        );
        assert_eq!(
            format!(
                "{}",
                IdnCompressorError::from(binrw::Error::NoVariantMatch { pos: 0 })
            ),
            "Serialize error: no variants matched at 0x0"
        );
        assert_eq!(
            format!("{}", IdnCompressorError::sequence_too_long(5, 2)),
            "Sequence too long (sequence length: 5, limit: 2)"
        );
    }

    #[test]
    fn test_error_source() {
        assert!(IdnCompressorError::InvalidState.source().is_none());
    }
}
