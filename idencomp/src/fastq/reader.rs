use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::io::BufRead;

use crate::fastq::consts::{
    FASTQ_BYTE_TO_ACID, FASTQ_BYTE_TO_Q_SCORE, FASTQ_VALID_ACID_BYTES, FASTQ_VALID_Q_SCORE_BYTES,
};
use crate::fastq::{
    FastqQualityScore, FastqSequence, FASTQ_QUALITY_SCORE_SEPARATOR, FASTQ_TITLE_PREFIX,
};
use crate::progress::ByteNum;
use crate::sequence::Acid;

/// Error occurring during parsing a FASTQ file.
#[derive(Debug)]
pub enum FastqReaderError {
    /// I/O error occurred when reading the FASTQ file.
    IoError(std::io::Error),
    /// End-Of-File reached in the middle of reading the file.
    EofReached,
    /// Not a valid FASTQ file.
    InvalidFormat,
    /// Invalid acid character.
    InvalidAcid(char),
    /// Invalid quality score character.
    InvalidQualityScore(char),
    /// The length of acids and quality scores is not equal.
    AcidAndQualityScoreLengthMismatch,
}

impl From<std::io::Error> for FastqReaderError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl Display for FastqReaderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FastqReaderError::IoError(e) => write!(f, "IO error: {}", e),
            FastqReaderError::EofReached => write!(f, "Reached the end of file"),
            FastqReaderError::InvalidFormat => write!(f, "Invalid format"),
            FastqReaderError::InvalidAcid(ch) => write!(f, "Invalid acid: `{}`", ch),
            FastqReaderError::InvalidQualityScore(ch) => {
                write!(f, "Invalid quality score: `{}`", ch)
            }
            FastqReaderError::AcidAndQualityScoreLengthMismatch => {
                write!(f, "Acid and quality score length mismatch")
            }
        }
    }
}

impl Error for FastqReaderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FastqReaderError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

/// The result of a FASTQ reading operation.
pub type FastqResult<T> = Result<T, FastqReaderError>;

/// A builder for `FastqReaderParams`.
#[derive(Debug, Clone)]
pub struct FastqReaderParamsBuilder {
    delimiter: u8,
}

impl FastqReaderParamsBuilder {
    /// Returns a new instance of `FastqReaderParamsBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self { delimiter: b'\n' }
    }

    /// Sets the delimiter character to use instead of a newline.
    pub fn delimiter(&mut self, delimiter: u8) -> &mut Self {
        let mut new = self;
        new.delimiter = delimiter;
        new
    }

    /// Builds and returns [`FastqReaderParams`].
    pub fn build(&self) -> FastqReaderParams {
        FastqReaderParams {
            delimiter: self.delimiter,
        }
    }
}

impl Default for FastqReaderParamsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// FASTQ reading params.
#[derive(Debug, Clone)]
pub struct FastqReaderParams {
    delimiter: u8,
}

impl FastqReaderParams {
    /// Returns new builder for `FastqReaderParams`.
    #[must_use]
    pub fn builder() -> FastqReaderParamsBuilder {
        FastqReaderParamsBuilder::new()
    }
}

impl Default for FastqReaderParams {
    fn default() -> Self {
        FastqReaderParamsBuilder::default().build()
    }
}

/// FASTQ format reader capable of deserializing the sequences into
/// [`FastqSequence`] objects.
#[derive(Debug)]
pub struct FastqReader<R> {
    reader: R,
    params: FastqReaderParams,
    bytes_read: usize,
    buffer: Vec<u8>,
}

impl<R: BufRead> FastqReader<R> {
    /// Creates new `FastqReader` instance with default parameters.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::reader::FastqReader;
    ///
    /// let buf = Vec::new();
    /// let _reader = FastqReader::new(buf.as_slice());
    /// ```
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self::with_params(reader, FastqReaderParams::default())
    }

    /// Creates new `FastqReader` instance with given parameters.
    ///
    /// # Examples
    /// ```
    /// use idencomp::fastq::reader::{FastqReader, FastqReaderParams};
    ///
    /// let buf = Vec::new();
    /// let params = FastqReaderParams::builder().delimiter(b'#').build();
    /// let _reader = FastqReader::with_params(buf.as_slice(), params);
    /// ```
    #[must_use]
    pub fn with_params(reader: R, params: FastqReaderParams) -> Self {
        Self {
            reader,
            params,
            bytes_read: 0,
            buffer: Vec::with_capacity(4096),
        }
    }

    /// Reads a single FASTQ file from given reader.
    pub fn read_sequence(&mut self) -> FastqResult<FastqSequence> {
        self.bytes_read = 0;
        let title = self.parse_title()?;
        let acids = self.parse_acids()?;
        self.parse_separator()?;
        let quality_scores = self.parse_quality_scores()?;

        if acids.len() != quality_scores.len() {
            return Err(FastqReaderError::AcidAndQualityScoreLengthMismatch);
        }

        let seq =
            FastqSequence::with_size(title, acids, quality_scores, ByteNum::new(self.bytes_read));
        Ok(seq)
    }

    /// Reads the title from given FASTQ file.
    pub fn parse_title(&mut self) -> FastqResult<String> {
        let line = loop {
            let line = Self::read_line(
                &mut self.reader,
                self.params.delimiter,
                &mut self.buffer,
                &mut self.bytes_read,
            )?;
            let line = String::from_utf8_lossy(line);

            if !line.trim().is_empty() {
                break line;
            }
        };

        if !line.starts_with(FASTQ_TITLE_PREFIX) {
            return Err(FastqReaderError::InvalidFormat);
        }

        let title = line[1..].trim().to_owned();
        Ok(title)
    }

    /// Reads the acid list from given FASTQ file.
    pub fn parse_acids(&mut self) -> FastqResult<Vec<Acid>> {
        let line = Self::read_line(
            &mut self.reader,
            self.params.delimiter,
            &mut self.buffer,
            &mut self.bytes_read,
        )?;

        let mut acids = Vec::with_capacity(line.len());
        for &ch in line {
            if FASTQ_VALID_ACID_BYTES[ch as usize] {
                acids.push(FASTQ_BYTE_TO_ACID[ch as usize]);
            } else {
                return Err(FastqReaderError::InvalidAcid(ch as char));
            }
        }

        Ok(acids)
    }

    /// Reads acid-quality score separator from given FASTQ file.
    pub fn parse_separator(&mut self) -> FastqResult<()> {
        let line = Self::read_line(
            &mut self.reader,
            self.params.delimiter,
            &mut self.buffer,
            &mut self.bytes_read,
        )?;
        if line.is_empty() || line[0] != FASTQ_QUALITY_SCORE_SEPARATOR {
            return Err(FastqReaderError::InvalidFormat);
        }

        Ok(())
    }

    /// Reads the quality score list from given FASTQ file.
    pub fn parse_quality_scores(&mut self) -> FastqResult<Vec<FastqQualityScore>> {
        let line = Self::read_line(
            &mut self.reader,
            self.params.delimiter,
            &mut self.buffer,
            &mut self.bytes_read,
        )?;
        let mut quality_scores = Vec::with_capacity(line.len());

        for &ch in line {
            if FASTQ_VALID_Q_SCORE_BYTES[ch as usize] {
                quality_scores.push(FASTQ_BYTE_TO_Q_SCORE[ch as usize]);
            } else {
                return Err(FastqReaderError::InvalidQualityScore(ch as char));
            }
        }

        Ok(quality_scores)
    }

    fn read_line<'a, T: BufRead>(
        mut buf_reader: T,
        delimiter: u8,
        buffer: &'a mut Vec<u8>,
        total_bytes_read: &mut usize,
    ) -> FastqResult<&'a [u8]> {
        buffer.clear();
        let bytes_read = buf_reader.read_until(delimiter, buffer)?;
        if bytes_read == 0 {
            return Err(FastqReaderError::EofReached);
        }
        *total_bytes_read += bytes_read;

        let mut buffer = buffer.as_slice();
        while buffer.last().copied() == Some(delimiter) {
            buffer = &buffer[..buffer.len() - 1];
        }

        Ok(buffer)
    }
}

impl<R: BufRead> IntoIterator for FastqReader<R> {
    type Item = FastqResult<FastqSequence>;
    type IntoIter = FastqReaderIterator<R>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            reader: self,
            no_errors: true,
        }
    }
}

/// Iterator implementation for [`FastqReader`] which iterates over all
/// sequences in a file.
#[derive(Debug)]
pub struct FastqReaderIterator<R> {
    reader: FastqReader<R>,
    no_errors: bool,
}

impl<R: BufRead> Iterator for FastqReaderIterator<R> {
    type Item = FastqResult<FastqSequence>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.no_errors {
            return None;
        }

        let result = self.reader.read_sequence();
        if result.is_err() {
            self.no_errors = false;
            if matches!(result, Err(FastqReaderError::EofReached)) {
                return None;
            }
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::ErrorKind::NotFound;

    use crate::_internal_test_data::{
        EMPTY_TEST_SEQUENCE, EMPTY_TEST_SEQUENCE_STR, SEQ_1K_READS_FASTQ, SEQ_1M_FASTQ,
        SIMPLE_TEST_SEQUENCE, SIMPLE_TEST_SEQUENCE_STR,
    };
    use crate::fastq::reader::{FastqReader, FastqReaderError};

    #[test]
    fn should_return_empty_seq() {
        let reader = EMPTY_TEST_SEQUENCE_STR.as_bytes();
        let sequence = FastqReader::new(reader).read_sequence().unwrap();

        assert_eq!(sequence, *EMPTY_TEST_SEQUENCE)
    }

    #[test]
    fn should_return_invalid_acid_error() {
        let reader = "@seq
X
+
!"
        .as_bytes();
        let sequence = FastqReader::new(reader).read_sequence().unwrap_err();

        assert!(matches!(sequence, FastqReaderError::InvalidAcid('X')));
    }

    #[test]
    fn should_return_invalid_quality_score_error() {
        let reader = "@seq
A
+
\x07"
            .as_bytes();
        let sequence = FastqReader::new(reader).read_sequence().unwrap_err();

        assert!(matches!(
            sequence,
            FastqReaderError::InvalidQualityScore('\x07')
        ));
    }

    #[test]
    fn should_return_acid_and_quality_score_length_mismatch_error() {
        let reader = "@seq
A
+
123"
        .as_bytes();
        let sequence = FastqReader::new(reader).read_sequence().unwrap_err();

        assert!(matches!(
            sequence,
            FastqReaderError::AcidAndQualityScoreLengthMismatch
        ));
    }

    #[test]
    fn test_read_1k_reads() {
        let reader = FastqReader::new(SEQ_1K_READS_FASTQ);
        let result: Result<Vec<_>, _> = reader.into_iter().collect();
        let sequences = result.unwrap();

        assert_eq!(sequences.len(), 1000);
        assert!(sequences.iter().all(|seq| !seq.identifier().is_empty()));
        assert!(sequences.iter().all(|seq| seq.len() == 76));
    }

    #[test]
    fn test_read_1mb() {
        let mut reader = FastqReader::new(SEQ_1M_FASTQ);
        let sequence = reader.read_sequence().unwrap();

        assert_eq!(sequence.len(), 500000);
    }

    #[test]
    fn read_returns_simple_seq() {
        let sequence = FastqReader::new(SIMPLE_TEST_SEQUENCE_STR.as_bytes())
            .read_sequence()
            .unwrap();

        assert_eq!(sequence, *SIMPLE_TEST_SEQUENCE);
    }

    #[test]
    fn read_all_returns_empty_iterator_for_empty_file() {
        let reader = "".as_bytes();
        let vec: Vec<_> = FastqReader::new(reader).into_iter().collect();

        assert!(vec.is_empty(), "results not empty: {:?}", vec);
    }

    #[test]
    fn read_all_returns_empty_iterator_for_empty_line() {
        let reader = "\n".as_bytes();
        let vec: Vec<_> = FastqReader::new(reader).into_iter().collect();

        assert!(vec.is_empty(), "results not empty: {:?}", vec);
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            format!("{}", FastqReaderError::from(std::io::Error::from(NotFound))),
            "IO error: entity not found"
        );
        assert_eq!(
            format!("{}", FastqReaderError::EofReached),
            "Reached the end of file"
        );
        assert_eq!(
            format!("{}", FastqReaderError::InvalidFormat),
            "Invalid format"
        );
        assert_eq!(
            format!("{}", FastqReaderError::InvalidAcid('#')),
            "Invalid acid: `#`"
        );
        assert_eq!(
            format!("{}", FastqReaderError::InvalidQualityScore(' ')),
            "Invalid quality score: ` `"
        );
        assert_eq!(
            format!("{}", FastqReaderError::AcidAndQualityScoreLengthMismatch),
            "Acid and quality score length mismatch"
        );
    }

    #[test]
    fn test_error_source() {
        assert!(FastqReaderError::from(std::io::Error::from(NotFound))
            .source()
            .is_some());
        assert!(FastqReaderError::EofReached.source().is_none());
        assert!(FastqReaderError::InvalidFormat.source().is_none());
        assert!(FastqReaderError::InvalidAcid('#').source().is_none());
        assert!(FastqReaderError::InvalidQualityScore(' ')
            .source()
            .is_none());
        assert!(FastqReaderError::AcidAndQualityScoreLengthMismatch
            .source()
            .is_none());
    }
}
