use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::bail;
use atty::Stream;
use log::info;

#[derive(clap::Args, Debug, Clone)]
pub struct Directory {
    path: PathBuf,
}

impl Display for Directory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

pub fn directory(path: &str) -> Result<Directory, String> {
    let result = Directory {
        path: PathBuf::from(path),
    };

    Ok(result)
}

impl Directory {
    pub fn as_path_buf(&self) -> Result<PathBuf, anyhow::Error> {
        let path = Path::new(&self.path);
        if !path.is_dir() {
            bail!(
                "Provided path: {} does not point to a directory",
                path.display()
            );
        }

        Ok(path.to_path_buf())
    }
}

#[derive(Debug, Clone)]
pub struct InputFile {
    path: PathBuf,
}

impl Display for InputFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

pub fn input_file(path: &str) -> Result<InputFile, String> {
    let output_path = Path::new(path);
    let result = InputFile {
        path: output_path.to_path_buf(),
    };

    Ok(result)
}

impl InputFile {
    pub fn as_reader(&self) -> Result<InputReader, anyhow::Error> {
        InputReader::from_path(&self.path)
    }
}

pub fn input_stream(path: &str) -> Result<InputStream, String> {
    let output_path = Path::new(path);
    let result = InputStream {
        path: output_path.to_path_buf(),
    };

    Ok(result)
}

#[derive(Debug, Clone)]
pub struct InputStream {
    path: PathBuf,
}

impl Display for InputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

impl Default for InputStream {
    fn default() -> Self {
        Self {
            path: PathBuf::from("-"),
        }
    }
}

impl InputStream {
    pub fn as_reader(&self) -> Result<InputReader, anyhow::Error> {
        InputReader::from_path(&self.path)
    }
}

#[derive(Debug)]
pub enum InputReader {
    Stdin(io::Stdin),
    File { file: File, path: PathBuf },
}

impl InputReader {
    fn from_path(path: &Path) -> anyhow::Result<Self> {
        let is_stdin = path.to_string_lossy() == "-";

        let val = if is_stdin {
            Self::Stdin(io::stdin())
        } else {
            let file = File::open(path)?;

            Self::File {
                file,
                path: path.to_owned(),
            }
        };
        Ok(val)
    }

    pub fn reopen_file(&self) -> anyhow::Result<Self> {
        match self {
            InputReader::File { path, .. } => Self::from_path(path),
            _ => panic!("Cannot reopen stdin"),
        }
    }

    pub fn length(&self) -> anyhow::Result<Option<u64>> {
        let val = match self {
            InputReader::Stdin(_) => None,
            InputReader::File { file, .. } => Some(file.metadata()?.len()),
        };
        Ok(val)
    }

    pub fn file_path(&self) -> Option<&Path> {
        match self {
            InputReader::Stdin(_) => None,
            InputReader::File { path, .. } => Some(path),
        }
    }

    #[must_use]
    pub fn into_read(self) -> Box<dyn Read + Send> {
        match self {
            InputReader::Stdin(stdin) => Box::new(stdin),
            InputReader::File { file, .. } => Box::new(file),
        }
    }
}

impl Default for InputReader {
    fn default() -> Self {
        Self::Stdin(io::stdin())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum OutputMode {
    Text,
    Binary,
}

#[derive(Debug)]
pub enum OutputWriter {
    Stdout(io::Stdout),
    File(File),
}

impl OutputWriter {
    pub fn from_path_and_input(
        output: &Option<PathBuf>,
        input: &InputReader,
        new_extension: &str,
        mode: OutputMode,
    ) -> anyhow::Result<Self> {
        if let Some(path) = output {
            Self::from_path(path, mode)
        } else {
            let path = input
                .file_path()
                .map(|path| path.with_extension(new_extension))
                .unwrap_or_else(|| PathBuf::from("-"));

            Self::from_path(&path, mode)
        }
    }

    fn from_path(path: &Path, mode: OutputMode) -> anyhow::Result<Self> {
        info!("Output file: {}", path.display());

        let is_stdout = path.to_string_lossy() == "-";

        if mode == OutputMode::Binary && is_stdout && atty::is(Stream::Stdout) {
            bail!("Cannot output binary file to stdout when running in terminal; please use -o option instead or pipe the standard output");
        }

        let writer = if is_stdout {
            Self::Stdout(io::stdout())
        } else {
            let file = File::create(path)?;
            Self::File(file)
        };

        Ok(writer)
    }

    pub fn into_write(self) -> Box<dyn Write + Send> {
        match self {
            OutputWriter::Stdout(stdout) => Box::new(stdout),
            OutputWriter::File(file) => Box::new(file),
        }
    }
}
