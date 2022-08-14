use std::io::{Seek, Write};

use binrw::BinWrite;
use itertools::Itertools;

use crate::idn::compressor::IdnCompressResult;
use crate::idn::data::{IdnHeader, IdnMetadataHeader, IdnMetadataItem, IdnModelsMetadata};
use crate::model::ModelIdentifier;

#[derive(Debug)]
pub(super) struct IdnWriter<W> {
    writer: W,
    metadata_items: Option<Vec<IdnMetadataItem>>,
}

impl<W: Write + Seek> IdnWriter<W> {
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            metadata_items: Some(Vec::new()),
        }
    }

    pub fn write_header(&mut self, version: u8) -> IdnCompressResult<()> {
        let header = IdnHeader { version };
        header.write_to(&mut self.writer)?;
        Ok(())
    }

    pub fn add_models_metadata(&mut self, model_identifiers: &[ModelIdentifier]) {
        let metadata = IdnModelsMetadata {
            num_models: model_identifiers.len() as u8,
            model_identifiers: model_identifiers.iter().map_into().collect(),
        };

        let item = IdnMetadataItem::Models(metadata);
        self.metadata_items
            .as_mut()
            .expect("Metadata already written")
            .push(item);
    }

    pub fn write_metadata(&mut self) -> IdnCompressResult<()> {
        let metadata_items = self
            .metadata_items
            .take()
            .expect("Metadata already written");
        let metadata_header = IdnMetadataHeader {
            item_num: metadata_items.len() as u8,
        };

        metadata_header.write_to(&mut self.writer)?;
        for item in metadata_items {
            item.write_to(&mut self.writer)?;
        }

        Ok(())
    }

    fn is_metadata_written(&self) -> bool {
        self.metadata_items.is_none()
    }

    pub fn writer_for_block(&mut self) -> &mut W {
        debug_assert!(self.is_metadata_written());

        &mut self.writer
    }
}
