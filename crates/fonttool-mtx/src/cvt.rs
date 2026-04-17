use core::fmt;

const CVT_LOWESTCODE: i32 = 238;
const CVT_WORDCODE: u8 = 238;
const CVT_NEG0: u8 = 239;
const CVT_NEG8: u8 = 247;
const CVT_POS1: u8 = 248;
const CVT_POS8: u8 = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CvtCodecError {
    CorruptData,
    InvalidArgument,
}

impl fmt::Display for CvtCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CvtCodecError::CorruptData => f.write_str("cvt data is corrupt"),
            CvtCodecError::InvalidArgument => f.write_str("invalid cvt codec argument"),
        }
    }
}

impl std::error::Error for CvtCodecError {}

pub fn cvt_encode(decoded: &[u8]) -> Result<Vec<u8>, CvtCodecError> {
    if decoded.len() % 2 != 0 {
        return Err(CvtCodecError::CorruptData);
    }

    let num_entries = decoded.len() / 2;
    let mut output = Vec::with_capacity(2 + num_entries * 3);
    output.extend_from_slice(
        &u16::try_from(num_entries)
            .map_err(|_| CvtCodecError::InvalidArgument)?
            .to_be_bytes(),
    );

    let mut last_value = 0i16;
    for index in 0..num_entries {
        let offset = index * 2;
        let value = i16::from_be_bytes(
            decoded[offset..offset + 2]
                .try_into()
                .map_err(|_| CvtCodecError::CorruptData)?,
        );
        let delta = value.wrapping_sub(last_value);
        let abs_value = i32::from(delta).unsigned_abs() as i32;
        let code_index = abs_value / CVT_LOWESTCODE;

        if code_index <= 8 {
            if delta < 0 {
                output.push(
                    CVT_NEG0
                        .checked_add(
                            u8::try_from(code_index).map_err(|_| CvtCodecError::CorruptData)?,
                        )
                        .ok_or(CvtCodecError::CorruptData)?,
                );
                output.push(
                    u8::try_from(abs_value - code_index * CVT_LOWESTCODE)
                        .map_err(|_| CvtCodecError::CorruptData)?,
                );
            } else {
                if code_index > 0 {
                    output.push(
                        CVT_POS1
                            .checked_add(
                                u8::try_from(code_index - 1)
                                    .map_err(|_| CvtCodecError::CorruptData)?,
                            )
                            .ok_or(CvtCodecError::CorruptData)?,
                    );
                }
                output.push(
                    u8::try_from(abs_value - code_index * CVT_LOWESTCODE)
                        .map_err(|_| CvtCodecError::CorruptData)?,
                );
            }
        } else {
            output.push(CVT_WORDCODE);
            output.extend_from_slice(&(delta as u16).to_be_bytes());
        }

        last_value = value;
    }

    Ok(output)
}

pub fn cvt_decode(encoded: &[u8]) -> Result<Vec<u8>, CvtCodecError> {
    if encoded.len() < 2 {
        return Err(CvtCodecError::CorruptData);
    }

    let num_entries = usize::from(u16::from_be_bytes([encoded[0], encoded[1]]));
    let mut output = vec![0u8; num_entries * 2];
    let mut pos = 2usize;
    let mut last_value = 0i16;

    for index in 0..num_entries {
        if pos >= encoded.len() {
            return Err(CvtCodecError::CorruptData);
        }

        let code = encoded[pos];
        pos += 1;

        let delta = if code == CVT_WORDCODE {
            if pos + 1 >= encoded.len() {
                return Err(CvtCodecError::CorruptData);
            }
            let value = i16::from_be_bytes([encoded[pos], encoded[pos + 1]]);
            pos += 2;
            value
        } else if (CVT_NEG0..=CVT_NEG8).contains(&code) {
            if pos >= encoded.len() {
                return Err(CvtCodecError::CorruptData);
            }
            let code_index = i32::from(code - CVT_NEG0);
            let abs_value = code_index * CVT_LOWESTCODE + i32::from(encoded[pos]);
            pos += 1;
            -(abs_value as i16)
        } else if (CVT_POS1..=CVT_POS8).contains(&code) {
            if pos >= encoded.len() {
                return Err(CvtCodecError::CorruptData);
            }
            let code_index = i32::from(code - CVT_POS1 + 1);
            let abs_value = code_index * CVT_LOWESTCODE + i32::from(encoded[pos]);
            pos += 1;
            abs_value as i16
        } else {
            i16::from(code)
        };

        let value = last_value.wrapping_add(delta);
        let offset = index * 2;
        output[offset..offset + 2].copy_from_slice(&(value as u16).to_be_bytes());
        last_value = value;
    }

    Ok(output)
}
