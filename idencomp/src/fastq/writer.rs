use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;

use crate::fastq::consts::{FASTQ_ACID_TO_BYTE, FASTQ_Q_SCORE_TO_BYTE};
use crate::fastq::{
    FastqQualityScore, FastqSequence, FASTQ_QUALITY_SCORE_SEPARATOR, FASTQ_TITLE_PREFIX,
};
use crate::sequence::Acid;

/// Error occurring during serializing a FASTQ file.
#[derive(Debug)]
pub enum FastqWriterError {
    /// I/O error occurred when writing the FASTQ file.
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

/// FASTQ writing parameters that can be set by user.
#[derive(Debug, Clone)]
pub struct FastqWriterParams {
    output_title_with_separator: bool,
}

impl FastqWriterParams {
    /// Returns new builder instance for `FastqWriterParams`.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::writer::FastqWriterParams;
    ///
    /// let params: FastqWriterParams = FastqWriterParams::builder().build();
    /// ```
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

/// A builder for [`FastqWriterParams`].
#[derive(Debug, Clone)]
pub struct FastqWriterParamsBuilder {
    output_title_with_separator: bool,
}

impl FastqWriterParamsBuilder {
    /// Returns a new `FastqWriterParamsBuilder` instance.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::writer::{FastqWriterParams, FastqWriterParamsBuilder};
    ///
    /// let params: FastqWriterParams = FastqWriterParamsBuilder::new().build();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            output_title_with_separator: false,
        }
    }

    /// Whether the FASTQ writer should write the sequence names along with the
    /// separators.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::writer::{FastqWriterParams, FastqWriterParamsBuilder};
    ///
    /// let params: FastqWriterParams = FastqWriterParamsBuilder::new()
    ///     .output_title_with_separator(true)
    ///     .build();
    /// ```
    pub fn output_title_with_separator(&mut self, output_title_with_separator: bool) -> &mut Self {
        let mut new = self;
        new.output_title_with_separator = output_title_with_separator;
        new
    }

    /// Builds the [`FastqWriterParams`] object.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::writer::{FastqWriterParams, FastqWriterParamsBuilder};
    ///
    /// let params: FastqWriterParams = FastqWriterParamsBuilder::new().build();
    /// ```
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

/// A serializer for [`FastqSequence`] objects that outputs the data in the
/// FASTQ format.
#[derive(Debug)]
pub struct FastqWriter<W> {
    writer: W,
    params: FastqWriterParams,
}

impl<W: Write> FastqWriter<W> {
    /// Creates new `FastqWriter` instance with default parameters.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::writer::FastqWriter;
    ///
    /// let mut buf = Vec::new();
    /// let _writer = FastqWriter::new(&mut buf);
    /// ```
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self::with_params(writer, FastqWriterParams::default())
    }

    /// Creates new `FastqWriter` instance with given parameters.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::writer::{FastqWriter, FastqWriterParams};
    ///
    /// let mut buf = Vec::new();
    /// let params = FastqWriterParams::builder()
    ///     .output_title_with_separator(true)
    ///     .build();
    /// let _writer = FastqWriter::with_params(&mut buf, params);
    /// ```
    #[must_use]
    pub fn with_params(writer: W, params: FastqWriterParams) -> Self {
        Self { writer, params }
    }

    /// Writes the sequence as FASTQ.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::writer::FastqWriter;
    /// use idencomp::fastq::{FastqQualityScore, FastqSequence};
    /// # use idencomp::fastq::writer::FastqWriterError;
    /// use idencomp::sequence::{Acid, NucleotideSequenceIdentifier};
    ///
    /// let mut buf = Vec::new();
    /// let mut writer = FastqWriter::new(&mut buf);
    /// let sequence = FastqSequence::new(
    ///     NucleotideSequenceIdentifier::from("seq"),
    ///     [Acid::A],
    ///     [FastqQualityScore::new(5)],
    /// );
    /// writer.write_sequence(&sequence)?;
    ///
    /// # Ok::<(), FastqWriterError>(())
    /// ```
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

    /// Flushes the internal writer object.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::writer::FastqWriter;
    /// # use idencomp::fastq::writer::FastqWriterError;
    ///
    /// let mut buf = Vec::new();
    /// let mut writer = FastqWriter::new(&mut buf);
    /// writer.flush()?;
    ///
    /// # Ok::<(), FastqWriterError>(())
    /// ```
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
        SIMPLE_TEST_SEQUENCE_SEPARATOR_TITLE_STR, SIMPLE_TEST_SEQUENCE_STR,
    };
    use crate::fastq::writer::{FastqWriter, FastqWriterError, FastqWriterParams};

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
    fn should_return_simple_seq_with_title_at_separator() {
        let mut buf = Vec::new();
        let params = FastqWriterParams::builder()
            .output_title_with_separator(true)
            .build();
        FastqWriter::with_params(&mut buf, params)
            .write_sequence(&SIMPLE_TEST_SEQUENCE)
            .unwrap();

        assert_eq!(
            String::from_utf8(buf).unwrap(),
            SIMPLE_TEST_SEQUENCE_SEPARATOR_TITLE_STR
        );
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
