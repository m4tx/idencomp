use criterion::{criterion_group, criterion_main, Criterion};
use idencomp::_internal_test_data::{RANDOM_200_CTX_Q_SCORE_MODEL, RANDOM_500_CTX_Q_SCORE_MODEL};
use idencomp::context_binning::bin_contexts_with_model;

fn bin_200_ctx(c: &mut Criterion) {
    // Ensure the model has been created
    assert_eq!(RANDOM_200_CTX_Q_SCORE_MODEL.len(), 200);

    c.bench_function("Make 200 context tree", |b| {
        b.iter(|| {
            let tree = bin_contexts_with_model(&RANDOM_200_CTX_Q_SCORE_MODEL, &Default::default());
            assert_eq!(tree.len(), 399);
        })
    });
}

fn bin_500_ctx(c: &mut Criterion) {
    // Ensure the model has been created
    assert_eq!(RANDOM_500_CTX_Q_SCORE_MODEL.len(), 500);

    c.bench_function("Make 500 context tree", |b| {
        b.iter(|| {
            let tree = bin_contexts_with_model(&RANDOM_500_CTX_Q_SCORE_MODEL, &Default::default());
            assert_eq!(tree.len(), 999);
        })
    });
}

criterion_group!(benches, bin_200_ctx, bin_500_ctx);
criterion_main!(benches);
