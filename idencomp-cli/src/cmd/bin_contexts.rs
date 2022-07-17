use std::io::{BufReader, BufWriter, Read, Write};

use anyhow::Context;
use idencomp::context_binning::{bin_contexts_with_model, ContextBinningOptions};
use idencomp::model::Model;
use idencomp::model_serializer::SerializableModel;
use log::info;

use crate::PROGRESS_BAR;

pub fn bin_contexts<R: Read, W: Write>(
    reader: R,
    writer: W,
    num_contexts: usize,
    pre_bin: Option<usize>,
) -> anyhow::Result<()> {
    let model = SerializableModel::read_model(BufReader::new(reader))
        .context("Could not read the model")?;
    let model_type = model.model_type();
    let spec_type = model.context_spec_type();

    let mut options = ContextBinningOptions::builder().progress_notifier(Box::new(&*PROGRESS_BAR));
    if let Some(pre_bin) = pre_bin {
        options = options.pre_binning_num(pre_bin);
    }
    let tree = bin_contexts_with_model(&model, &options.build());

    let model = Model::with_model_and_spec_type(model_type, spec_type, tree.traverse(num_contexts));
    info!(
        "Generated model: contexts: {}, rate: {}",
        model.len(),
        model.rate()
    );
    SerializableModel::write_model(&model, BufWriter::new(writer))
        .context("Could not write the new model")?;

    Ok(())
}
