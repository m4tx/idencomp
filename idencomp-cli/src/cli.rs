use std::path::PathBuf;

use clap::{Parser, PossibleValue, Subcommand, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use idencomp::context_spec::ContextSpecType;
use lazy_static::lazy_static;

use crate::cmd::generate_model::GenerateModelMode;
use crate::opts::InputStream;
use crate::opts::{directory, input_file, input_stream, Directory, InputFile};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
pub struct Cli {
    #[clap(flatten)]
    pub verbose: Verbosity<InfoLevel>,

    /// Don't display a progress bar/spinner
    #[clap(long, global = true, value_parser)]
    pub no_progress: bool,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Copy, Clone, Debug)]
pub struct ContextSpecTypeCli {
    pub inner: ContextSpecType,
}

impl ContextSpecTypeCli {
    #[must_use]
    pub fn new(inner: ContextSpecType) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn variants() -> Vec<Self> {
        ContextSpecType::VALUES
            .iter()
            .map(|&inner| ContextSpecTypeCli::new(inner))
            .collect()
    }
}

lazy_static! {
    static ref CTX_SPEC_TYPE_CLI_VARIANTS: Vec<ContextSpecTypeCli> = ContextSpecTypeCli::variants();
}

impl ValueEnum for ContextSpecTypeCli {
    fn value_variants<'a>() -> &'a [Self] {
        &CTX_SPEC_TYPE_CLI_VARIANTS
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue<'a>> {
        let value = PossibleValue::new(self.inner.name());
        Some(value)
    }
}

impl From<&ContextSpecTypeCli> for ContextSpecType {
    fn from(spec_type: &ContextSpecTypeCli) -> Self {
        spec_type.inner
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate a new model using statistics from given FASTQ file
    GenerateModel {
        /// Whether to generate acid model or quality score model
        #[clap(arg_enum, value_parser)]
        mode: GenerateModelMode,

        /// Context spec type to use
        #[clap(arg_enum)]
        context: ContextSpecTypeCli,

        /// Input FASTQ file path
        #[clap(default_value_t, value_parser = input_stream)]
        input: InputStream,

        /// Output file path; `-` is the standard output
        #[clap(short, long, value_parser)]
        output: Option<PathBuf>,

        /// Abort generating model at given number of unique contexts
        /// encountered
        #[clap(default_value_t = 10_000_000, long, value_parser)]
        limit: u32,
    },

    /// Generate all possible models for given FASTQ file
    GenerateModelAll {
        /// Input FASTQ file path
        #[clap(value_parser = input_file)]
        input: InputFile,

        /// Output directory path
        #[clap(value_parser = directory)]
        output: Directory,

        /// Base model name
        #[clap(value_parser)]
        name: String,

        /// Output stats about generated models as a CSV file to the standard
        /// output
        #[clap(long, value_parser)]
        csv: bool,

        /// Abort generating model at given number of unique contexts
        /// encountered
        #[clap(default_value_t = 500_000, long, value_parser)]
        limit: u32,
    },

    /// Make model more compact by combining multiple contexts into one
    BinContexts {
        /// Input model file path
        #[clap(default_value_t, value_parser = input_stream)]
        input: InputStream,

        /// Output file path; `-` is the standard output
        #[clap(short, long, value_parser)]
        output: Option<PathBuf>,

        /// Number of distinct contexts to generate
        #[clap(long, short, value_parser, value_name = "CONTEXT_NUM", value_parser = clap::value_parser!(u32).range(1..))]
        contexts: u32,

        /// Bin the least probable contexts (all above this number) before doing
        /// the proper binning. This harms the generated context quality, but
        /// increases the performance dramatically
        #[clap(long, value_parser, value_name = "CONTEXT_NUM", value_parser = clap::value_parser!(u32).range(1..))]
        pre_bin: Option<u32>,
    },

    /// Generate all possible binned variants for given model
    BinContextsAll {
        /// Input model file path
        #[clap(value_parser = input_stream)]
        input: InputStream,

        /// Output directory path
        #[clap(value_parser = directory)]
        output: Directory,

        /// Base model name
        #[clap(value_parser)]
        name: String,

        /// Maximum number of models to generate
        #[clap(long, short, value_parser, value_name = "MODEL_NUM", value_parser = clap::value_parser!(u32).range(1..))]
        num: Option<u32>,

        /// Bin the least probable contexts (all above this number) before doing
        /// the proper binning. This harms the generated context quality, but
        /// increases the performance dramatically
        #[clap(long, value_parser, value_name = "CONTEXT_NUM", value_parser = clap::value_parser!(u32).range(1..))]
        pre_bin: Option<u32>,

        /// Output stats about generated models as a CSV file to the standard
        /// output
        #[clap(long, value_parser)]
        csv: bool,
    },

    /// Compress a FASTQ file
    Compress {
        /// Input FASTQ file to read; `-` is the standard output
        #[clap(default_value_t, value_parser = input_stream)]
        input: InputStream,

        /// Output IDN file path; `-` is the standard output
        #[clap(short, long, value_parser)]
        output: Option<PathBuf>,

        /// Number of additional threads to spawn
        #[clap(long, value_parser)]
        threads: Option<usize>,

        /// Maximum single block length (expressed as sequence length)
        #[clap(long, value_parser)]
        block_length: Option<usize>,

        /// Do not include sequence identifiers when compressing data
        #[clap(long, value_parser)]
        no_identifiers: bool,

        /// Compression quality (1 - fast, 9 - best)
        #[clap(default_value_t = 7, long, value_parser = clap::value_parser!(u8).range(1..=9))]
        quality: u8,

        /// Make compression as fast as possible. Affects displaying statistics.
        /// Implies --quality=1
        #[clap(long, value_parser)]
        fast: bool,
    },

    /// Decompress an IDN file to FASTQ file
    Decompress {
        /// Input IDN file to read
        #[clap(default_value_t, value_parser = input_stream)]
        input: InputStream,

        /// Output file path; `-` is the standard output
        #[clap(short, long, value_parser)]
        output: Option<PathBuf>,

        /// Number of additional threads to spawn
        #[clap(long, value_parser)]
        threads: Option<usize>,
    },

    /// Print statistics about a FASTQ file
    Stats {
        /// Input FASTQ file to read; `-` is the standard output
        #[clap(default_value_t, value_parser = input_stream)]
        input: InputStream,
    },
}
