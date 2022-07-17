use idencomp::_internal_test_data::{SEQ_1K_READS, SEQ_1M, SEQ_1M_IDN, SIMPLE_MODEL_PROVIDER};
use idencomp::idn::compressor::{IdnCompressor, IdnCompressorParams};
use idencomp::idn::decompressor::{IdnDecompressor, IdnDecompressorParams};

#[test]
fn test_compress_simple_1m() {
    let mut data = Vec::new();
    let params = IdnCompressorParams::builder()
        .model_provider(SIMPLE_MODEL_PROVIDER.clone())
        .build();

    let mut idn_compressor = IdnCompressor::with_params(&mut data, params);
    idn_compressor.add_sequence(SEQ_1M.clone()).unwrap();
    idn_compressor.finish().unwrap();

    assert!(!data.is_empty());
}

#[test]
fn test_decompress_simple_1m() {
    let mut data = SEQ_1M_IDN;
    let params = IdnDecompressorParams::builder()
        .model_provider(SIMPLE_MODEL_PROVIDER.clone())
        .build();

    let mut idn_decompressor = IdnDecompressor::with_params(&mut data, params);
    let seq = idn_decompressor.next_sequence().unwrap();

    assert!(seq.is_some());
    assert_eq!(seq.unwrap(), *SEQ_1M);
    assert!(idn_decompressor.next_sequence().unwrap().is_none());
}

#[test_log::test]
fn test_round_trip_small_blocks() {
    let mut data = Vec::new();

    // Compress
    let params = IdnCompressorParams::builder()
        .model_provider(SIMPLE_MODEL_PROVIDER.clone())
        .max_block_total_len(200)
        .build();

    let mut idn_compressor = IdnCompressor::with_params(&mut data, params);
    for sequence in SEQ_1K_READS.iter() {
        idn_compressor.add_sequence(sequence.clone()).unwrap();
    }
    idn_compressor.finish().unwrap();

    // Decompress
    let params = IdnDecompressorParams::builder()
        .model_provider(SIMPLE_MODEL_PROVIDER.clone())
        .build();

    let idn_decompressor = IdnDecompressor::with_params(data.as_slice(), params);
    let result: Result<Vec<_>, _> = idn_decompressor.into_iter().collect();
    let sequences = result.unwrap();

    assert_eq!(sequences, *SEQ_1K_READS);
}

#[test_log::test]
fn test_round_trip_small_blocks_threaded() {
    let mut data = Vec::new();

    // Compress
    let params = IdnCompressorParams::builder()
        .model_provider(SIMPLE_MODEL_PROVIDER.clone())
        .max_block_total_len(200)
        .thread_num(8)
        .build();

    let mut idn_compressor = IdnCompressor::with_params(&mut data, params);
    for sequence in SEQ_1K_READS.iter() {
        idn_compressor.add_sequence(sequence.clone()).unwrap();
    }
    idn_compressor.finish().unwrap();

    // Decompress
    let params = IdnDecompressorParams::builder()
        .model_provider(SIMPLE_MODEL_PROVIDER.clone())
        .thread_num(8)
        .build();

    let idn_decompressor = IdnDecompressor::with_params(data.as_slice(), params);
    let result: Result<Vec<_>, _> = idn_decompressor.into_iter().collect();
    let sequences = result.unwrap();

    assert_eq!(sequences, *SEQ_1K_READS);
}
