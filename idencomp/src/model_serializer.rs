use std::io::{Read, Write};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::context::{Context, Probability};
use crate::context_binning::ComplexContext;
use crate::context_spec::{ContextSpec, ContextSpecType};
use crate::model::{Model, ModelIdentifier, ModelType};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
struct SerializableContext {
    pub context_prob: Probability,
    pub symbol_prob: Vec<Probability>,
}

impl SerializableContext {
    #[must_use]
    pub fn new(context_prob: Probability, symbol_prob: Vec<Probability>) -> Self {
        Self {
            context_prob,
            symbol_prob,
        }
    }
}

impl From<Context> for SerializableContext {
    fn from(ctx: Context) -> Self {
        Self::new(ctx.context_prob, ctx.symbol_prob)
    }
}

impl From<SerializableContext> for Context {
    fn from(serializable_ctx: SerializableContext) -> Self {
        Self::new(serializable_ctx.context_prob, serializable_ctx.symbol_prob)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
struct SerializableComplexContext {
    specs: Vec<ContextSpec>,
    context: SerializableContext,
}

impl SerializableComplexContext {
    #[must_use]
    fn new(specs: Vec<ContextSpec>, context: SerializableContext) -> Self {
        Self { specs, context }
    }
}

impl From<ComplexContext> for SerializableComplexContext {
    fn from(ctx: ComplexContext) -> Self {
        Self::new(ctx.specs, ctx.context.into())
    }
}

impl From<SerializableComplexContext> for ComplexContext {
    fn from(serializable_ctx: SerializableComplexContext) -> Self {
        Self::new(serializable_ctx.specs, serializable_ctx.context.into())
    }
}

/// An intermediate structure that can be converted to and from [`Model`], and additionally can be serialized and deserialized.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableModel {
    identifier: ModelIdentifier,
    model_type: ModelType,
    context_spec_type: ContextSpecType,
    contexts: Vec<SerializableComplexContext>,
}

impl SerializableModel {
    pub fn read_model<R: Read>(reader: R) -> anyhow::Result<Model> {
        let result = Self::read(reader)?;
        Ok(result.into())
    }

    pub fn read<R: Read>(reader: R) -> anyhow::Result<SerializableModel> {
        let result = rmp_serde::from_read(reader)?;
        Ok(result)
    }

    pub fn write_model<W: Write>(model: &Model, mut writer: W) -> anyhow::Result<()> {
        SerializableModel::from(model).write(&mut writer)?;
        writer.flush()?;
        Ok(())
    }

    pub fn write<W: Write>(&self, mut writer: W) -> anyhow::Result<()> {
        self.serialize(&mut rmp_serde::Serializer::new(&mut writer))?;
        writer.flush()?;
        Ok(())
    }
}

impl From<&Model> for SerializableModel {
    fn from(model: &Model) -> Self {
        Self {
            identifier: model.identifier().clone(),
            model_type: model.model_type(),
            context_spec_type: model.context_spec_type(),
            contexts: model
                .as_complex_contexts()
                .iter()
                .sorted()
                .cloned()
                .map_into()
                .collect(),
        }
    }
}

impl From<SerializableModel> for Model {
    fn from(ser_model: SerializableModel) -> Self {
        let contexts: Vec<ComplexContext> = ser_model.contexts.into_iter().map_into().collect();
        let model = Model::with_model_and_spec_type(
            ser_model.model_type,
            ser_model.context_spec_type,
            contexts,
        );

        assert_eq!(model.identifier(), &ser_model.identifier);
        model
    }
}

#[cfg(test)]
mod tests {
    use crate::_internal_test_data::SIMPLE_ACID_MODEL;
    use crate::context::Context;
    use crate::context_binning::ComplexContext;
    use crate::context_spec::{ContextSpec, ContextSpecType, GenericContextSpec};
    use crate::model::{Model, ModelType};
    use crate::model_serializer::SerializableModel;
    use crate::sequence::Acid;

    #[test]
    fn test_model_to_serializable() {
        let ctx1 = Context::new_from(0.25, [0.80, 0.10, 0.05, 0.05, 0.00]);
        let spec1: ContextSpec = GenericContextSpec::without_pos([Acid::A], []).into();
        let spec2: ContextSpec = GenericContextSpec::without_pos([Acid::T], []).into();
        let ctx2 = Context::new_from(0.25, [0.25, 0.50, 0.15, 0.10, 0.00]);
        let spec3: ContextSpec = GenericContextSpec::without_pos([Acid::C], []).into();
        let contexts = [
            ComplexContext::new([spec1, spec2], ctx1),
            ComplexContext::with_single_spec(spec3, ctx2),
        ];

        let model = Model::with_model_and_spec_type(
            ModelType::Acids,
            ContextSpecType::Generic1Acids0QScores0PosBits,
            contexts.clone(),
        );

        let serializable_model = SerializableModel::from(&model);
        assert_eq!(serializable_model.model_type, ModelType::Acids);
        assert_eq!(
            serializable_model.context_spec_type,
            ContextSpecType::Generic1Acids0QScores0PosBits
        );
        assert_eq!(serializable_model.contexts, contexts.map(|x| x.into()));

        let model_2 = Model::from(serializable_model);
        assert_eq!(model, model_2);
    }

    #[test]
    fn test_write_and_read_model() {
        let mut data = Vec::new();
        let model = SIMPLE_ACID_MODEL.clone();

        SerializableModel::write_model(&model, &mut data).unwrap();
        let model_2 = SerializableModel::read_model(data.as_slice()).unwrap();

        assert_eq!(model, model_2);
    }
}
