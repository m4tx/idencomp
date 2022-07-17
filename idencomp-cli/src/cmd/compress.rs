use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use idencomp::fastq::reader::FastqReader;
use idencomp::idn::compressor::{CompressionQuality, IdnCompressor, IdnCompressorParams};
use idencomp::idn::model_provider::ModelProvider;
use idencomp::progress::ProgressNotifier;

pub fn compress<R: Read, W: Write + Send>(
    reader: R,
    writer: W,
    threads: Option<usize>,
    block_length: Option<usize>,
    no_identifiers: bool,
    quality: u8,
    progress_notifier: Arc<dyn ProgressNotifier>,
) -> anyhow::Result<()> {
    let fastq_reader = FastqReader::new(BufReader::new(reader));

    let mut params = IdnCompressorParams::builder();
    params
        .model_provider(ModelProvider::from_directory(Path::new("models/"))?)
        .progress_notifier(progress_notifier)
        .quality(CompressionQuality::new(quality))
        .include_identifiers(!no_identifiers);
    if let Some(threads) = threads {
        params.thread_num(threads);
    }
    if let Some(block_length) = block_length {
        params.max_block_total_len(block_length);
    }
    let params = params.build();
    let mut idn_writer = IdnCompressor::with_params(writer, params);

    for sequence in fastq_reader {
        let sequence = sequence.context("Could not parse a sequence from the FASTQ file")?;
        idn_writer
            .add_sequence(sequence)
            .context("Could not write a sequence to the compressed file")?;
    }

    idn_writer.finish()?;

    Ok(())
}
