use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;

use crate::fastq::consts::{FASTQ_ACID_TO_BYTE, FASTQ_Q_SCORE_TO_BYTE};
use crate::fastq::{
    FastqQualityScore, FastqSequence, FASTQ_QUALITY_SCORE_SEPARATOR, FASTQ_TITLE_PREFIX,
};
use crate::sequence::Acid;

#[derive(Debug)]
pub enum FastqWriterError {
    IoError(std::io::Error),
}

impl From<std::io::Error> for FastqWriterError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl Display for FastqWriterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FastqWriterError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl Error for FastqWriterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FastqWriterError::IoError(e) => Some(e),
        }
    }
}

type FastqWriteResult<T> = Result<T, FastqWriterError>;

#[derive(Debug, Clone)]
pub struct FastqWriterParams {
    output_title_with_separator: bool,
}

impl FastqWriterParams {
    #[must_use]
    pub fn builder() -> FastqWriterParamsBuilder {
        FastqWriterParamsBuilder::new()
    }
}

impl Default for FastqWriterParams {
    fn default() -> Self {
        FastqWriterParamsBuilder::default().build()
    }
}

#[derive(Debug, Clone)]
pub struct FastqWriterParamsBuilder {
    output_title_with_separator: bool,
}

impl FastqWriterParamsBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            output_title_with_separator: false,
        }
    }

    pub fn output_title_with_separator(&mut self, output_title_with_separator: bool) -> &mut Self {
        let mut new = self;
        new.output_title_with_separator = output_title_with_separator;
        new
    }

    #[must_use]
    pub fn build(&self) -> FastqWriterParams {
        FastqWriterParams {
            output_title_with_separator: self.output_title_with_separator,
        }
    }
}

impl Default for FastqWriterParamsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct FastqWriter<W> {
    writer: W,
    params: FastqWriterParams,
}

impl<W: Write> FastqWriter<W> {
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self::with_params(writer, FastqWriterParams::default())
    }

    #[must_use]
    pub fn with_params(writer: W, params: FastqWriterParams) -> Self {
        Self { writer, params }
    }

    pub fn write_sequence(&mut self, fastq_sequence: &FastqSequence) -> FastqWriteResult<()> {
        self.output_title(fastq_sequence)?;
        self.output_acids(fastq_sequence.acids())?;
        self.output_quality_scores_separator(&fastq_sequence.identifier().0)?;
        self.output_quality_scores(fastq_sequence.quality_scores())?;

        Ok(())
    }

    fn output_title(&mut self, fastq_sequence: &FastqSequence) -> FastqWriteResult<()> {
        writeln!(
            &mut self.writer,
            "{}{}",
            FASTQ_TITLE_PREFIX,
            fastq_sequence.identifier()
        )?;

        Ok(())
    }

    fn output_acids(&mut self, acids: &[Acid]) -> FastqWriteResult<()> {
        let mut data = Vec::with_capacity(acids.len());
        for &acid in acids {
            data.push(FASTQ_ACID_TO_BYTE[acid as usize]);
        }
        self.writer.write_all(&data)?;
        writeln!(&mut self.writer)?;

        Ok(())
    }

    fn output_quality_scores_separator(&mut self, identifier: &str) -> FastqWriteResult<()> {
        write!(
            &mut self.writer,
            "{}",
            FASTQ_QUALITY_SCORE_SEPARATOR as char
        )?;
        if self.params.output_title_with_separator {
            write!(&mut self.writer, "{}", identifier)?;
        }
        writeln!(&mut self.writer)?;

        Ok(())
    }

    fn output_quality_scores(
        &mut self,
        quality_scores: &[FastqQualityScore],
    ) -> FastqWriteResult<()> {
        let mut data = Vec::with_capacity(quality_scores.len());
        for &quality_score in quality_scores {
            data.push(FASTQ_Q_SCORE_TO_BYTE[quality_score.get()]);
        }
        self.writer.write_all(&data)?;
        writeln!(&mut self.writer)?;

        Ok(())
    }

    pub fn flush(&mut self) -> FastqWriteResult<()> {
        self.writer.flush()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::ErrorKind::NotFound;

    use crate::_internal_test_data::{
        EMPTY_TEST_SEQUENCE, EMPTY_TEST_SEQUENCE_STR, SEQ_1M, SEQ_1M_FASTQ, SIMPLE_TEST_SEQUENCE,
        SIMPLE_TEST_SEQUENCE_STR,
    };
    use crate::fastq::writer::{FastqWriter, FastqWriterError};

    #[test]
    fn should_return_empty_seq() {
        let string = EMPTY_TEST_SEQUENCE_STR;

        let mut buf = Vec::new();
        FastqWriter::new(&mut buf)
            .write_sequence(&EMPTY_TEST_SEQUENCE)
            .unwrap();

        assert_eq!(String::from_utf8(buf).unwrap(), string);
    }

    #[test]
    fn test_writer_cloned() {
        let string = EMPTY_TEST_SEQUENCE_STR;

        let mut buf = Vec::new();
        FastqWriter::new(&mut buf)
            .write_sequence(&EMPTY_TEST_SEQUENCE)
            .unwrap();

        assert_eq!(String::from_utf8(buf).unwrap(), string);
    }

    #[test]
    fn should_return_simple_seq() {
        let mut buf = Vec::new();
        FastqWriter::new(&mut buf)
            .write_sequence(&SIMPLE_TEST_SEQUENCE)
            .unwrap();

        assert_eq!(String::from_utf8(buf).unwrap(), SIMPLE_TEST_SEQUENCE_STR);
    }

    #[test]
    fn test_write_1mb() {
        let mut buf = Vec::new();
        FastqWriter::new(&mut buf).write_sequence(&SEQ_1M).unwrap();

        assert_eq!(buf, SEQ_1M_FASTQ);
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            format!("{}", FastqWriterError::from(std::io::Error::from(NotFound))),
            "IO error: entity not found"
        )
    }

    #[test]
    fn test_error_source() {
        assert!(FastqWriterError::from(std::io::Error::from(NotFound))
            .source()
            .is_some());
    }
}
