use std::ops::RangeInclusive;

use crate::sequence::{Acid, NucleotideSequence, QualityScore};

pub(super) const FASTQ_TITLE_PREFIX: char = '@';
pub(super) const FASTQ_QUALITY_SCORE_SEPARATOR: u8 = b'+';

const FASTQ_QUALITY_SCORE_BYTE_START: u8 = b'!';
const FASTQ_QUALITY_SCORE_BYTE_END: u8 = b'~';

const FASTQ_QUALITY_SCORE_CHAR_START: char = FASTQ_QUALITY_SCORE_BYTE_START as char;
const FASTQ_QUALITY_SCORE_CHAR_END: char = FASTQ_QUALITY_SCORE_BYTE_END as char;
pub(crate) const FASTQ_QUALITY_SCORE_CHARS: RangeInclusive<char> =
    FASTQ_QUALITY_SCORE_CHAR_START..=FASTQ_QUALITY_SCORE_CHAR_END;

const FASTQ_ACID_NUM: usize = 5;

/// Number of distinct quality scores that are possible to be encoded in FASTQ
/// format (i.e. quality score can be in range `0..=FASTQ_Q_END`)
pub const FASTQ_Q_END: usize = 94;

/// Nucleotide sequence that conforms to the FASTQ maximum quality score value
/// (94).
pub type FastqSequence = NucleotideSequence<FASTQ_Q_END>;
/// Quality score that conforms to the FASTQ maximum quality score value (94).
pub type FastqQualityScore = QualityScore<FASTQ_Q_END>;

pub(super) const FASTQ_VALID_ACID_BYTES: [bool; 256] = {
    let mut valid = [false; 256];

    valid[b'A' as usize] = true;
    valid[b'T' as usize] = true;
    valid[b'C' as usize] = true;
    valid[b'G' as usize] = true;
    valid[b'N' as usize] = true;

    valid
};

pub(super) const FASTQ_BYTE_TO_ACID: [Acid; 256] = {
    let mut acids = [Acid::N; 256];

    acids[b'A' as usize] = Acid::A;
    acids[b'T' as usize] = Acid::T;
    acids[b'C' as usize] = Acid::C;
    acids[b'G' as usize] = Acid::G;
    acids[b'N' as usize] = Acid::N;

    acids
};

pub(super) const FASTQ_VALID_Q_SCORE_BYTES: [bool; 256] = {
    let mut valid = [false; 256];

    let mut byte = FASTQ_QUALITY_SCORE_BYTE_START;
    while byte <= FASTQ_QUALITY_SCORE_BYTE_END {
        valid[byte as usize] = true;
        byte += 1;
    }

    valid
};

pub(super) const FASTQ_BYTE_TO_Q_SCORE: [FastqQualityScore; 256] = {
    let mut q_scores = [FastqQualityScore::ZERO; 256];

    let mut byte = FASTQ_QUALITY_SCORE_BYTE_START;
    while byte <= FASTQ_QUALITY_SCORE_BYTE_END {
        q_scores[byte as usize] = FastqQualityScore::new(byte - FASTQ_QUALITY_SCORE_BYTE_START);
        byte += 1;
    }

    q_scores
};

pub(super) const FASTQ_ACID_TO_BYTE: [u8; FASTQ_ACID_NUM] = {
    let mut bytes = [0; FASTQ_ACID_NUM];

    bytes[Acid::A as usize] = b'A';
    bytes[Acid::C as usize] = b'C';
    bytes[Acid::T as usize] = b'T';
    bytes[Acid::G as usize] = b'G';
    bytes[Acid::N as usize] = b'N';

    bytes
};

pub(super) const FASTQ_Q_SCORE_TO_BYTE: [u8; FASTQ_Q_END] = {
    let mut bytes = [0; FASTQ_Q_END];

    let mut value = 0;
    while value < FASTQ_Q_END {
        bytes[value] = FASTQ_QUALITY_SCORE_BYTE_START + (value as u8);
        value += 1;
    }

    bytes
};
