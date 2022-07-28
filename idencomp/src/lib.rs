#![warn(missing_docs)]

//! idencomp (jap. 遺伝コンプレッサー (idenkonpuressa) — "genetic compressor")
//! is an attempt on building a compression tool for genetic data (precisely,
//! for FASTQ files). The goal is beat the performance of most commonly used
//! tools, while maintaining a decent compression ratio.
//!
//! This is based on several building blocks:
//!
//! * [context binning and k-means model clustering](https://arxiv.org/abs/2201.05028)
//! * [rANS entropy coder](https://en.wikipedia.org/wiki/Asymmetric_numeral_systems#Range_variants_(rANS)_and_streaming)
//! * [Deflate](https://en.wikipedia.org/wiki/Deflate) and [Brotli](https://en.wikipedia.org/wiki/Brotli)
//!   (compressing sequence names)
//!
//! The compressor has been built with modern multicore CPUs in mind and can
//! utilize multiple cores/threads for all the critical parts. It contains a CLI
//! interface and an accompanying Rust library.

mod compressor;
/// Statistical model for a single local situation.
pub mod context;
/// Context binning module that can be used to make smaller models while
/// maintaining decent compression rate.
pub mod context_binning;
/// Context specifier generators that can describe local situations in a
/// sequence with a single number.
pub mod context_spec;
/// FASTQ file reader and writer.
pub mod fastq;
/// IDN compressor, decompressor, and utilities around.
pub mod idn;
/// Statistical model used to compress/decompress sequences.
pub mod model;
/// Utilities that can be used to create models using nucleotide sequences.
pub mod model_generator;
/// Nucleotide sequence and its building blocks.
pub mod sequence;
mod sequence_compressor;

#[doc(hidden)]
pub mod _internal_test_data;
mod clustering;
mod int_queue;
/// Serializer and deserializer of the statistical model.
pub mod model_serializer;
/// Progress notifier that can be used to get the progress of the long-running
/// operations.
pub mod progress;
