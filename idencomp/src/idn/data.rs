use binrw::binrw;

#[binrw]
#[brw(big, magic = b"IDENCOMP")]
#[derive(Debug)]
pub struct IdnHeader {
    pub version: u8,
}

#[binrw]
#[brw(big)]
#[derive(Debug)]
pub struct IdnMetadataHeader {
    pub item_num: u8,
}

#[binrw]
#[brw(big)]
#[derive(Debug)]
pub enum IdnMetadataItem {
    #[brw(magic = 0u8)]
    Models(IdnModelsMetadata),
}

#[binrw]
#[brw(big)]
#[derive(Debug)]
pub struct IdnModelsMetadata {
    pub num_models: u8,

    #[br(count = num_models)]
    pub model_identifiers: Vec<[u8; 32]>,
}

#[binrw]
#[brw(big)]
#[derive(Debug)]
pub struct IdnBlockHeader {
    pub length: u32,
    pub seq_checksum: u32,
    pub block_num: u32,
}

#[binrw]
#[brw(big)]
#[derive(Debug)]
pub enum IdnSliceHeader {
    #[brw(magic = 0u8)]
    Identifiers(IdnIdentifiersHeader),
    #[brw(magic = 1u8)]
    SwitchModel(IdnSwitchModelHeader),
    #[brw(magic = 2u8)]
    Sequence(IdnSequenceHeader),
}

#[binrw]
#[brw(big, repr = u8)]
#[derive(Debug)]
pub enum IdnIdentifierCompression {
    Brotli,
    Deflate,
}

#[binrw]
#[brw(big)]
#[derive(Debug)]
pub struct IdnIdentifiersHeader {
    pub length: u32,
    pub compression: IdnIdentifierCompression,
}

#[binrw]
#[brw(big)]
#[derive(Debug)]
pub struct IdnSwitchModelHeader {
    pub model_index: u8,
}

#[binrw]
#[brw(big)]
#[derive(Debug)]
pub struct IdnSequenceHeader {
    pub length: u32,
    pub seq_len: u32,
}
