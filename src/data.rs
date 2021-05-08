use bytes::Bytes;
use rml_rtmp::sessions::StreamMetadata;

pub enum RtmpInput {
    Media(Media),
    Metadata(StreamMetadata),
}

pub struct Media {
    pub media_type: MediaType,
    pub data: Bytes,
    pub timestamp: u32,
    pub can_be_dropped: bool,
}

pub enum MediaType {
    Video,
    Audio,
}
