use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HdmxCodecError {
    CorruptData,
    InvalidArgument,
}

impl fmt::Display for HdmxCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HdmxCodecError::CorruptData => f.write_str("hdmx data is corrupt"),
            HdmxCodecError::InvalidArgument => f.write_str("invalid hdmx codec argument"),
        }
    }
}

impl std::error::Error for HdmxCodecError {}

pub fn hdmx_encode(
    decoded: &[u8],
    hmtx: &[u8],
    hhea: &[u8],
    head: &[u8],
    maxp: &[u8],
) -> Result<Vec<u8>, HdmxCodecError> {
    if decoded.len() < 8 || head.len() < 20 || maxp.len() < 6 {
        return Err(HdmxCodecError::CorruptData);
    }

    let num_records = read_u16_be(decoded, 2)? as usize;
    let record_size = read_u32_be(decoded, 4)? as usize;
    let units_per_em = read_u16_be(head, 18)?;
    let num_glyphs = usize::from(read_u16_be(maxp, 4)?);
    let expected_size = 8usize
        .checked_add(
            num_records
                .checked_mul(record_size)
                .ok_or(HdmxCodecError::CorruptData)?,
        )
        .ok_or(HdmxCodecError::CorruptData)?;

    if units_per_em == 0 || record_size < num_glyphs + 2 || expected_size > decoded.len() {
        return Err(HdmxCodecError::CorruptData);
    }

    let mut mag = Vec::new();
    for record_index in 0..num_records {
        let record_offset = 8 + record_index * record_size;
        let ppem = decoded[record_offset];

        for glyph in 0..num_glyphs {
            let advance_width = read_advance_width(hmtx, hhea, glyph)?;
            let rounded_tt_aw =
                (((64 * i32::from(ppem) * i32::from(advance_width)) + i32::from(units_per_em) / 2)
                    / i32::from(units_per_em)
                    + 32)
                    / 64;
            let surprise = i32::from(decoded[record_offset + 2 + glyph]) - rounded_tt_aw;
            write_magnitude_value(&mut mag, surprise)?;
        }
    }

    let mut result_size = 8 + num_records * 2 + mag.len();
    if result_size < 12 {
        result_size = 12;
    }
    let mut output = vec![0u8; result_size];
    output[2..4].copy_from_slice(&(num_records as u16).to_be_bytes());
    output[4..8].copy_from_slice(&(record_size as u32).to_be_bytes());
    for record_index in 0..num_records {
        let record_offset = 8 + record_index * record_size;
        output[8 + record_index * 2] = decoded[record_offset];
        output[8 + record_index * 2 + 1] = decoded[record_offset + 1];
    }
    output[8 + num_records * 2..8 + num_records * 2 + mag.len()].copy_from_slice(&mag);
    Ok(output)
}

pub fn hdmx_decode(
    encoded: &[u8],
    hmtx: &[u8],
    hhea: &[u8],
    head: &[u8],
    maxp: &[u8],
) -> Result<Vec<u8>, HdmxCodecError> {
    if encoded.len() < 12 || head.len() < 20 || maxp.len() < 6 {
        return Err(HdmxCodecError::CorruptData);
    }

    let num_records = usize::from(read_u16_be(encoded, 2)?);
    let record_size = read_u32_be(encoded, 4)? as usize;
    let units_per_em = read_u16_be(head, 18)?;
    let num_glyphs = usize::from(read_u16_be(maxp, 4)?);
    let mag_data_offset = 8usize
        .checked_add(num_records.checked_mul(2).ok_or(HdmxCodecError::CorruptData)?)
        .ok_or(HdmxCodecError::CorruptData)?;
    if mag_data_offset > encoded.len() || units_per_em == 0 {
        return Err(HdmxCodecError::CorruptData);
    }

    let output_size = 8usize
        .checked_add(
            num_records
                .checked_mul(record_size)
                .ok_or(HdmxCodecError::CorruptData)?,
        )
        .ok_or(HdmxCodecError::CorruptData)?;
    let mut output = vec![0u8; output_size];
    output[2..4].copy_from_slice(&(num_records as u16).to_be_bytes());
    output[4..8].copy_from_slice(&(record_size as u32).to_be_bytes());

    let mut mag_pos = mag_data_offset;
    for record_index in 0..num_records {
        let header_offset = 8 + record_index * 2;
        if header_offset + 1 >= encoded.len() {
            return Err(HdmxCodecError::CorruptData);
        }
        let ppem = encoded[header_offset];
        let max_width = encoded[header_offset + 1];
        let record_offset = 8 + record_index * record_size;
        if record_offset + record_size > output.len() {
            return Err(HdmxCodecError::CorruptData);
        }

        output[record_offset] = ppem;
        output[record_offset + 1] = max_width;

        for glyph in 0..num_glyphs {
            let advance_width = read_advance_width(hmtx, hhea, glyph)?;
            let rounded_tt_aw =
                (((64 * i32::from(ppem) * i32::from(advance_width)) + i32::from(units_per_em) / 2)
                    / i32::from(units_per_em)
                    + 32)
                    / 64;
            let surprise = read_magnitude_value(encoded, &mut mag_pos)?;
            let width = (rounded_tt_aw + surprise).clamp(0, 255) as u8;
            output[record_offset + 2 + glyph] = width;
        }
    }

    Ok(output)
}

fn read_u16_be(bytes: &[u8], offset: usize) -> Result<u16, HdmxCodecError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(HdmxCodecError::CorruptData)?;
    Ok(u16::from_be_bytes(slice.try_into().expect("slice should fit")))
}

fn read_u32_be(bytes: &[u8], offset: usize) -> Result<u32, HdmxCodecError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(HdmxCodecError::CorruptData)?;
    Ok(u32::from_be_bytes(slice.try_into().expect("slice should fit")))
}

fn read_advance_width(hmtx: &[u8], hhea: &[u8], glyph: usize) -> Result<u16, HdmxCodecError> {
    if hhea.len() >= 36 {
        let num_h_metrics = usize::from(read_u16_be(hhea, 34)?);
        if num_h_metrics > 0 {
            if glyph < num_h_metrics {
                let offset = glyph.checked_mul(4).ok_or(HdmxCodecError::CorruptData)?;
                return read_u16_be(hmtx, offset);
            }
            let offset = (num_h_metrics - 1)
                .checked_mul(4)
                .ok_or(HdmxCodecError::CorruptData)?;
            return read_u16_be(hmtx, offset);
        }
    }

    let offset = glyph.checked_mul(4).ok_or(HdmxCodecError::CorruptData)?;
    read_u16_be(hmtx, offset)
}

fn write_magnitude_value(output: &mut Vec<u8>, value: i32) -> Result<(), HdmxCodecError> {
    if (-139..=111).contains(&value) {
        output.push((value + 139) as u8);
        return Ok(());
    }
    if (108..=875).contains(&value) {
        let biased = value - 108;
        output.push((251 + biased / 256) as u8);
        output.push((biased % 256) as u8);
        return Ok(());
    }
    if (-363..=-108).contains(&value) {
        output.push(254);
        output.push((-(value + 108)) as u8);
        return Ok(());
    }
    if value <= -364 {
        let biased = -(value + 364);
        output.push(255);
        output.push(((biased >> 8) & 0xFF) as u8);
        output.push((biased & 0xFF) as u8);
        return Ok(());
    }

    Err(HdmxCodecError::CorruptData)
}

fn read_magnitude_value(bytes: &[u8], pos: &mut usize) -> Result<i32, HdmxCodecError> {
    let first = *bytes.get(*pos).ok_or(HdmxCodecError::CorruptData)?;
    *pos += 1;

    if first < 251 {
        return Ok(i32::from(first) - 139);
    }
    if first < 254 {
        let second = *bytes.get(*pos).ok_or(HdmxCodecError::CorruptData)?;
        *pos += 1;
        return Ok(i32::from(first - 251) * 256 + i32::from(second) + 108);
    }
    if first == 254 {
        let second = *bytes.get(*pos).ok_or(HdmxCodecError::CorruptData)?;
        *pos += 1;
        return Ok(-i32::from(second) - 108);
    }

    let byte1 = *bytes.get(*pos).ok_or(HdmxCodecError::CorruptData)?;
    let byte2 = *bytes.get(*pos + 1).ok_or(HdmxCodecError::CorruptData)?;
    *pos += 2;
    Ok(-(i32::from(byte1) * 256 + i32::from(byte2)) - 364)
}
