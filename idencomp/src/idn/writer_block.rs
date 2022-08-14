use std::hash::Hash;
use std::io::{Cursor, Seek, Write};

use binrw::BinWrite;

use crate::fastq::FastqSequence;
use crate::idn::compressor::IdnCompressResult;
use crate::idn::data::{
    IdnBlockHeader, IdnIdentifierCompression, IdnIdentifiersHeader, IdnSequenceHeader,
    IdnSliceHeader, IdnSwitchModelHeader,
};

pub(super) struct BlockWriter {
    data: Cursor<Vec<u8>>,
    hasher: crc32fast::Hasher,
}

impl BlockWriter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Cursor::new(Vec::new()),
            hasher: crc32fast::Hasher::new(),
        }
    }

    pub fn write_to<W: Write + Seek>(self, mut writer: W) -> IdnCompressResult<()> {
        let data = self.data.into_inner();
        let checksum = self.hasher.finalize();

        let header = IdnBlockHeader {
            length: data.len() as u32,
            seq_checksum: checksum,
        };

        header.write_to(&mut writer)?;
        writer.write_all(&data)?;

        Ok(())
    }

    pub fn write_identifiers(
        &mut self,
        compression_method: IdnIdentifierCompression,
        data: &[u8],
    ) -> IdnCompressResult<()> {
        let header = IdnIdentifiersHeader {
            length: data.len() as u32,
            compression: compression_method,
        };
        let header = IdnSliceHeader::Identifiers(header);

        self.write_slice_header(header)?;
        self.data.write_all(data)?;

        Ok(())
    }

    pub fn write_sequence(&mut self, sequence: &FastqSequence, data: &[u8]) -> IdnCompressResult<()> {
        sequence.hash(&mut self.hasher);

        let header = IdnSequenceHeader {
            length: data.len() as u32,
            seq_len: sequence.len() as u32,
        };
        let header = IdnSliceHeader::Sequence(header);

        self.write_slice_header(header)?;
        self.data.write_all(data)?;

        Ok(())
    }

    pub fn write_switch_model(&mut self, index: u8) -> IdnCompressResult<()> {
        let header = IdnSwitchModelHeader { model_index: index };
        let header = IdnSliceHeader::SwitchModel(header);
        self.write_slice_header(header)
    }

    fn write_slice_header(&mut self, header: IdnSliceHeader) -> IdnCompressResult<()> {
        header.write_to(&mut self.data)?;
        Ok(())
    }
}
