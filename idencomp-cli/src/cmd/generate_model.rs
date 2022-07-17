use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::mem;
use std::path::Path;

use clap::ArgEnum;
use idencomp::context_spec::ContextSpecType;
use idencomp::fastq::reader::FastqReader;
use idencomp::fastq::FastqQualityScore;
use idencomp::model::{CompressionRate, Model, ModelType};
use idencomp::model_generator::ModelGenerator;
use idencomp::model_serializer::SerializableModel;
use idencomp::progress::{ByteNum, ProgressNotifier};
use idencomp::sequence::{Acid, Symbol};
use itertools::iproduct;
use log::info;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelIterator;

use crate::csv_stat::CsvStatOutput;
use crate::opts::InputReader;
use crate::PROGRESS_BAR;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
pub enum GenerateModelMode {
    Acids,
    QScores,
}

impl GenerateModelMode {
    pub const VALUES: [GenerateModelMode; 2] =
        [GenerateModelMode::Acids, GenerateModelMode::QScores];
}

impl Display for GenerateModelMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GenerateModelMode::Acids => write!(f, "acids"),
            GenerateModelMode::QScores => write!(f, "q_scores"),
        }
    }
}

pub(crate) struct CliModelGenerator {
    input: InputReader,
    stat_output: CsvStatOutput,
    ctx_limit: u32,
}

impl CliModelGenerator {
    #[must_use]
    pub fn new(input: InputReader, output_csv: bool, ctx_limit: u32) -> Self {
        Self {
            input,
            stat_output: CsvStatOutput::new(output_csv),
            ctx_limit,
        }
    }

    pub fn generate_model_all(&self, directory: &Path, name: &str) -> anyhow::Result<()> {
        let variant_num = GenerateModelMode::VALUES.len() * ContextSpecType::VALUES.len();
        PROGRESS_BAR.set_total_bytes(self.input.length()?.unwrap() as u64 * variant_num as u64);

        let variants: Vec<_> =
            iproduct!(GenerateModelMode::VALUES, ContextSpecType::VALUES).collect();
        variants.into_par_iter().try_for_each(|(mode, spec_type)| {
            let name = format!("{}__{}__{}.msgpack", name, mode, spec_type);
            let output_path = directory.join(name);

            let input_file = self.input.reopen_file()?;
            let output_file = File::create(output_path)?;

            self.generate_model_internal(input_file, output_file, mode, spec_type)?;

            anyhow::Ok(())
        })?;

        self.stat_output.flush()?;

        Ok(())
    }

    pub fn generate_model<W: Write>(
        mut self,
        writer: W,
        mode: GenerateModelMode,
        context_type: ContextSpecType,
    ) -> anyhow::Result<()> {
        PROGRESS_BAR.set_total_bytes(self.input.length()?.unwrap_or(0) as u64);

        let reader = mem::take(&mut self.input);
        self.generate_model_internal(reader, writer, mode, context_type)
    }

    fn generate_model_internal<W: Write>(
        &self,
        input: InputReader,
        writer: W,
        mode: GenerateModelMode,
        context_spec_type: ContextSpecType,
    ) -> anyhow::Result<()> {
        match mode {
            GenerateModelMode::Acids => self.save_contexts(
                self.generate_acid_contexts(input, context_spec_type)?,
                ModelType::Acids,
                context_spec_type,
                writer,
            )?,
            GenerateModelMode::QScores => self.save_contexts(
                self.generate_q_score_contexts(input, context_spec_type)?,
                ModelType::QualityScores,
                context_spec_type,
                writer,
            )?,
        }

        Ok(())
    }

    fn save_contexts<T: Symbol, W: Write>(
        &self,
        ctx_gen: Option<ModelGenerator<T>>,
        model_type: ModelType,
        context_spec_type: ContextSpecType,
        writer: W,
    ) -> anyhow::Result<()> {
        if let Some(ctx_gen) = ctx_gen {
            let contexts = ctx_gen.complex_contexts();
            let model = Model::with_model_and_spec_type(model_type, context_spec_type, contexts);
            SerializableModel::write_model(&model, BufWriter::new(writer))?;

            info!(
                "Generated model: model type={}, spec type={}, rate={}, context num={}",
                model_type,
                context_spec_type,
                model.rate(),
                model.len(),
            );
            self.stat_output.add_gen_model_stat(
                model_type,
                context_spec_type,
                model.rate(),
                model.len(),
            )?;
        } else {
            let max_rate = CompressionRate::new(1_000_000.0);

            info!(
                "Model too big: model type={}, spec type={}",
                model_type, context_spec_type,
            );
            self.stat_output.add_gen_model_stat(
                model_type,
                context_spec_type,
                max_rate,
                self.ctx_limit as usize,
            )?;
        }

        Ok(())
    }

    fn generate_acid_contexts(
        &self,
        input: InputReader,
        spec_type: ContextSpecType,
    ) -> anyhow::Result<Option<ModelGenerator<Acid>>> {
        self.generate_contexts(input, spec_type, |acid, _| acid)
    }

    fn generate_q_score_contexts(
        &self,
        input: InputReader,
        spec_type: ContextSpecType,
    ) -> anyhow::Result<Option<ModelGenerator<FastqQualityScore>>> {
        self.generate_contexts(input, spec_type, |_, q_score| q_score)
    }

    fn generate_contexts<T: Symbol, F: Fn(Acid, FastqQualityScore) -> T>(
        &self,
        input: InputReader,
        spec_type: ContextSpecType,
        get_ctx_gen_value: F,
    ) -> anyhow::Result<Option<ModelGenerator<T>>> {
        let mut ctx_gen = ModelGenerator::new();
        let input_length = input.length()?.unwrap_or(0);
        let fastq_reader = FastqReader::new(BufReader::new(input.into_read()));

        let mut processed = ByteNum::ZERO;
        for seq_result in fastq_reader {
            let sequence = seq_result?;
            let seq_size = sequence.size();

            let mut generator = spec_type.generator(sequence.len());

            let acids = sequence.acids().iter();
            let quality_scores = sequence.quality_scores().iter();
            for (acid, q_score) in acids.zip(quality_scores) {
                let ctx_spec = generator.current_context();
                ctx_gen.add(ctx_spec, get_ctx_gen_value(*acid, *q_score));
                generator.update(*acid, *q_score);

                if ctx_gen.len() >= self.ctx_limit as usize {
                    let remaining = input_length.saturating_sub(processed.get() as u64);
                    PROGRESS_BAR.processed_bytes(ByteNum::new(remaining as usize));

                    return Ok(None);
                }
            }

            PROGRESS_BAR.processed_bytes(seq_size);
            processed += seq_size;
        }

        Ok(Some(ctx_gen))
    }
}

impl CsvStatOutput {
    fn add_gen_model_stat(
        &self,
        model_type: ModelType,
        spec_type: ContextSpecType,
        rate: CompressionRate,
        context_num: usize,
    ) -> anyhow::Result<()> {
        self.use_header(&["model type", "spec type", "rate", "context num"])?;
        self.add_record(&[
            model_type.to_string(),
            spec_type.to_string(),
            format!("{}", rate.get()),
            context_num.to_string(),
        ])?;

        anyhow::Ok(())
    }
}
