use std::io::{BufReader, Read};

use anyhow::Context;
use idencomp::fastq::reader::FastqReader;
use idencomp::fastq::{FastqQualityScore, FastqSequence};
use idencomp::model_generator::ContextCounter;
use idencomp::progress::ProgressNotifier;
use idencomp::sequence::{Acid, Symbol};

use crate::PROGRESS_BAR;

pub(crate) fn stats<R: Read>(reader: R) -> anyhow::Result<()> {
    let fastq_reader = FastqReader::new(BufReader::new(reader));
    let mut stats = FastqStats::new();

    for sequence in fastq_reader {
        let sequence = sequence.context("Could not parse a sequence from the FASTQ file")?;

        stats.process_sequence(&sequence);
        PROGRESS_BAR.processed_bytes(sequence.size());
    }

    PROGRESS_BAR.finish();

    stats.print_acid_stats();
    eprintln!();
    stats.print_q_score_stats();

    Ok(())
}

#[derive(Debug)]
struct FastqStats {
    acid_counter: ContextCounter<Acid>,
    q_score_counter: ContextCounter<FastqQualityScore>,
}

impl FastqStats {
    pub fn new() -> Self {
        Self {
            acid_counter: ContextCounter::new(),
            q_score_counter: ContextCounter::new(),
        }
    }

    pub fn process_sequence(&mut self, sequence: &FastqSequence) {
        for &acid in sequence.acids() {
            self.acid_counter.add(acid);
        }

        for &quality_score in sequence.quality_scores() {
            self.q_score_counter.add(quality_score);
        }
    }

    pub fn print_acid_stats(&self) {
        eprintln!("Acids:");
        for acid in Acid::values() {
            eprintln!(
                "  {}: {:.4}%",
                acid,
                self.acid_counter.percentage(acid) * 100.0,
            );
        }
    }

    pub fn print_q_score_stats(&self) {
        eprintln!("Quality Scores:");
        for quality_score in FastqQualityScore::values() {
            eprintln!(
                "  {}: {:.4}%",
                quality_score.get(),
                self.q_score_counter.percentage(quality_score) * 100.0,
            );
        }
    }
}
