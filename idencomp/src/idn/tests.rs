use crate::_internal_test_data::{
    SHORT_TEST_SEQUENCE, SIMPLE_TEST_SEQUENCE, TEST_ACID_MODEL_PREFER_A, TEST_ACID_MODEL_PREFER_C,
    TEST_SEQUENCE_PREFER_A, TEST_SEQUENCE_PREFER_C,
};
use crate::fastq::FastqSequence;
use crate::idn::compressor::{
    CompressionQuality, IdnCompressor, IdnCompressorParams, IdnCompressorParamsBuilder,
};
use crate::idn::decompressor::{IdnDecompressor, IdnDecompressorParams};
use crate::idn::model_provider::ModelProvider;
use crate::model::{Model, ModelType};

#[test_log::test]
fn test_round_trip_empty_file() {
    round_trip_sequences(&[]);
}

#[test_log::test]
fn test_round_trip_short_sequence() {
    let sequences = [SHORT_TEST_SEQUENCE.clone()];
    round_trip_sequences(&sequences);
}

#[test]
fn test_round_trip_sequence_with_name() {
    let sequences = [SIMPLE_TEST_SEQUENCE.clone()];
    round_trip_sequences(&sequences);
}

#[test]
fn test_round_trip_sequence_identifiers_disabled() {
    let sequences_in = [SIMPLE_TEST_SEQUENCE.clone()];
    let sequences_out = [SIMPLE_TEST_SEQUENCE.clone().with_identifier_discarded()];
    round_trip_sequences_custom(
        &sequences_in,
        &sequences_out,
        ModelProvider::default(),
        |builder| {
            builder.include_identifiers(false);
        },
    );
}

#[test]
fn test_round_trip_multiple_sequences() {
    let sequences = [SHORT_TEST_SEQUENCE.clone(), SIMPLE_TEST_SEQUENCE.clone()];
    round_trip_sequences(&sequences);
}

#[test_log::test]
fn test_round_trip_multiple_models() {
    let models = vec![
        TEST_ACID_MODEL_PREFER_A.clone(),
        TEST_ACID_MODEL_PREFER_C.clone(),
        Model::empty(ModelType::QualityScores),
    ];
    let model_provider = ModelProvider::new(models);

    let sequences = [
        TEST_SEQUENCE_PREFER_A.clone(),
        TEST_SEQUENCE_PREFER_C.clone(),
    ];
    round_trip_sequences_with_model_provider(&sequences, model_provider);
}

#[test_log::test]
fn test_round_trip_all_quals() {
    let models = vec![
        TEST_ACID_MODEL_PREFER_A.clone(),
        TEST_ACID_MODEL_PREFER_C.clone(),
        Model::empty(ModelType::QualityScores),
    ];
    let model_provider = ModelProvider::new(models);

    let sequences = [
        TEST_SEQUENCE_PREFER_A.clone(),
        TEST_SEQUENCE_PREFER_C.clone(),
    ];

    for quality in 1..=9 {
        round_trip_sequences_custom(&sequences, &sequences, model_provider.clone(), |builder| {
            builder.quality(CompressionQuality::new(quality));
        });
    }
}

fn round_trip_sequences(sequences: &[FastqSequence]) {
    round_trip_sequences_with_model_provider(sequences, ModelProvider::default())
}

fn round_trip_sequences_with_model_provider(
    sequences: &[FastqSequence],
    model_provider: ModelProvider,
) {
    round_trip_sequences_custom(sequences, sequences, model_provider, |_| {});
}

fn round_trip_sequences_custom<F>(
    sequences_in: &[FastqSequence],
    sequences_out: &[FastqSequence],
    model_provider: ModelProvider,
    params_modifier: F,
) where
    F: FnOnce(&mut IdnCompressorParamsBuilder),
{
    let mut data = Vec::new();

    let mut writer_params_builder = IdnCompressorParams::builder();
    writer_params_builder.model_provider(model_provider.clone());
    params_modifier(&mut writer_params_builder);
    let writer_params = writer_params_builder.build();

    let mut idn_writer = IdnCompressor::with_params(&mut data, writer_params);
    for sequence in sequences_in {
        idn_writer.add_sequence(sequence.clone()).unwrap();
    }
    idn_writer.finish().unwrap();

    let reader_params = IdnDecompressorParams::builder()
        .model_provider(model_provider)
        .build();
    let mut idn_reader = IdnDecompressor::with_params(data.as_slice(), reader_params);
    for sequence in sequences_out {
        assert_eq!(idn_reader.next_sequence().unwrap().as_ref(), Some(sequence));
    }
    assert_eq!(idn_reader.next_sequence().unwrap(), None);
}
