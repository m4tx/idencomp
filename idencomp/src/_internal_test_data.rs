use itertools::Itertools;
use lazy_static::lazy_static;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use crate::context::Context;
use crate::context_binning::ComplexContext;
use crate::context_spec::{ContextSpec, ContextSpecType, GenericContextSpec};
use crate::fastq::reader::FastqReader;
use crate::fastq::{FastqQualityScore, FastqSequence, FASTQ_Q_END};
use crate::idn::model_provider::ModelProvider;
use crate::model::{Model, ModelType};
use crate::sequence::Acid::{A, C, G, T};

pub const EMPTY_TEST_SEQUENCE_STR: &str = "@seq

+

";

lazy_static! {
    pub static ref EMPTY_TEST_SEQUENCE: FastqSequence = FastqSequence::new("seq", [], []);
}

pub const SIMPLE_TEST_SEQUENCE_STR: &str = "@SEQ_ID
GATTTGGGGTTCAAAGCAGTATCGATCAAATAGTAAATCCATTTGTTCAACTCACAGTTT
+
!''*((((***+))%%%++)(%%%%).1***-+*''))**55CCF>>>>>>CCCCCCC65
";

pub const SIMPLE_TEST_SEQUENCE_SEPARATOR_TITLE_STR: &str = "@SEQ_ID
GATTTGGGGTTCAAAGCAGTATCGATCAAATAGTAAATCCATTTGTTCAACTCACAGTTT
+SEQ_ID
!''*((((***+))%%%++)(%%%%).1***-+*''))**55CCF>>>>>>CCCCCCC65
";

lazy_static! {
    pub static ref SHORT_TEST_SEQUENCE: FastqSequence = FastqSequence::new(
        "",
        [A, C, T, G],
        [0, 1, 13, 50]
            .into_iter()
            .map_into()
            .collect::<Vec<FastqQualityScore>>(),
    );
    pub static ref TEST_SEQUENCE_PREFER_A: FastqSequence = FastqSequence::new(
        "PREFER_A",
        [A; 100],
        [0; 100]
            .into_iter()
            .map_into()
            .collect::<Vec<FastqQualityScore>>(),
    );
    pub static ref TEST_SEQUENCE_PREFER_C: FastqSequence = FastqSequence::new(
        "PREFER_C",
        [C; 100],
        [50; 100]
            .into_iter()
            .map_into()
            .collect::<Vec<FastqQualityScore>>(),
    );
    pub static ref SIMPLE_TEST_SEQUENCE: FastqSequence = FastqSequence::new(
        "SEQ_ID",
        [
            G, A, T, T, T, G, G, G, G, T, T, C, A, A, A, G, C, A, G, T, A, T, C, G, A, T, C, A, A,
            A, T, A, G, T, A, A, A, T, C, C, A, T, T, T, G, T, T, C, A, A, C, T, C, A, C, A, G, T,
            T, T
        ],
        [
            0, 6, 6, 9, 7, 7, 7, 7, 9, 9, 9, 10, 8, 8, 4, 4, 4, 10, 10, 8, 7, 4, 4, 4, 4, 8, 13,
            16, 9, 9, 9, 12, 10, 9, 6, 6, 8, 8, 9, 9, 20, 20, 34, 34, 37, 29, 29, 29, 29, 29, 29,
            34, 34, 34, 34, 34, 34, 34, 21, 20
        ]
        .into_iter()
        .map_into()
        .collect::<Vec<FastqQualityScore>>(),
    );
}

lazy_static! {
    pub static ref CONTEXTS_10: [Context; 10] = [
        Context::new_from(
            0.10,
            [
                0.010_070_696,
                0.197_133_56,
                0.037_697_78,
                0.158_691_27,
                0.154_352_43,
                0.074_826_05,
                0.006_116_782,
                0.203_739_76,
                0.103_777_07,
                0.053_594_608,
            ],
        ),
        Context::new_from(
            0.10,
            [
                0.010_070_696,
                0.197_133_56,
                0.037_697_78,
                0.158_691_27,
                0.154_352_43,
                0.074_826_05,
                0.006_116_782,
                0.203_739_76,
                0.103_777_07,
                0.053_594_608
            ],
        ),
        Context::new_from(
            0.10,
            [
                0.115_177_24,
                0.132_888_73,
                0.135_738_18,
                0.137_837_84,
                0.047_280_237,
                0.050_240_755,
                0.074_097_27,
                0.137_256_38,
                0.092_149_35,
                0.077_334_02
            ]
        ),
        Context::new_from(
            0.10,
            [
                0.122_820_504,
                0.062_127_96,
                0.140_341_04,
                0.129_409_91,
                0.113_334_686,
                0.040_493_485,
                0.113_194_115,
                0.034_223_627,
                0.139_141_57,
                0.104_913_086
            ]
        ),
        Context::new_from(
            0.10,
            [
                0.132_579_61,
                0.123_856_78,
                0.005_169_783_3,
                0.142_291_75,
                0.034_180_03,
                0.105_346_91,
                0.079_145_71,
                0.122_371_85,
                0.145_571_22,
                0.109_486_36
            ]
        ),
        Context::new_from(
            0.10,
            [
                0.174_560_46,
                0.053_836_59,
                0.154_215_07,
                0.153_958_59,
                0.002_684_080_5,
                0.023_054_933,
                0.118_025_11,
                0.147_706_87,
                0.116_522_53,
                0.055_435_76
            ]
        ),
        Context::new_from(
            0.10,
            [
                0.049_407_884,
                0.013_928_039,
                0.175_146_49,
                0.018_354_481,
                0.090_210_56,
                0.133_388_9,
                0.035_597_675,
                0.196_690_3,
                0.207_679_81,
                0.079_595_834
            ]
        ),
        Context::new_from(
            0.10,
            [
                0.053_198_535,
                0.145_106_82,
                0.024_291_487,
                0.158_956_94,
                0.118_159_086,
                0.138_562_52,
                0.088_637_136,
                0.044_603_217,
                0.072_741_08,
                0.155_743_17
            ]
        ),
        Context::new_from(
            0.10,
            [
                0.138_507_63,
                0.104_356_036,
                0.128_981_1,
                0.162_930_52,
                0.031_601_198,
                0.020_049_524,
                0.075_027_06,
                0.124_530_7,
                0.046_608_59,
                0.167_407_65
            ]
        ),
        Context::new_from(
            0.10,
            [
                0.064_611_554,
                0.174_937_17,
                0.058_010_552,
                0.167_391_42,
                0.126_533_96,
                0.066_039_726,
                0.051_141_575,
                0.036_756_743,
                0.106_894_73,
                0.147_682_56
            ]
        ),
    ];
    pub static ref TEST_ACID_MODEL_PREFER_A: Model = create_acid_model_prefer_a();
    pub static ref TEST_ACID_MODEL_PREFER_C: Model = create_acid_model_prefer_c();
    pub static ref SIMPLE_ACID_MODEL: Model = create_simple_acid_model();
    pub static ref SIMPLE_Q_SCORE_MODEL: Model = create_simple_qscore_model();
    pub static ref SIMPLE_MODEL_PROVIDER: ModelProvider = ModelProvider::new(vec![
        SIMPLE_ACID_MODEL.clone(),
        SIMPLE_Q_SCORE_MODEL.clone(),
    ]);
    pub static ref RANDOM_200_CTX_Q_SCORE_MODEL: Model = create_random_q_score_model(200);
    pub static ref RANDOM_500_CTX_Q_SCORE_MODEL: Model = create_random_q_score_model(500);
}

fn create_simple_acid_model() -> Model {
    let ctx1 = Context::new_from(0.25, [0.00, 0.80, 0.10, 0.05, 0.05]);
    let ctx2 = Context::new_from(0.25, [0.00, 0.25, 0.50, 0.15, 0.10]);
    let ctx3 = Context::new_from(0.25, [0.00, 0.01, 0.01, 0.97, 0.01]);
    let ctx4 = Context::new_from(0.25, [0.00, 0.30, 0.30, 0.30, 0.10]);
    let contexts = [
        ComplexContext::with_single_spec(GenericContextSpec::without_pos([A], []).into(), ctx1),
        ComplexContext::with_single_spec(GenericContextSpec::without_pos([C], []).into(), ctx2),
        ComplexContext::with_single_spec(GenericContextSpec::without_pos([T], []).into(), ctx3),
        ComplexContext::with_single_spec(GenericContextSpec::without_pos([G], []).into(), ctx4),
    ];

    Model::with_model_and_spec_type(
        ModelType::Acids,
        ContextSpecType::Generic1Acids0QScores0PosBits,
        contexts,
    )
}

fn create_acid_model_prefer_a() -> Model {
    let ctx1 = Context::new_from(1.0, [0.001, 0.900, 0.033, 0.033, 0.033]);
    let contexts = [ComplexContext::with_single_spec(
        GenericContextSpec::without_pos([], []).into(),
        ctx1,
    )];

    Model::with_model_and_spec_type(ModelType::Acids, ContextSpecType::Dummy, contexts)
}

fn create_acid_model_prefer_c() -> Model {
    let ctx1 = Context::new_from(1.0, [0.001, 0.033, 0.900, 0.033, 0.033]);
    let contexts = [ComplexContext::with_single_spec(
        GenericContextSpec::without_pos([], []).into(),
        ctx1,
    )];

    Model::with_model_and_spec_type(ModelType::Acids, ContextSpecType::Dummy, contexts)
}

fn create_simple_qscore_model() -> Model {
    let mut contexts = Vec::new();

    for i in 0..FASTQ_Q_END {
        let mut symbols = Vec::new();
        for j in 0..FASTQ_Q_END {
            let symbol_prob = if i == j { 0.06 } else { 0.01 };
            symbols.push(symbol_prob);
        }
        let spec = GenericContextSpec::without_pos([], [FastqQualityScore::new(i as u8)]);
        let ctx = Context::new_from(1.0 / FASTQ_Q_END as f32, symbols);

        contexts.push(ComplexContext::with_single_spec(spec.into(), ctx));
    }

    Model::with_model_and_spec_type(
        ModelType::QualityScores,
        ContextSpecType::Generic0Acids1QScores0PosBits,
        contexts,
    )
}

fn create_random_q_score_model(num: usize) -> Model {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(1337);
    let mut contexts = Vec::new();

    let probs = make_random_probs(&mut rng, num);

    for (i, prob) in probs.into_iter().enumerate() {
        let spec = ContextSpec::new(i as u32);
        let ctx_probs = make_random_probs(&mut rng, 94);

        let ctx = Context::new_from(prob, ctx_probs);
        let complex_ctx = ComplexContext::with_single_spec(spec, ctx);
        contexts.push(complex_ctx);
    }

    Model::with_model_and_spec_type(
        ModelType::QualityScores,
        ContextSpecType::Generic4Acids0QScores0PosBits,
        contexts,
    )
}

fn make_random_probs<T: Rng>(rng: &mut T, num: usize) -> Vec<f32> {
    let mut probs = vec![0.0f32; num];
    for prob in &mut probs {
        *prob = rng.gen();
    }
    let sum: f32 = probs.iter().sum();
    for prob in &mut probs {
        *prob /= sum;
    }

    probs
}

pub const SEQ_1M_FASTQ: &[u8] = include_bytes!("../samples/1M.fastq");
pub const SEQ_1M_IDN: &[u8] = include_bytes!("../samples/1M.idn");
pub const SEQ_1K_READS_FASTQ: &[u8] = include_bytes!("../samples/1k-reads.fastq");

lazy_static! {
    pub static ref SEQ_1M: FastqSequence = bytes_to_fastq_sequence(SEQ_1M_FASTQ);
    pub static ref SEQ_1K_READS: Vec<FastqSequence> =
        bytes_to_all_fastq_sequences(SEQ_1K_READS_FASTQ);
}

fn bytes_to_fastq_sequence(data: &[u8]) -> FastqSequence {
    let mut parser = FastqReader::new(data);
    parser.read_sequence().unwrap()
}

fn bytes_to_all_fastq_sequences(data: &[u8]) -> Vec<FastqSequence> {
    let parser = FastqReader::new(data);
    let result: Result<Vec<_>, _> = parser.into_iter().collect();
    result.unwrap()
}
