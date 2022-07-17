use criterion::{criterion_group, criterion_main, Criterion};
use idencomp::_internal_test_data::{SEQ_1K_READS_FASTQ, SEQ_1M};
use idencomp::fastq::reader::FastqReader;
use idencomp::fastq::writer::FastqWriter;

fn read_1k_reads(c: &mut Criterion) {
    c.bench_function("Read 1k reads from FASTQ", |b| {
        b.iter(|| {
            let reader = FastqReader::new(SEQ_1K_READS_FASTQ);
            let result: Result<Vec<_>, _> = reader.into_iter().collect();
            assert_eq!(result.unwrap().len(), 1000);
        })
    });
}

fn write_1mb(c: &mut Criterion) {
    c.bench_function("Write 1MB FASTQ", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            {
                let mut writer = FastqWriter::new(&mut buf);
                writer.write_sequence(&SEQ_1M).unwrap();
            }
            assert_eq!(buf.len(), 1_000_038);
        })
    });
}

criterion_group!(benches, read_1k_reads, write_1mb);
criterion_main!(benches);
