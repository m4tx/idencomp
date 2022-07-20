#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use cli::{Cli, Commands};
use cmd::{bin_contexts, bin_contexts_all, compress, decompress, generate_model, stats};
use human_panic::setup_panic;
use lazy_static::lazy_static;

use crate::logging::init_logging;
use crate::opts::{OutputMode, OutputWriter};
use crate::progress_bar::IdnProgressBar;

mod cli;
mod cmd;
mod csv_stat;
mod logging;
mod opts;
mod progress_bar;

lazy_static! {
    pub(crate) static ref PROGRESS_BAR: IdnProgressBar = IdnProgressBar::new();
}

fn main() -> anyhow::Result<()> {
    setup_panic!();

    let cli: Cli = Cli::parse();

    if !cli.no_progress {
        PROGRESS_BAR.show();
    }

    init_logging(cli.verbose.log_level_filter()).expect("Could not initialize logging");

    match &cli.command {
        Commands::GenerateModel {
            input,
            output,
            context,
            mode,
            limit,
        } => {
            let reader = input.as_reader()?;
            let output =
                OutputWriter::from_path_and_input(output, &reader, "msgpack", OutputMode::Binary)?;

            let generator = generate_model::CliModelGenerator::new(reader, false, *limit);
            generator
                .generate_model(output.into_write(), *mode, context.into())
                .context("Failed to generate a model for given FASTQ file")?;
        }
        Commands::GenerateModelAll {
            input,
            output,
            name,
            csv,
            limit,
        } => {
            let reader = input.as_reader()?;

            let generator = generate_model::CliModelGenerator::new(reader, *csv, *limit);
            generator
                .generate_model_all(&output.as_path_buf()?, name)
                .context("Failed to generate a model for given FASTQ file")?;
        }
        Commands::BinContexts {
            input,
            output,
            contexts,
            pre_bin,
        } => {
            let reader = input.as_reader()?;
            let output =
                OutputWriter::from_path_and_input(output, &reader, "msgpack", OutputMode::Binary)?;

            bin_contexts::bin_contexts(
                reader.into_read(),
                output.into_write(),
                *contexts as usize,
                pre_bin.map(|x| x as usize),
            )
            .context("Failed to bin contexts of given model")?;
        }
        Commands::BinContextsAll {
            input,
            output,
            name,
            num,
            pre_bin,
            csv,
        } => {
            let reader = input.as_reader()?;

            bin_contexts_all::bin_contexts_all(
                reader.into_read(),
                &output.as_path_buf()?,
                name,
                num.map(|x| x as usize),
                pre_bin.map(|x| x as usize),
                *csv,
            )
            .context("Failed to bin contexts of given model")?;
        }
        Commands::Compress {
            input,
            output,
            threads,
            block_length,
            no_identifiers,
            quality,
        } => {
            let reader = input.as_reader()?;
            PROGRESS_BAR.set_total_bytes(reader.length()?.unwrap_or(0));
            let output =
                OutputWriter::from_path_and_input(output, &reader, "idn", OutputMode::Binary)?;

            compress::compress(
                reader.into_read(),
                output.into_write(),
                *threads,
                *block_length,
                *no_identifiers,
                *quality,
                Arc::new(PROGRESS_BAR.clone()),
            )
            .context("Failed to compress given file")?;
        }
        Commands::Decompress {
            input,
            output,
            threads,
        } => {
            let reader = input.as_reader()?;
            PROGRESS_BAR.set_total_bytes(reader.length()?.unwrap_or(0));
            let output =
                OutputWriter::from_path_and_input(output, &reader, "fastq", OutputMode::Text)?;

            decompress::decompress(
                reader.into_read(),
                output.into_write(),
                *threads,
                Arc::new(PROGRESS_BAR.clone()),
            )
            .context("Failed to decompress given file")?;
        }
        Commands::Stats { input } => {
            let reader = input.as_reader()?;
            PROGRESS_BAR.set_total_bytes(reader.length()?.unwrap_or(0));

            stats::stats(reader.into_read()).context("Failed to compute file statistics")?;
        }
    }

    PROGRESS_BAR.finish();
    Ok(())
}
