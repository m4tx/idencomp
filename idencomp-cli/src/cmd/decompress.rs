use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use idencomp::fastq::writer::FastqWriter;
use idencomp::idn::decompressor::{IdnDecompressor, IdnDecompressorParams};
use idencomp::idn::model_provider::ModelProvider;
use idencomp::progress::ProgressNotifier;

pub fn decompress<R: Read + Send, W: Write>(
    reader: R,
    writer: W,
    threads: Option<usize>,
    progress_notifier: Arc<dyn ProgressNotifier>,
) -> anyhow::Result<()> {
    let mut params = IdnDecompressorParams::builder();
    params
        .model_provider(ModelProvider::from_directory(Path::new("models/"))?)
        .progress_notifier(progress_notifier);
    if let Some(threads) = threads {
        params.thread_num(threads);
    }
    let params = params.build();
    let idn_reader = IdnDecompressor::with_params(reader, params);

    let mut fastq_writer = FastqWriter::new(BufWriter::new(writer));

    for sequence in idn_reader {
        let sequence = sequence.context("Could not read a sequence from the compressed file")?;
        fastq_writer
            .write_sequence(&sequence)
            .context("Could not write a sequence to the FASTQ file")?;
    }

    fastq_writer.flush()?;

    Ok(())
}
