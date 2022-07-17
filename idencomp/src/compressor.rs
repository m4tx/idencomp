use rans::byte_decoder::{ByteRansDecSymbol, ByteRansDecoderMulti};
use rans::byte_encoder::{ByteRansEncSymbol, ByteRansEncoderMulti};
#[cfg(test)]
use rans::RansDecoder;
use rans::{RansDecSymbol, RansDecoderMulti, RansEncSymbol, RansEncoder, RansEncoderMulti};

use crate::context::Context;

type Encoder<const N: usize> = ByteRansEncoderMulti<N>;
type EncoderSymbol = ByteRansEncSymbol;
type Decoder<'a, const N: usize> = ByteRansDecoderMulti<'a, N>;
type DecoderSymbol = ByteRansDecSymbol;

#[derive(Debug, Clone)]
pub struct RansEncContext<const SYMBOLS_NUM: usize> {
    symbols: [EncoderSymbol; SYMBOLS_NUM],
}

impl<const SYMBOLS_NUM: usize> RansEncContext<SYMBOLS_NUM> {
    #[must_use]
    pub fn from_context(context: &Context, scale_bits: u8) -> Self {
        let cum_freqs = context.as_integer_cum_freqs(scale_bits);
        let mut freqs = cum_freqs.clone();
        Context::cum_freq_to_freq(&mut freqs, 1 << scale_bits);

        let symbols = cum_freqs
            .iter()
            .zip(freqs.iter())
            .map(|(&cum_freq, &freq)| EncoderSymbol::new(cum_freq, freq, scale_bits as u32))
            .collect::<Vec<EncoderSymbol>>()
            .try_into()
            .unwrap();

        Self { symbols }
    }
}

#[derive(Debug)]
pub struct RansCompressor<const N: usize> {
    encoder: Encoder<N>,
}

const MAX_BLOCK_SIZE: usize = 32 * 1024 * 1024; // 32MiB

impl<const N: usize> RansCompressor<N> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            encoder: Encoder::new(MAX_BLOCK_SIZE),
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.encoder.reset();
    }

    #[inline]
    pub fn flush(&mut self) {
        self.encoder.flush_all();
    }

    #[inline]
    #[must_use]
    pub fn data(&self) -> &[u8] {
        self.encoder.data()
    }
}

impl RansCompressor<1> {
    #[inline]
    pub fn put<const SYMBOLS_NUM: usize>(
        &mut self,
        context: &RansEncContext<SYMBOLS_NUM>,
        symbol_index: usize,
    ) {
        assert!(symbol_index < SYMBOLS_NUM);

        self.encoder.put(&context.symbols[symbol_index]);
    }
}

impl RansCompressor<2> {
    #[inline]
    pub fn put<const SYMBOLS_NUM_1: usize, const SYMBOLS_NUM_2: usize>(
        &mut self,
        context_1: &RansEncContext<SYMBOLS_NUM_1>,
        symbol_index_1: usize,
        context_2: &RansEncContext<SYMBOLS_NUM_2>,
        symbol_index_2: usize,
    ) {
        debug_assert!(symbol_index_1 < SYMBOLS_NUM_1);
        debug_assert!(symbol_index_2 < SYMBOLS_NUM_2);

        self.encoder.put_at(0, &context_1.symbols[symbol_index_1]);
        self.encoder.put_at(1, &context_2.symbols[symbol_index_2]);
    }
}

#[derive(Debug, Clone)]
pub struct RansDecContext<const SYMBOLS_NUM: usize> {
    symbols: [DecoderSymbol; SYMBOLS_NUM],
    freq_to_symbol: Vec<usize>,
    scale_bits: u32,
}

impl<const SYMBOLS_NUM: usize> RansDecContext<SYMBOLS_NUM> {
    #[must_use]
    pub fn from_context(context: &Context, scale_bits: u8) -> Self {
        let total_freq = 1 << scale_bits;

        let cum_freqs = context.as_integer_cum_freqs(scale_bits);
        let mut freqs = cum_freqs.clone();
        Context::cum_freq_to_freq(&mut freqs, total_freq);

        let symbols = cum_freqs
            .iter()
            .zip(freqs.iter())
            .map(|(&cum_freq, &freq)| DecoderSymbol::new(cum_freq, freq))
            .collect::<Vec<DecoderSymbol>>()
            .try_into()
            .unwrap();

        let mut freq_to_symbol = Vec::with_capacity(total_freq as usize);
        for i in 0..cum_freqs.len() - 1 {
            freq_to_symbol.resize(cum_freqs[i + 1] as usize, i);
        }
        freq_to_symbol.resize(total_freq as usize, cum_freqs.len() - 1);

        Self {
            symbols,
            freq_to_symbol,
            scale_bits: scale_bits as u32,
        }
    }

    #[must_use]
    pub fn cum_freq_to_symbol_index(&self, cum_freq: u32) -> usize {
        self.freq_to_symbol[cum_freq as usize]
    }
}

pub struct RansDecompressor<'a, const N: usize> {
    decoder: Decoder<'a, N>,
}

impl<'a, const N: usize> RansDecompressor<'a, N> {
    #[must_use]
    pub fn new(data: &'a mut [u8]) -> Self {
        Self {
            decoder: Decoder::new(data),
        }
    }
}

#[cfg(test)]
impl<'a> RansDecompressor<'a, 1> {
    #[inline]
    #[must_use]
    pub fn get<const SYMBOLS_NUM: usize>(
        &mut self,
        context: &RansDecContext<SYMBOLS_NUM>,
    ) -> usize {
        let cum_freq = self.decoder.get(context.scale_bits);
        let symbol_index = context.cum_freq_to_symbol_index(cum_freq);
        self.decoder
            .advance(&context.symbols[symbol_index], context.scale_bits);

        symbol_index
    }
}

impl<'a> RansDecompressor<'a, 2> {
    #[inline]
    #[must_use]
    pub fn get<const SYMBOLS_NUM_1: usize, const SYMBOLS_NUM_2: usize>(
        &mut self,
        context_1: &RansDecContext<SYMBOLS_NUM_1>,
        context_2: &RansDecContext<SYMBOLS_NUM_2>,
    ) -> (usize, usize) {
        let cum_freq_2 = self.decoder.get_at(0, context_2.scale_bits);
        let cum_freq_1 = self.decoder.get_at(1, context_1.scale_bits);
        let symbol_index_2 = context_2.cum_freq_to_symbol_index(cum_freq_2);
        let symbol_index_1 = context_1.cum_freq_to_symbol_index(cum_freq_1);
        self.decoder
            .advance_step_at(0, &context_2.symbols[symbol_index_2], context_2.scale_bits);
        self.decoder
            .advance_step_at(1, &context_1.symbols[symbol_index_1], context_1.scale_bits);
        self.decoder.renorm_all();

        (symbol_index_1, symbol_index_2)
    }
}

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng};
    use rand_xoshiro::Xoshiro256PlusPlus;

    use crate::_internal_test_data::CONTEXTS_10;
    use crate::compressor::{RansCompressor, RansDecContext, RansDecompressor, RansEncContext};
    use crate::context::Context;

    #[test]
    fn enc_context_from_context() {
        let context = Context::new_from(
            1.0,
            [0.05, 0.10, 0.125, 0.125, 0.30, 0.03, 0.07, 0.05, 0.12, 0.03],
        );

        let _ctx = RansEncContext::<10>::from_context(&context, 10);
    }

    #[test]
    fn dec_context_from_context() {
        let context = Context::new_from(
            1.0,
            [0.05, 0.10, 0.125, 0.125, 0.30, 0.03, 0.07, 0.05, 0.12, 0.03],
        );

        let _ctx = RansDecContext::<10>::from_context(&context, 10);
    }

    #[test]
    fn test_small_output() {
        const SCALE_BITS: u8 = 16;

        let ctx1 = Context::new_from(1.0, [0.001, 0.001, 0.997, 0.001]);
        let enc_ctx1 = RansEncContext::<4>::from_context(&ctx1, SCALE_BITS);

        let mut compressor = RansCompressor::<1>::new();
        for _ in 0..500 {
            compressor.put(&enc_ctx1, 2);
        }
        compressor.flush();

        let compressed = compressor.data();

        assert_eq!(compressed.len(), 4);
    }

    #[test]
    fn round_trip_single_ctx() {
        let ctx = Context::new_from(1.0, [0.25, 0.25, 0.25, 0.25]);

        let data = vec![(&ctx, 0), (&ctx, 1), (&ctx, 2), (&ctx, 3)];

        test_round_trip::<4>(data);
    }

    #[test]
    fn round_trip_multi_ctx() {
        let ctx1 = Context::new_from(0.25, [0.25, 0.25, 0.25, 0.25]);
        let ctx2 = Context::new_from(0.25, [0.90, 0.03, 0.03, 0.04]);
        let ctx3 = Context::new_from(0.25, [0.50, 0.125, 0.125, 0.25]);
        let ctx4 = Context::new_from(0.25, [0.0, 0.33, 0.33, 0.34]);

        let data = vec![(&ctx1, 0), (&ctx2, 1), (&ctx3, 2), (&ctx4, 3)];

        test_round_trip::<4>(data);
    }

    #[test]
    fn round_trip_more_data() {
        let mut data = Vec::new();
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(1337);
        for _ in 0..4 * 1024 {
            data.push((&CONTEXTS_10[rng.gen_range(0..10)], rng.gen_range(0..10)));
        }

        test_round_trip::<10>(data);
    }

    fn test_round_trip<const SYMBOLS_NUM: usize>(mut data: Vec<(&Context, usize)>) {
        const SCALE_BITS: u8 = 6;

        let mut compressor = RansCompressor::<1>::new();
        for (ctx, val) in &data {
            let enc_ctx = RansEncContext::<SYMBOLS_NUM>::from_context(ctx, SCALE_BITS);
            compressor.put(&enc_ctx, *val);
        }
        compressor.flush();
        let mut compressed = compressor.data().to_owned();
        data.reverse();

        let mut decompressor = RansDecompressor::<1>::new(&mut compressed);
        for (ctx, val) in &data {
            let dec_ctx = RansDecContext::<SYMBOLS_NUM>::from_context(ctx, SCALE_BITS);
            assert_eq!(decompressor.get(&dec_ctx), *val);
        }
    }

    #[test]
    fn round_trip_two_channels() {
        const SCALE_BITS: u8 = 6;

        let ctx1 = Context::new_from(1.0, [0.25, 0.25, 0.25, 0.25]);
        let ctx2 = Context::new_from(
            1.0,
            [0.125, 0.125, 0.125, 0.125, 0.125, 0.125, 0.125, 0.125],
        );
        let enc_ctx1 = RansEncContext::<4>::from_context(&ctx1, SCALE_BITS);
        let enc_ctx2 = RansEncContext::<8>::from_context(&ctx2, SCALE_BITS);
        let dec_ctx1 = RansDecContext::<4>::from_context(&ctx1, SCALE_BITS);
        let dec_ctx2 = RansDecContext::<8>::from_context(&ctx2, SCALE_BITS);

        let mut compressor = RansCompressor::<2>::new();
        compressor.put(&enc_ctx1, 0, &enc_ctx2, 1);
        compressor.put(&enc_ctx1, 1, &enc_ctx2, 3);
        compressor.put(&enc_ctx1, 2, &enc_ctx2, 5);
        compressor.put(&enc_ctx1, 3, &enc_ctx2, 7);
        compressor.flush();

        let mut compressed = compressor.data().to_owned();

        let mut decompressor = RansDecompressor::<2>::new(&mut compressed);
        assert_eq!(decompressor.get(&dec_ctx1, &dec_ctx2), (3, 7));
        assert_eq!(decompressor.get(&dec_ctx1, &dec_ctx2), (2, 5));
        assert_eq!(decompressor.get(&dec_ctx1, &dec_ctx2), (1, 3));
        assert_eq!(decompressor.get(&dec_ctx1, &dec_ctx2), (0, 1));
    }
}
