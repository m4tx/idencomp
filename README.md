idencomp (遺伝コンプレッサー)
=============================

[![Build Status](https://github.com/m4tx/idencomp/workflows/Rust%20CI/badge.svg)](https://github.com/m4tx/idencomp/actions)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/m4tx/idencomp/blob/master/LICENSE)
[![codecov](https://codecov.io/gh/m4tx/idencomp/branch/master/graph/badge.svg?token=6YWGUVOZH4)](https://codecov.io/gh/m4tx/idencomp)

idencomp (jap. 遺伝コンプレッサー (idenkonpuressa) — "genetic compressor") is an
attempt on building a compression tool for genetic data (precisely, for FASTQ
files). The goal is beat the performance of most commonly used tools, while
maintaining a decent compression ratio.

This is based on several building blocks:

* [context binning and k-means model clustering](https://arxiv.org/abs/2201.05028)
* [rANS entropy coder](https://en.wikipedia.org/wiki/Asymmetric_numeral_systems#Range_variants_(rANS)_and_streaming)
* [Deflate](https://en.wikipedia.org/wiki/Deflate) and [Brotli](https://en.wikipedia.org/wiki/Brotli) (compressing sequence names)

The compressor has been built with modern multicore CPUs in mind and can utilize
multiple cores/threads for all the critical parts. It contains a CLI interface
and an accompanying Rust library.

## License

The project is licensed under the [MIT license](LICENSE).

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the project by you shall be licensed as MIT, without any
additional terms or conditions.
