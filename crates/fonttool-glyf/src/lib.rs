mod decode;
mod encode;

pub use decode::{decode_glyf, DecodedGlyfData, GlyfDecodeError};
pub use encode::{encode_glyf, EncodedGlyfData, GlyfEncodeError};
