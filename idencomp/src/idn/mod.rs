mod common;
/// IDN file compressor.
pub mod compressor;
mod compressor_block;
mod compressor_initializer;
mod data;
/// IDN file decompressor.
pub mod decompressor;
mod decompressor_block;
mod model_chooser;
/// The collection of models that can be used when compressing or decompressing
/// an IDN file.
pub mod model_provider;
pub mod no_seek;
#[cfg(test)]
mod tests;
mod thread_pool;
mod writer_block;
mod writer_idn;
