use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use idencomp::_internal_test_data::{SEQ_1M, SEQ_1M_IDN, SIMPLE_MODEL_PROVIDER};
use idencomp::idn::compressor::{IdnCompressor, IdnCompressorParams};
use idencomp::idn::decompressor::{IdnDecompressor, IdnDecompressorParams};

fn compress_1m(c: &mut Criterion) {
    c.bench_function("Compress 1MB FASTQ to IDN", |b| {
        b.iter_batched_ref(
            || {
                IdnCompressorParams::builder()
                    .model_provider(SIMPLE_MODEL_PROVIDER.clone())
                    .build()
            },
            |params| {
                let mut data = Vec::new();
                let mut idn_compressor = IdnCompressor::with_params(&mut data, params.clone());
                idn_compressor.add_sequence(SEQ_1M.clone()).unwrap();
                idn_compressor.finish().unwrap();
            },
            BatchSize::LargeInput,
        )
    });
}

fn decompress_1m(c: &mut Criterion) {
    c.bench_function("Decompress 1MB FASTQ from IDN", |b| {
        b.iter_batched_ref(
            || {
                IdnDecompressorParams::builder()
                    .model_provider(SIMPLE_MODEL_PROVIDER.clone())
                    .build()
            },
            |params| {
                let mut data = SEQ_1M_IDN;
                let idn_decompressor = IdnDecompressor::with_params(&mut data, params.clone());

                let sequences = idn_decompressor.into_iter();
                assert_eq!(sequences.count(), 1);
            },
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, compress_1m, decompress_1m);
criterion_main!(benches);
