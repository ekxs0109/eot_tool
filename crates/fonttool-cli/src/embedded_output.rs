use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedPayloadFormat {
    Mtx,
    Sfnt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedXorMode {
    Off,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedEotVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedOutputOptions {
    pub payload_format: EmbeddedPayloadFormat,
    pub xor_mode: EmbeddedXorMode,
    pub eot_version: EmbeddedEotVersion,
}

impl Default for EmbeddedOutputOptions {
    fn default() -> Self {
        Self {
            payload_format: EmbeddedPayloadFormat::Mtx,
            xor_mode: EmbeddedXorMode::Off,
            eot_version: EmbeddedEotVersion::V2,
        }
    }
}

pub fn embedded_output_allowed(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "eot" | "fntdata"))
}
