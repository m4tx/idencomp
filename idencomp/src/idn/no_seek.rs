use std::fmt::Debug;
use std::io::{Error, ErrorKind, Read, Seek, Write};

/// Wrapper over a [`std::io::Read`] or [`std::io::Write`] object that provides
/// a dummy [`std::io::Seek`] implementation.
///
/// The [`Seek`] implementation does nothing for no-op seeks, and
/// returns errors otherwise. This may be useful for libraries/functions that
/// require [`Seek`], but are only doing no-op seeks in some specific
/// cases.
#[derive(Debug)]
pub struct NoSeek<T> {
    inner: T,
    position: u64,
}

impl<T> NoSeek<T> {
    /// Constructs a new [`NoSeek<T>`] object.
    ///
    /// # Examples
    /// ```
    /// use std::io::{Seek, SeekFrom};
    ///
    /// use idencomp::idn::no_seek::NoSeek;
    ///
    /// let data: Vec<u8> = Vec::new();
    /// let mut reader = NoSeek::new(&data);
    ///
    /// assert!(reader.seek(SeekFrom::Start(0)).is_ok());
    /// assert!(reader.seek(SeekFrom::Start(1)).is_err());
    /// ```
    pub fn new(inner: T) -> Self {
        Self { inner, position: 0 }
    }

    /// Returns the position of this [`NoSeek<T>`] object.
    ///
    /// # Examples
    /// ```
    /// use std::io::{Seek, SeekFrom};
    ///
    /// use idencomp::idn::no_seek::NoSeek;
    ///
    /// let data: Vec<u8> = Vec::new();
    /// let mut reader = NoSeek::new(&data);
    ///
    /// assert_eq!(reader.position(), 0);
    /// ```
    pub fn position(&self) -> u64 {
        self.position
    }

    fn seek_error() -> Error {
        Error::new(ErrorKind::Other, "Non-noop seek on a NoSeek object")
    }
}

impl<T> Seek for NoSeek<T> {
    #[inline]
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Start(i) => {
                if i == self.position {
                    Ok(self.position)
                } else {
                    Err(Self::seek_error())
                }
            }
            std::io::SeekFrom::End(_) => unimplemented!(),
            std::io::SeekFrom::Current(i) => {
                if i == 0 {
                    Ok(self.position)
                } else {
                    Err(Self::seek_error())
                }
            }
        }
    }
}

impl<R: Read> Read for NoSeek<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let size = self.inner.read(buf)?;
        self.position += size as u64;
        Ok(size)
    }

    #[inline]
    fn read_vectored(&mut self, bufs: &mut [std::io::IoSliceMut<'_>]) -> std::io::Result<usize> {
        let size = self.inner.read_vectored(bufs)?;
        self.position += size as u64;
        Ok(size)
    }
}

impl<W: Write> Write for NoSeek<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let size = self.inner.write(buf)?;
        self.position += size as u64;
        Ok(size)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        let size = self.inner.write_vectored(bufs)?;
        self.position += size as u64;
        Ok(size)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
