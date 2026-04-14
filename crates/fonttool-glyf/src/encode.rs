use core::fmt;

const TT_ON_CURVE: u8 = 0x01;
const TT_X_SHORT: u8 = 0x02;
const TT_Y_SHORT: u8 = 0x04;
const TT_REPEAT_FLAG: u8 = 0x08;
const TT_X_SAME: u8 = 0x10;
const TT_Y_SAME: u8 = 0x20;

const TT_NPUSHB: u8 = 0x40;
const TT_NPUSHW: u8 = 0x41;
const TT_PUSHB_BASE: u8 = 0xB0;

const COMPOSITE_ARG_WORDS: u16 = 0x0001;
const COMPOSITE_HAVE_SCALE: u16 = 0x0008;
const COMPOSITE_MORE_COMPONENTS: u16 = 0x0020;
const COMPOSITE_HAVE_XY_SCALE: u16 = 0x0040;
const COMPOSITE_HAVE_TWO_BY_TWO: u16 = 0x0080;
const COMPOSITE_HAVE_INSTRUCTIONS: u16 = 0x0100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlyfEncodeError {
    InvalidArgument,
    CorruptData,
}

impl fmt::Display for GlyfEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlyfEncodeError::InvalidArgument => f.write_str("invalid glyf encode argument"),
            GlyfEncodeError::CorruptData => f.write_str("glyf/loca data is corrupt"),
        }
    }
}

impl std::error::Error for GlyfEncodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedGlyfData {
    pub glyf_stream: Vec<u8>,
    pub push_stream: Vec<u8>,
    pub code_stream: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct DecodedPoint {
    x: i32,
    y: i32,
    on_curve: bool,
}

#[derive(Default)]
struct ByteWriter {
    bytes: Vec<u8>,
}

impl ByteWriter {
    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_u16_be(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn write_i16_be(&mut self, value: i16) {
        self.write_u16_be(value as u16);
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
}

pub fn encode_glyf(
    glyf_table: &[u8],
    loca_table: &[u8],
    index_to_loca_format: i16,
    num_glyphs: u16,
) -> Result<EncodedGlyfData, GlyfEncodeError> {
    let mut glyf_writer = ByteWriter::default();
    let mut push_writer = ByteWriter::default();
    let mut code_writer = ByteWriter::default();

    for glyph_id in 0..usize::from(num_glyphs) {
        let glyph_offset = read_loca_offset(loca_table, index_to_loca_format, glyph_id)?;
        let next_offset = read_loca_offset(loca_table, index_to_loca_format, glyph_id + 1)?;

        if next_offset < glyph_offset || next_offset > glyf_table.len() {
            return Err(GlyfEncodeError::CorruptData);
        }

        if glyph_offset == next_offset {
            glyf_writer.write_u16_be(0);
            continue;
        }

        let glyph = glyf_table
            .get(glyph_offset..next_offset)
            .ok_or(GlyfEncodeError::CorruptData)?;
        if glyph.len() < 10 {
            return Err(GlyfEncodeError::CorruptData);
        }

        let contour_count = read_i16_be(glyph, 0)?;
        match contour_count {
            0.. => encode_simple_glyph(
                glyph,
                contour_count,
                &mut glyf_writer,
                &mut push_writer,
                &mut code_writer,
            )?,
            -1 => {
                encode_composite_glyph(glyph, &mut glyf_writer, &mut push_writer, &mut code_writer)?
            }
            _ => return Err(GlyfEncodeError::CorruptData),
        }
    }

    Ok(EncodedGlyfData {
        glyf_stream: glyf_writer.into_inner(),
        push_stream: push_writer.into_inner(),
        code_stream: code_writer.into_inner(),
    })
}

fn read_loca_offset(
    loca_table: &[u8],
    index_to_loca_format: i16,
    glyph_id: usize,
) -> Result<usize, GlyfEncodeError> {
    match index_to_loca_format {
        0 => {
            let offset = glyph_id
                .checked_mul(2)
                .ok_or(GlyfEncodeError::CorruptData)?;
            let value = u32::from(read_u16_be(loca_table, offset)?)
                .checked_mul(2)
                .ok_or(GlyfEncodeError::CorruptData)?;
            usize::try_from(value).map_err(|_| GlyfEncodeError::CorruptData)
        }
        1 => {
            let offset = glyph_id
                .checked_mul(4)
                .ok_or(GlyfEncodeError::CorruptData)?;
            usize::try_from(read_u32_be(loca_table, offset)?)
                .map_err(|_| GlyfEncodeError::CorruptData)
        }
        _ => Err(GlyfEncodeError::CorruptData),
    }
}

fn encode_simple_glyph(
    glyph: &[u8],
    contour_count: i16,
    glyf_writer: &mut ByteWriter,
    push_writer: &mut ByteWriter,
    code_writer: &mut ByteWriter,
) -> Result<(), GlyfEncodeError> {
    if glyph.len() < 10 {
        return Err(GlyfEncodeError::CorruptData);
    }

    glyf_writer.write_i16_be(contour_count);
    if contour_count == 0 {
        return Ok(());
    }

    let contour_count = usize::try_from(contour_count).map_err(|_| GlyfEncodeError::CorruptData)?;
    let mut pos = 10usize;
    let mut total_points = 0usize;

    for contour_index in 0..contour_count {
        let end_point = usize::from(read_u16_be(glyph, pos)?);
        pos += 2;
        if end_point + 1 < total_points + 1 {
            return Err(GlyfEncodeError::CorruptData);
        }

        let contour_points = end_point + 1 - total_points;
        if contour_points == 0 {
            return Err(GlyfEncodeError::CorruptData);
        }

        total_points = total_points
            .checked_add(contour_points)
            .ok_or(GlyfEncodeError::CorruptData)?;
        let encoded_points = if contour_index == 0 {
            contour_points - 1
        } else {
            contour_points
        };
        glyf_writer.write_bytes(&encode_255_ushort(
            u16::try_from(encoded_points).map_err(|_| GlyfEncodeError::CorruptData)?,
        )?);
    }

    if total_points == 0 {
        return Err(GlyfEncodeError::CorruptData);
    }

    let instruction_length = usize::from(read_u16_be(glyph, pos)?);
    pos += 2;
    let instructions = glyph
        .get(pos..pos + instruction_length)
        .ok_or(GlyfEncodeError::CorruptData)?;
    pos += instruction_length;

    let mut flags = Vec::with_capacity(total_points);
    while flags.len() < total_points {
        let flag = *glyph.get(pos).ok_or(GlyfEncodeError::CorruptData)?;
        pos += 1;
        flags.push(flag);

        if flag & TT_REPEAT_FLAG != 0 {
            let repeat_count = usize::from(*glyph.get(pos).ok_or(GlyfEncodeError::CorruptData)?);
            pos += 1;
            if repeat_count > total_points - flags.len() {
                return Err(GlyfEncodeError::CorruptData);
            }
            for _ in 0..repeat_count {
                flags.push(flag);
            }
        }
    }

    let mut points = vec![
        DecodedPoint {
            x: 0,
            y: 0,
            on_curve: false,
        };
        total_points
    ];

    let mut last_x = 0i32;
    for (index, flag) in flags.iter().copied().enumerate() {
        let dx = if flag & TT_X_SHORT != 0 {
            let value = i32::from(*glyph.get(pos).ok_or(GlyfEncodeError::CorruptData)?);
            pos += 1;
            if flag & TT_X_SAME != 0 {
                value
            } else {
                -value
            }
        } else if flag & TT_X_SAME == 0 {
            let value = i32::from(read_i16_be(glyph, pos)?);
            pos += 2;
            value
        } else {
            0
        };

        last_x = last_x.checked_add(dx).ok_or(GlyfEncodeError::CorruptData)?;
        if !(i16::MIN as i32..=i16::MAX as i32).contains(&last_x) {
            return Err(GlyfEncodeError::CorruptData);
        }

        points[index].x = last_x;
        points[index].on_curve = flag & TT_ON_CURVE != 0;
    }

    let mut last_y = 0i32;
    for (index, flag) in flags.iter().copied().enumerate() {
        let dy = if flag & TT_Y_SHORT != 0 {
            let value = i32::from(*glyph.get(pos).ok_or(GlyfEncodeError::CorruptData)?);
            pos += 1;
            if flag & TT_Y_SAME != 0 {
                value
            } else {
                -value
            }
        } else if flag & TT_Y_SAME == 0 {
            let value = i32::from(read_i16_be(glyph, pos)?);
            pos += 2;
            value
        } else {
            0
        };

        last_y = last_y.checked_add(dy).ok_or(GlyfEncodeError::CorruptData)?;
        if !(i16::MIN as i32..=i16::MAX as i32).contains(&last_y) {
            return Err(GlyfEncodeError::CorruptData);
        }

        points[index].y = last_y;
    }

    if !glyph[pos..].iter().all(|byte| *byte == 0) {
        return Err(GlyfEncodeError::CorruptData);
    }

    let mut payload_writer = ByteWriter::default();
    let mut last_x = 0i32;
    let mut last_y = 0i32;
    for point in points {
        let dx = point.x - last_x;
        let dy = point.y - last_y;
        let (flag, payload) = encode_triplet(point.on_curve, dx, dy)?;
        glyf_writer.write_u8(flag);
        payload_writer.write_bytes(&payload);
        last_x = point.x;
        last_y = point.y;
    }

    glyf_writer.write_bytes(&payload_writer.into_inner());
    split_and_append_instructions(instructions, glyf_writer, push_writer, code_writer)
}

fn encode_composite_glyph(
    glyph: &[u8],
    glyf_writer: &mut ByteWriter,
    push_writer: &mut ByteWriter,
    code_writer: &mut ByteWriter,
) -> Result<(), GlyfEncodeError> {
    if glyph.len() < 10 {
        return Err(GlyfEncodeError::CorruptData);
    }

    glyf_writer.write_i16_be(-1);
    glyf_writer.write_bytes(&glyph[2..10]);

    let mut pos = 10usize;
    let mut flags;
    loop {
        let component_start = pos;
        flags = read_u16_be(glyph, pos)?;
        pos += 4;

        let extra_bytes = composite_component_extra_bytes(flags);
        let end = pos
            .checked_add(extra_bytes)
            .ok_or(GlyfEncodeError::CorruptData)?;
        let component = glyph
            .get(component_start..end)
            .ok_or(GlyfEncodeError::CorruptData)?;
        glyf_writer.write_bytes(component);
        pos = end;

        if flags & COMPOSITE_MORE_COMPONENTS == 0 {
            break;
        }
    }

    if flags & COMPOSITE_HAVE_INSTRUCTIONS != 0 {
        let instruction_length = usize::from(read_u16_be(glyph, pos)?);
        pos += 2;
        let instructions = glyph
            .get(pos..pos + instruction_length)
            .ok_or(GlyfEncodeError::CorruptData)?;
        pos += instruction_length;
        split_and_append_instructions(instructions, glyf_writer, push_writer, code_writer)?;
    }

    if !glyph[pos..].iter().all(|byte| *byte == 0) {
        return Err(GlyfEncodeError::CorruptData);
    }

    Ok(())
}

fn split_and_append_instructions(
    instructions: &[u8],
    glyf_writer: &mut ByteWriter,
    push_writer: &mut ByteWriter,
    code_writer: &mut ByteWriter,
) -> Result<(), GlyfEncodeError> {
    let (push_count, push_stream, code_stream) = split_push_code(instructions)?;
    glyf_writer.write_bytes(&encode_255_ushort(
        u16::try_from(push_count).map_err(|_| GlyfEncodeError::CorruptData)?,
    )?);
    glyf_writer.write_bytes(&encode_255_ushort(
        u16::try_from(code_stream.len()).map_err(|_| GlyfEncodeError::CorruptData)?,
    )?);
    push_writer.write_bytes(&push_stream);
    code_writer.write_bytes(&code_stream);
    Ok(())
}

fn split_push_code(instructions: &[u8]) -> Result<(usize, Vec<u8>, Vec<u8>), GlyfEncodeError> {
    let mut i = 0usize;
    let mut push_values = Vec::new();

    while i + 1 < instructions.len() {
        let mut ix = i + 1;
        let instr = instructions[i];
        let (count, value_size) = if instr == TT_NPUSHB || instr == TT_NPUSHW {
            (usize::from(instructions[ix]), usize::from((instr & 1) + 1))
        } else if (TT_PUSHB_BASE..0xC0).contains(&instr) {
            (
                usize::from(1 + (instr & 7)),
                usize::from(((instr & 8) >> 3) + 1),
            )
        } else {
            break;
        };

        ix += 1;
        let payload_size = count
            .checked_mul(value_size)
            .ok_or(GlyfEncodeError::CorruptData)?;
        if i.checked_add(payload_size)
            .ok_or(GlyfEncodeError::CorruptData)?
            > instructions.len()
        {
            break;
        }

        for _ in 0..count {
            match value_size {
                1 => {
                    let value = *instructions.get(ix).ok_or(GlyfEncodeError::CorruptData)?;
                    push_values.push(i16::from(value));
                    ix += 1;
                }
                2 => {
                    push_values.push(read_i16_be(instructions, ix)?);
                    ix += 2;
                }
                _ => return Err(GlyfEncodeError::CorruptData),
            }
        }

        i = ix;
    }

    let mut push_stream = ByteWriter::default();
    encode_push_sequence(&mut push_stream, &push_values)?;

    Ok((
        push_values.len(),
        push_stream.into_inner(),
        instructions[i..].to_vec(),
    ))
}

fn encode_push_sequence(writer: &mut ByteWriter, values: &[i16]) -> Result<(), GlyfEncodeError> {
    let mut hop_skip = 0usize;

    for (index, value) in values.iter().copied().enumerate() {
        if hop_skip & 1 == 0 {
            if hop_skip == 0
                && index >= 2
                && index + 2 < values.len()
                && value == values[index - 2]
                && value == values[index + 2]
            {
                if index + 4 < values.len() && value == values[index + 4] {
                    writer.write_u8(252);
                    hop_skip = 0x14;
                } else {
                    writer.write_u8(251);
                    hop_skip = 4;
                }
            } else {
                writer.write_bytes(&encode_255_short(value)?);
            }
        }

        hop_skip >>= 1;
    }

    Ok(())
}

fn encode_255_ushort(value: u16) -> Result<Vec<u8>, GlyfEncodeError> {
    let mut encoded = Vec::with_capacity(3);
    if value < 253 {
        encoded.push(u8::try_from(value).map_err(|_| GlyfEncodeError::CorruptData)?);
    } else if value < 506 {
        encoded.push(255);
        encoded.push(u8::try_from(value - 253).map_err(|_| GlyfEncodeError::CorruptData)?);
    } else if value < 762 {
        encoded.push(254);
        encoded.push(u8::try_from(value - 506).map_err(|_| GlyfEncodeError::CorruptData)?);
    } else {
        encoded.push(253);
        encoded.extend_from_slice(&value.to_be_bytes());
    }

    Ok(encoded)
}

fn encode_255_short(value: i16) -> Result<Vec<u8>, GlyfEncodeError> {
    let mut encoded = Vec::with_capacity(3);
    if !(-749..=749).contains(&value) {
        encoded.push(253);
        encoded.extend_from_slice(&(value as u16).to_be_bytes());
        return Ok(encoded);
    }

    let mut short_value = i32::from(value);
    if short_value < 0 {
        encoded.push(250);
        short_value = -short_value;
    }

    if short_value >= 250 {
        short_value -= 250;
        if short_value >= 250 {
            short_value -= 250;
            encoded.push(254);
        } else {
            encoded.push(255);
        }
    }

    encoded.push(u8::try_from(short_value).map_err(|_| GlyfEncodeError::CorruptData)?);
    Ok(encoded)
}

fn encode_triplet(on_curve: bool, dx: i32, dy: i32) -> Result<(u8, Vec<u8>), GlyfEncodeError> {
    let abs_x = dx.unsigned_abs();
    let abs_y = dy.unsigned_abs();
    let on_curve_bit = if on_curve { 0 } else { 128 };
    let x_sign_bit = if dx < 0 { 0 } else { 1 };
    let y_sign_bit = if dy < 0 { 0 } else { 1 };
    let xy_sign_bits = x_sign_bit + 2 * y_sign_bit;

    if dx == 0 && abs_y < 1280 {
        return Ok((
            (on_curve_bit + (((abs_y & 0xF00) >> 7) as i32) + y_sign_bit) as u8,
            vec![u8::try_from(abs_y & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?],
        ));
    }

    if dy == 0 && abs_x < 1280 {
        return Ok((
            (on_curve_bit + 10 + (((abs_x & 0xF00) >> 7) as i32) + x_sign_bit) as u8,
            vec![u8::try_from(abs_x & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?],
        ));
    }

    if abs_x < 65 && abs_y < 65 {
        return Ok((
            (on_curve_bit
                + 20
                + (i32::try_from(abs_x - 1).map_err(|_| GlyfEncodeError::CorruptData)? & 0x30)
                + ((i32::try_from(abs_y - 1).map_err(|_| GlyfEncodeError::CorruptData)? & 0x30)
                    >> 2)
                + xy_sign_bits) as u8,
            vec![
                (((u8::try_from(abs_x - 1).map_err(|_| GlyfEncodeError::CorruptData)? & 0x0F)
                    << 4)
                    | (u8::try_from(abs_y - 1).map_err(|_| GlyfEncodeError::CorruptData)? & 0x0F)),
            ],
        ));
    }

    if abs_x < 769 && abs_y < 769 {
        return Ok((
            (on_curve_bit
                + 84
                + 12 * (((abs_x - 1) & 0x300) >> 8) as i32
                + (((abs_y - 1) & 0x300) >> 6) as i32
                + xy_sign_bits) as u8,
            vec![
                u8::try_from((abs_x - 1) & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?,
                u8::try_from((abs_y - 1) & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?,
            ],
        ));
    }

    if abs_x < 4096 && abs_y < 4096 {
        return Ok((
            (on_curve_bit + 120 + xy_sign_bits) as u8,
            vec![
                u8::try_from(abs_x >> 4).map_err(|_| GlyfEncodeError::CorruptData)?,
                ((u8::try_from(abs_x & 0x0F).map_err(|_| GlyfEncodeError::CorruptData)? << 4)
                    | u8::try_from(abs_y >> 8).map_err(|_| GlyfEncodeError::CorruptData)?),
                u8::try_from(abs_y & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?,
            ],
        ));
    }

    Ok((
        (on_curve_bit + 124 + xy_sign_bits) as u8,
        vec![
            u8::try_from((abs_x >> 8) & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?,
            u8::try_from(abs_x & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?,
            u8::try_from((abs_y >> 8) & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?,
            u8::try_from(abs_y & 0xFF).map_err(|_| GlyfEncodeError::CorruptData)?,
        ],
    ))
}

fn composite_component_extra_bytes(flags: u16) -> usize {
    let mut bytes = if flags & COMPOSITE_ARG_WORDS != 0 {
        4
    } else {
        2
    };
    if flags & COMPOSITE_HAVE_SCALE != 0 {
        bytes += 2;
    } else if flags & COMPOSITE_HAVE_XY_SCALE != 0 {
        bytes += 4;
    } else if flags & COMPOSITE_HAVE_TWO_BY_TWO != 0 {
        bytes += 8;
    }
    bytes
}

fn read_u16_be(bytes: &[u8], offset: usize) -> Result<u16, GlyfEncodeError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(GlyfEncodeError::CorruptData)?;
    Ok(u16::from_be_bytes([slice[0], slice[1]]))
}

fn read_i16_be(bytes: &[u8], offset: usize) -> Result<i16, GlyfEncodeError> {
    Ok(read_u16_be(bytes, offset)? as i16)
}

fn read_u32_be(bytes: &[u8], offset: usize) -> Result<u32, GlyfEncodeError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(GlyfEncodeError::CorruptData)?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}
