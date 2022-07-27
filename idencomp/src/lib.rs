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
pub mod context;
pub mod context_binning;
pub mod context_spec;
pub mod fastq;
pub mod idn;
pub mod model;
pub mod model_generator;
pub mod sequence;
mod sequence_compressor;

#[doc(hidden)]
pub mod _internal_test_data;
mod clustering;
mod int_queue;
pub mod model_serializer;
pub mod progress;
