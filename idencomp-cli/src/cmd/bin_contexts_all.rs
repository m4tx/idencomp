use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::path::Path;

use anyhow::Context;
use idencomp::context_binning::{bin_contexts_with_model, ContextBinningOptions};
use idencomp::model::{CompressionRate, Model};
use idencomp::model_serializer::SerializableModel;
use log::info;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelIterator;

use crate::csv_stat::CsvStatOutput;
use crate::PROGRESS_BAR;

pub fn bin_contexts_all<R: Read>(
    reader: R,
    directory: &Path,
    name: &str,
    max_num: Option<usize>,
    pre_bin: Option<usize>,
    output_csv: bool,
) -> anyhow::Result<()> {
    let stat_output = CsvStatOutput::new(output_csv);

    let model = SerializableModel::read_model(BufReader::new(reader))
        .context("Could not read the model")?;
    info!(
        "Binning model: model type={}, spec type={}, rate={}, context num={}",
        model.model_type(),
        model.context_spec_type(),
        model.rate(),
        model.len(),
    );

    let model_type = model.model_type();
    let spec_type = model.context_spec_type();
    let mut model_size = model.len();
    if let Some(pre_bin) = &pre_bin {
        model_size = model_size.min(*pre_bin);
    }

    info!("Building the context tree");
    let mut options = ContextBinningOptions::builder().progress_notifier(Box::new(&*PROGRESS_BAR));
    if let Some(pre_bin) = &pre_bin {
        options = options.pre_binning_num(*pre_bin);
    }
    let tree = bin_contexts_with_model(&model, &options.build());
    info!("Generating the binned versions");

    let max_num = max_num.unwrap_or(model_size - 1) as usize;
    PROGRESS_BAR.set_length(max_num as u64);

    steps_iter(1, model_size, max_num)
        .into_par_iter()
        .try_for_each(|num_contexts| {
            let tree = tree.clone();
            let model =
                Model::with_model_and_spec_type(model_type, spec_type, tree.traverse(num_contexts));
            info!(
                "Generated binned model: contexts: {}, rate: {}",
                model.len(),
                model.rate()
            );

            let name = format!("{}_{}.msgpack", name, num_contexts);
            let output_path = directory.join(name);
            let file = File::create(&output_path).context("Could not create the output file")?;
            SerializableModel::write_model(&model, BufWriter::new(file))
                .context("Could not write the new model")?;

            stat_output.add_bin_ctx_stat(&output_path, model.len(), model.rate())?;

            PROGRESS_BAR.inc(1);
            anyhow::Ok(())
        })?;

    stat_output.flush()?;

    Ok(())
}

fn steps_iter(start: usize, end: usize, max_items: usize) -> Vec<usize> {
    let max_value = end - start;

    if max_items >= max_value {
        (start..end).collect()
    } else {
        (0..max_items)
            .map(|val| val * max_value / max_items + start)
            .collect()
    }
}

impl CsvStatOutput {
    fn add_bin_ctx_stat(
        &self,
        filename: &Path,
        context_num: usize,
        rate: CompressionRate,
    ) -> anyhow::Result<()> {
        self.use_header(&["filename", "context number", "rate"])?;
        self.add_record(&[
            filename.display().to_string(),
            context_num.to_string(),
            format!("{}", rate.get()),
        ])?;

        anyhow::Ok(())
    }
}
