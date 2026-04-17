use core::fmt;

const SIMPLE_GLYPH_BBOX_MARKER: i16 = 0x7FFF;

const TT_ON_CURVE: u8 = 0x01;
const TT_X_SHORT: u8 = 0x02;
const TT_Y_SHORT: u8 = 0x04;
const TT_REPEAT_FLAG: u8 = 0x08;
const TT_X_SAME: u8 = 0x10;
const TT_Y_SAME: u8 = 0x20;

const TT_NPUSHB: u8 = 0x40;
const TT_NPUSHW: u8 = 0x41;
const TT_PUSHB_BASE: u8 = 0xB0;
const TT_PUSHW_BASE: u8 = 0xB8;

const COMPOSITE_ARG_WORDS: u16 = 0x0001;
const COMPOSITE_HAVE_SCALE: u16 = 0x0008;
const COMPOSITE_MORE_COMPONENTS: u16 = 0x0020;
const COMPOSITE_HAVE_XY_SCALE: u16 = 0x0040;
const COMPOSITE_HAVE_TWO_BY_TWO: u16 = 0x0080;
const COMPOSITE_HAVE_INSTRUCTIONS: u16 = 0x0100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlyfDecodeError {
    CorruptData,
}

impl fmt::Display for GlyfDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlyfDecodeError::CorruptData => f.write_str("glyf MTX streams are corrupt"),
        }
    }
}

impl std::error::Error for GlyfDecodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedGlyfData {
    pub glyf_data: Vec<u8>,
    pub loca_data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default)]
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
    fn len(&self) -> usize {
        self.bytes.len()
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

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

pub fn decode_glyf(
    glyf_stream: &[u8],
    push_stream: &[u8],
    code_stream: &[u8],
    index_to_loca_format: i16,
    num_glyphs: u16,
) -> Result<DecodedGlyfData, GlyfDecodeError> {
    if !matches!(index_to_loca_format, 0 | 1) {
        return Err(GlyfDecodeError::CorruptData);
    }

    let mut glyf_writer = ByteWriter::default();
    let mut glyf_pos = 0usize;
    let mut push_pos = 0usize;
    let mut code_pos = 0usize;

    let loca_entry_size = if index_to_loca_format == 0 { 2 } else { 4 };
    let mut loca = vec![0u8; (usize::from(num_glyphs) + 1) * loca_entry_size];

    for glyph_id in 0..=num_glyphs {
        write_loca_entry(
            &mut loca,
            usize::from(glyph_id),
            glyf_writer.len(),
            index_to_loca_format,
        )?;

        if glyph_id == num_glyphs {
            break;
        }

        let contour_count = read_i16_be(glyf_stream, &mut glyf_pos)?;
        if contour_count == 0 {
            continue;
        }

        let glyph_start = glyf_writer.len();
        match contour_count {
            -1 => decode_composite_glyph(
                glyf_stream,
                &mut glyf_pos,
                push_stream,
                &mut push_pos,
                code_stream,
                &mut code_pos,
                &mut glyf_writer,
            )?,
            _ => decode_simple_glyph(
                glyf_stream,
                &mut glyf_pos,
                push_stream,
                &mut push_pos,
                code_stream,
                &mut code_pos,
                contour_count,
                &mut glyf_writer,
            )?,
        }

        let alignment = if index_to_loca_format == 0 { 2 } else { 4 };
        while (glyf_writer.len() - glyph_start) % alignment != 0 {
            glyf_writer.write_u8(0);
        }
    }

    if glyf_pos != glyf_stream.len()
        || push_pos != push_stream.len()
        || code_pos != code_stream.len()
    {
        return Err(GlyfDecodeError::CorruptData);
    }

    Ok(DecodedGlyfData {
        glyf_data: glyf_writer.into_inner(),
        loca_data: loca,
    })
}

fn decode_simple_glyph(
    glyf_stream: &[u8],
    glyf_pos: &mut usize,
    push_stream: &[u8],
    push_pos: &mut usize,
    code_stream: &[u8],
    code_pos: &mut usize,
    mut contour_count: i16,
    glyf_writer: &mut ByteWriter,
) -> Result<(), GlyfDecodeError> {
    let mut x_min = 0i16;
    let mut y_min = 0i16;
    let mut x_max = 0i16;
    let mut y_max = 0i16;
    let mut explicit_bbox = false;
    if contour_count == SIMPLE_GLYPH_BBOX_MARKER {
        explicit_bbox = true;
        contour_count = read_i16_be(glyf_stream, glyf_pos)?;
        x_min = read_i16_be(glyf_stream, glyf_pos)?;
        y_min = read_i16_be(glyf_stream, glyf_pos)?;
        x_max = read_i16_be(glyf_stream, glyf_pos)?;
        y_max = read_i16_be(glyf_stream, glyf_pos)?;
    }

    if contour_count < 0 {
        return Err(GlyfDecodeError::CorruptData);
    }

    let contour_count_usize =
        usize::try_from(contour_count).map_err(|_| GlyfDecodeError::CorruptData)?;
    let mut end_pts = Vec::with_capacity(contour_count_usize);
    let mut total_points = 0usize;
    for contour_index in 0..contour_count_usize {
        let mut contour_points = usize::from(read_255_ushort(glyf_stream, glyf_pos)?);
        if contour_index == 0 {
            contour_points += 1;
        }
        if contour_points == 0 {
            return Err(GlyfDecodeError::CorruptData);
        }
        total_points = total_points
            .checked_add(contour_points)
            .ok_or(GlyfDecodeError::CorruptData)?;
        if total_points > usize::from(u16::MAX) + 1 {
            return Err(GlyfDecodeError::CorruptData);
        }
        end_pts.push(u16::try_from(total_points - 1).map_err(|_| GlyfDecodeError::CorruptData)?);
    }

    let mut points = vec![DecodedPoint::default(); total_points];
    if total_points > 0 {
        let flags = slice(glyf_stream, *glyf_pos, total_points)?;
        let mut payload_pos = *glyf_pos + total_points;
        let mut last_x = 0i32;
        let mut last_y = 0i32;

        for (index, flag) in flags.iter().copied().enumerate() {
            let (dx, dy, on_curve) = decode_triplet(flag, glyf_stream, &mut payload_pos)?;
            last_x = last_x.checked_add(dx).ok_or(GlyfDecodeError::CorruptData)?;
            last_y = last_y.checked_add(dy).ok_or(GlyfDecodeError::CorruptData)?;
            ensure_i16(last_x)?;
            ensure_i16(last_y)?;

            points[index] = DecodedPoint {
                x: last_x,
                y: last_y,
                on_curve,
            };

            if !explicit_bbox || index == 0 {
                if index == 0 {
                    x_min = last_x as i16;
                    x_max = last_x as i16;
                    y_min = last_y as i16;
                    y_max = last_y as i16;
                } else {
                    x_min = x_min.min(last_x as i16);
                    x_max = x_max.max(last_x as i16);
                    y_min = y_min.min(last_y as i16);
                    y_max = y_max.max(last_y as i16);
                }
            }
        }

        *glyf_pos = payload_pos;
    }

    let mut instructions = Vec::new();
    if contour_count > 0 {
        let push_count = usize::from(read_255_ushort(glyf_stream, glyf_pos)?);
        let code_size = usize::from(read_255_ushort(glyf_stream, glyf_pos)?);
        instructions = build_instruction_stream(
            push_stream,
            push_pos,
            code_stream,
            code_pos,
            push_count,
            code_size,
        )?;
        if instructions.len() > usize::from(u16::MAX) {
            return Err(GlyfDecodeError::CorruptData);
        }
    }

    glyf_writer.write_i16_be(contour_count);
    glyf_writer.write_i16_be(x_min);
    glyf_writer.write_i16_be(y_min);
    glyf_writer.write_i16_be(x_max);
    glyf_writer.write_i16_be(y_max);
    for end_pt in end_pts {
        glyf_writer.write_u16_be(end_pt);
    }
    glyf_writer
        .write_u16_be(u16::try_from(instructions.len()).map_err(|_| GlyfDecodeError::CorruptData)?);
    glyf_writer.write_bytes(&instructions);
    if total_points > 0 {
        append_simple_points(glyf_writer, &points)?;
    }

    Ok(())
}

fn decode_composite_glyph(
    glyf_stream: &[u8],
    glyf_pos: &mut usize,
    push_stream: &[u8],
    push_pos: &mut usize,
    code_stream: &[u8],
    code_pos: &mut usize,
    glyf_writer: &mut ByteWriter,
) -> Result<(), GlyfDecodeError> {
    glyf_writer.write_i16_be(-1);
    for _ in 0..4 {
        glyf_writer.write_i16_be(read_i16_be(glyf_stream, glyf_pos)?);
    }

    let flags = loop {
        let flags = read_u16_be(glyf_stream, glyf_pos)?;
        let glyph_index = read_u16_be(glyf_stream, glyf_pos)?;
        glyf_writer.write_u16_be(flags);
        glyf_writer.write_u16_be(glyph_index);

        let component_bytes = composite_component_extra_bytes(flags);
        let component = slice(glyf_stream, *glyf_pos, component_bytes)?;
        glyf_writer.write_bytes(component);
        *glyf_pos += component_bytes;

        if flags & COMPOSITE_MORE_COMPONENTS == 0 {
            break flags;
        }
    };

    if flags & COMPOSITE_HAVE_INSTRUCTIONS != 0 {
        let push_count = usize::from(read_255_ushort(glyf_stream, glyf_pos)?);
        let code_size = usize::from(read_255_ushort(glyf_stream, glyf_pos)?);
        let instructions = build_instruction_stream(
            push_stream,
            push_pos,
            code_stream,
            code_pos,
            push_count,
            code_size,
        )?;
        if instructions.len() > usize::from(u16::MAX) {
            return Err(GlyfDecodeError::CorruptData);
        }
        glyf_writer.write_u16_be(
            u16::try_from(instructions.len()).map_err(|_| GlyfDecodeError::CorruptData)?,
        );
        glyf_writer.write_bytes(&instructions);
    }

    Ok(())
}

fn build_instruction_stream(
    push_stream: &[u8],
    push_pos: &mut usize,
    code_stream: &[u8],
    code_pos: &mut usize,
    push_count: usize,
    code_size: usize,
) -> Result<Vec<u8>, GlyfDecodeError> {
    let push_values = decode_push_values(push_stream, push_pos, push_count)?;
    let mut writer = ByteWriter::default();
    if !push_values.is_empty() {
        append_push_instructions(&mut writer, &push_values)?;
    }

    let code_bytes = slice(code_stream, *code_pos, code_size)?;
    writer.write_bytes(code_bytes);
    *code_pos += code_size;
    Ok(writer.into_inner())
}

fn decode_push_values(
    push_stream: &[u8],
    push_pos: &mut usize,
    push_count: usize,
) -> Result<Vec<i16>, GlyfDecodeError> {
    let mut values = Vec::with_capacity(push_count);
    while values.len() < push_count {
        let code = *push_stream
            .get(*push_pos)
            .ok_or(GlyfDecodeError::CorruptData)?;
        if matches!(code, 251 | 252) {
            let expansion = if code == 251 { 3usize } else { 5usize };
            if values.len() < 2 || values.len() + expansion > push_count {
                return Err(GlyfDecodeError::CorruptData);
            }

            let repeated_value = values[values.len() - 2];
            *push_pos += 1;
            values.push(repeated_value);
            values.push(read_255_short(push_stream, push_pos)?);
            values.push(repeated_value);
            if code == 252 {
                values.push(read_255_short(push_stream, push_pos)?);
                values.push(repeated_value);
            }
            continue;
        }

        values.push(read_255_short(push_stream, push_pos)?);
    }

    Ok(values)
}

fn append_push_instructions(
    writer: &mut ByteWriter,
    values: &[i16],
) -> Result<(), GlyfDecodeError> {
    let mut run_start = 0usize;
    while run_start < values.len() {
        let byte_sized = (0..=255).contains(&values[run_start]);
        let mut run_end = run_start + 1;
        while run_end < values.len() && (0..=255).contains(&values[run_end]) == byte_sized {
            run_end += 1;
        }
        append_push_run(writer, &values[run_start..run_end], byte_sized)?;
        run_start = run_end;
    }
    Ok(())
}

fn append_push_run(
    writer: &mut ByteWriter,
    values: &[i16],
    byte_sized: bool,
) -> Result<(), GlyfDecodeError> {
    let mut remaining = values;
    while !remaining.is_empty() {
        let chunk_len = remaining.len().min(255);
        let chunk = &remaining[..chunk_len];
        if byte_sized {
            if chunk.len() <= 8 {
                writer.write_u8(
                    TT_PUSHB_BASE
                        + u8::try_from(chunk.len() - 1)
                            .map_err(|_| GlyfDecodeError::CorruptData)?,
                );
            } else {
                writer.write_u8(TT_NPUSHB);
                writer
                    .write_u8(u8::try_from(chunk.len()).map_err(|_| GlyfDecodeError::CorruptData)?);
            }
            for &value in chunk {
                writer.write_u8(u8::try_from(value).map_err(|_| GlyfDecodeError::CorruptData)?);
            }
        } else {
            if chunk.len() <= 8 {
                writer.write_u8(
                    TT_PUSHW_BASE
                        + u8::try_from(chunk.len() - 1)
                            .map_err(|_| GlyfDecodeError::CorruptData)?,
                );
            } else {
                writer.write_u8(TT_NPUSHW);
                writer
                    .write_u8(u8::try_from(chunk.len()).map_err(|_| GlyfDecodeError::CorruptData)?);
            }
            for &value in chunk {
                writer.write_i16_be(value);
            }
        }
        remaining = &remaining[chunk_len..];
    }

    Ok(())
}

fn append_simple_points(
    writer: &mut ByteWriter,
    points: &[DecodedPoint],
) -> Result<(), GlyfDecodeError> {
    let mut flags = Vec::with_capacity(points.len());
    let mut x_bytes = ByteWriter::default();
    let mut y_bytes = ByteWriter::default();
    let mut last_x = 0i32;
    let mut last_y = 0i32;

    for point in points {
        let dx = point.x - last_x;
        let dy = point.y - last_y;
        ensure_i16(dx)?;
        ensure_i16(dy)?;

        let mut flag = if point.on_curve { TT_ON_CURVE } else { 0 };
        if dx == 0 {
            flag |= TT_X_SAME;
        } else if (1..255).contains(&dx) {
            flag |= TT_X_SHORT | TT_X_SAME;
            x_bytes.write_u8(dx as u8);
        } else if (-255..0).contains(&dx) {
            flag |= TT_X_SHORT;
            x_bytes.write_u8((-dx) as u8);
        } else {
            x_bytes.write_i16_be(dx as i16);
        }

        if dy == 0 {
            flag |= TT_Y_SAME;
        } else if (1..255).contains(&dy) {
            flag |= TT_Y_SHORT | TT_Y_SAME;
            y_bytes.write_u8(dy as u8);
        } else if (-255..0).contains(&dy) {
            flag |= TT_Y_SHORT;
            y_bytes.write_u8((-dy) as u8);
        } else {
            y_bytes.write_i16_be(dy as i16);
        }

        flags.push(flag);
        last_x = point.x;
        last_y = point.y;
    }

    append_compressed_flags(writer, &flags);
    writer.write_bytes(&x_bytes.into_inner());
    writer.write_bytes(&y_bytes.into_inner());
    Ok(())
}

fn append_compressed_flags(writer: &mut ByteWriter, flags: &[u8]) {
    let mut run_start = 0usize;
    while run_start < flags.len() {
        let flag = flags[run_start];
        let mut run_end = run_start + 1;
        while run_end < flags.len() && flags[run_end] == flag {
            run_end += 1;
        }
        append_flag_run(writer, flag, run_end - run_start);
        run_start = run_end;
    }
}

fn append_flag_run(writer: &mut ByteWriter, flag: u8, mut count: usize) {
    while count > 0 {
        let chunk = count.min(256);
        let mut encoded_flag = flag;
        if chunk > 1 {
            encoded_flag |= TT_REPEAT_FLAG;
        }
        writer.write_u8(encoded_flag);
        if chunk > 1 {
            writer.write_u8(u8::try_from(chunk - 1).expect("repeat chunk should fit"));
        }
        count -= chunk;
    }
}

fn decode_triplet(
    flag: u8,
    payload_stream: &[u8],
    payload_pos: &mut usize,
) -> Result<(i32, i32, bool), GlyfDecodeError> {
    let mut triplet_code = flag & 0x7F;
    let on_curve = flag & 0x80 == 0;
    if triplet_code < 10 {
        let b0 = read_u8(payload_stream, payload_pos)?;
        return Ok((
            0,
            with_sign(
                triplet_code,
                (i32::from(triplet_code & 0x0E) << 7) + i32::from(b0),
            ),
            on_curve,
        ));
    }
    if triplet_code < 20 {
        let b0 = read_u8(payload_stream, payload_pos)?;
        return Ok((
            with_sign(
                triplet_code,
                (i32::from((triplet_code - 10) & 0x0E) << 7) + i32::from(b0),
            ),
            0,
            on_curve,
        ));
    }
    if triplet_code < 84 {
        let b0 = read_u8(payload_stream, payload_pos)?;
        triplet_code -= 20;
        let dx = with_sign(
            flag,
            1 + i32::from(triplet_code & 0x30) + i32::from(b0 >> 4),
        );
        let dy = with_sign(
            flag >> 1,
            1 + (i32::from(triplet_code & 0x0C) << 2) + i32::from(b0 & 0x0F),
        );
        return Ok((dx, dy, on_curve));
    }
    if triplet_code < 120 {
        let b0 = read_u8(payload_stream, payload_pos)?;
        let b1 = read_u8(payload_stream, payload_pos)?;
        triplet_code -= 84;
        let dx = with_sign(
            flag,
            1 + (i32::from(triplet_code / 12) << 8) + i32::from(b0),
        );
        let dy = with_sign(
            flag >> 1,
            1 + (i32::from((triplet_code % 12) >> 2) << 8) + i32::from(b1),
        );
        return Ok((dx, dy, on_curve));
    }
    if triplet_code < 124 {
        let b0 = read_u8(payload_stream, payload_pos)?;
        let b1 = read_u8(payload_stream, payload_pos)?;
        let b2 = read_u8(payload_stream, payload_pos)?;
        let dx = with_sign(flag, (i32::from(b0) << 4) + i32::from(b1 >> 4));
        let dy = with_sign(flag >> 1, (i32::from(b1 & 0x0F) << 8) + i32::from(b2));
        return Ok((dx, dy, on_curve));
    }

    let b0 = read_u8(payload_stream, payload_pos)?;
    let b1 = read_u8(payload_stream, payload_pos)?;
    let b2 = read_u8(payload_stream, payload_pos)?;
    let b3 = read_u8(payload_stream, payload_pos)?;
    let dx = with_sign(flag, (i32::from(b0) << 8) + i32::from(b1));
    let dy = with_sign(flag >> 1, (i32::from(b2) << 8) + i32::from(b3));
    Ok((dx, dy, on_curve))
}

fn with_sign(sign_bits: u8, value: i32) -> i32 {
    if sign_bits & 1 != 0 {
        value
    } else {
        -value
    }
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

fn write_loca_entry(
    loca: &mut [u8],
    glyph_id: usize,
    offset: usize,
    index_to_loca_format: i16,
) -> Result<(), GlyfDecodeError> {
    match index_to_loca_format {
        0 => {
            if offset % 2 != 0 || offset / 2 > usize::from(u16::MAX) {
                return Err(GlyfDecodeError::CorruptData);
            }
            let start = glyph_id
                .checked_mul(2)
                .ok_or(GlyfDecodeError::CorruptData)?;
            loca[start..start + 2].copy_from_slice(
                &(u16::try_from(offset / 2).map_err(|_| GlyfDecodeError::CorruptData)?)
                    .to_be_bytes(),
            );
        }
        1 => {
            let start = glyph_id
                .checked_mul(4)
                .ok_or(GlyfDecodeError::CorruptData)?;
            loca[start..start + 4].copy_from_slice(
                &(u32::try_from(offset).map_err(|_| GlyfDecodeError::CorruptData)?).to_be_bytes(),
            );
        }
        _ => return Err(GlyfDecodeError::CorruptData),
    }
    Ok(())
}

fn read_255_ushort(bytes: &[u8], pos: &mut usize) -> Result<u16, GlyfDecodeError> {
    let code = read_u8(bytes, pos)?;
    match code {
        253 => read_u16_be(bytes, pos),
        254 => Ok(506 + u16::from(read_u8(bytes, pos)?)),
        255 => Ok(253 + u16::from(read_u8(bytes, pos)?)),
        value => Ok(u16::from(value)),
    }
}

fn read_255_short(bytes: &[u8], pos: &mut usize) -> Result<i16, GlyfDecodeError> {
    let mut code = read_u8(bytes, pos)?;
    if code == 253 {
        return read_i16_be(bytes, pos);
    }

    let negative = if code == 250 {
        code = read_u8(bytes, pos)?;
        true
    } else {
        false
    };

    let mut value = match code {
        254 => 500 + i16::from(read_u8(bytes, pos)?),
        255 => 250 + i16::from(read_u8(bytes, pos)?),
        0..=249 => i16::from(code),
        _ => return Err(GlyfDecodeError::CorruptData),
    };
    if negative {
        value = -value;
    }
    Ok(value)
}

fn read_u8(bytes: &[u8], pos: &mut usize) -> Result<u8, GlyfDecodeError> {
    let value = *bytes.get(*pos).ok_or(GlyfDecodeError::CorruptData)?;
    *pos += 1;
    Ok(value)
}

fn read_u16_be(bytes: &[u8], pos: &mut usize) -> Result<u16, GlyfDecodeError> {
    let slice = slice(bytes, *pos, 2)?;
    *pos += 2;
    Ok(u16::from_be_bytes(
        slice.try_into().expect("slice length should match"),
    ))
}

fn read_i16_be(bytes: &[u8], pos: &mut usize) -> Result<i16, GlyfDecodeError> {
    Ok(read_u16_be(bytes, pos)? as i16)
}

fn slice<'a>(bytes: &'a [u8], offset: usize, length: usize) -> Result<&'a [u8], GlyfDecodeError> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or(GlyfDecodeError::CorruptData)?,
        )
        .ok_or(GlyfDecodeError::CorruptData)
}

fn ensure_i16(value: i32) -> Result<(), GlyfDecodeError> {
    if (i16::MIN as i32..=i16::MAX as i32).contains(&value) {
        Ok(())
    } else {
        Err(GlyfDecodeError::CorruptData)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_u16_be(bytes: &mut Vec<u8>, value: u16) {
        bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn write_255_ushort_reference(bytes: &mut Vec<u8>, value: u16) {
        if value < 253 {
            bytes.push(value as u8);
        } else if value < 506 {
            bytes.push(255);
            bytes.push((value - 253) as u8);
        } else if value < 762 {
            bytes.push(254);
            bytes.push((value - 506) as u8);
        } else {
            bytes.push(253);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }

    fn write_255_short_reference(bytes: &mut Vec<u8>, value: i16) {
        if !(-749..=749).contains(&value) {
            bytes.push(253);
            bytes.extend_from_slice(&(value as u16).to_be_bytes());
            return;
        }

        let mut short_value = i32::from(value);
        if short_value < 0 {
            bytes.push(250);
            short_value = -short_value;
        }

        if short_value >= 250 {
            short_value -= 250;
            if short_value >= 250 {
                short_value -= 250;
                bytes.push(254);
            } else {
                bytes.push(255);
            }
        }

        bytes.push(short_value as u8);
    }

    fn write_triplet_reference(
        flag_dest: &mut Vec<u8>,
        coord_dest: &mut Vec<u8>,
        on_curve: bool,
        dx: i32,
        dy: i32,
    ) {
        let abs_x = dx.unsigned_abs();
        let abs_y = dy.unsigned_abs();
        let on_curve_bit = if on_curve { 0 } else { 128 };
        let x_sign_bit = if dx < 0 { 0 } else { 1 };
        let y_sign_bit = if dy < 0 { 0 } else { 1 };
        let xy_sign_bits = x_sign_bit + 2 * y_sign_bit;

        let (flag, payload) = if dx == 0 && abs_y < 1280 {
            (
                (on_curve_bit + (((abs_y & 0xF00) >> 7) as i32) + y_sign_bit) as u8,
                vec![(abs_y & 0xFF) as u8],
            )
        } else if dy == 0 && abs_x < 1280 {
            (
                (on_curve_bit + 10 + (((abs_x & 0xF00) >> 7) as i32) + x_sign_bit) as u8,
                vec![(abs_x & 0xFF) as u8],
            )
        } else if abs_x < 65 && abs_y < 65 {
            (
                (on_curve_bit
                    + 20
                    + ((abs_x as i32 - 1) & 0x30)
                    + (((abs_y as i32 - 1) & 0x30) >> 2)
                    + xy_sign_bits) as u8,
                vec![((((abs_x - 1) as u8) & 0x0F) << 4) | (((abs_y - 1) as u8) & 0x0F)],
            )
        } else if abs_x < 769 && abs_y < 769 {
            (
                (on_curve_bit
                    + 84
                    + 12 * (((abs_x - 1) & 0x300) >> 8) as i32
                    + (((abs_y - 1) & 0x300) >> 6) as i32
                    + xy_sign_bits) as u8,
                vec![((abs_x - 1) & 0xFF) as u8, ((abs_y - 1) & 0xFF) as u8],
            )
        } else if abs_x < 4096 && abs_y < 4096 {
            (
                (on_curve_bit + 120 + xy_sign_bits) as u8,
                vec![
                    (abs_x >> 4) as u8,
                    (((abs_x & 0x0F) as u8) << 4) | ((abs_y >> 8) as u8),
                    (abs_y & 0xFF) as u8,
                ],
            )
        } else {
            (
                (on_curve_bit + 124 + xy_sign_bits) as u8,
                vec![
                    ((abs_x >> 8) & 0xFF) as u8,
                    (abs_x & 0xFF) as u8,
                    ((abs_y >> 8) & 0xFF) as u8,
                    (abs_y & 0xFF) as u8,
                ],
            )
        };
        flag_dest.push(flag);
        coord_dest.extend_from_slice(&payload);
    }

    fn build_triangle_glyph_stream(push_count: u16, code_size: u16) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut flags = Vec::new();
        let mut coords = Vec::new();

        write_u16_be(&mut bytes, 1);
        write_255_ushort_reference(&mut bytes, 2);
        write_triplet_reference(&mut flags, &mut coords, true, 0, 10);
        write_triplet_reference(&mut flags, &mut coords, true, 10, 0);
        write_triplet_reference(&mut flags, &mut coords, true, 0, -10);
        bytes.extend_from_slice(&flags);
        bytes.extend_from_slice(&coords);
        write_255_ushort_reference(&mut bytes, push_count);
        write_255_ushort_reference(&mut bytes, code_size);
        bytes
    }

    #[test]
    fn read_255_ushort_examples_match_reference() {
        let cases = [
            (vec![0], 0u16),
            (vec![252], 252u16),
            (vec![255, 0], 253u16),
            (vec![254, 0], 506u16),
            (vec![253, 0x02, 0xFA], 762u16),
        ];

        for (bytes, expected) in cases {
            let mut pos = 0usize;
            let actual = read_255_ushort(&bytes, &mut pos).expect("decode should succeed");
            assert_eq!(actual, expected);
            assert_eq!(pos, bytes.len());
        }
    }

    #[test]
    fn read_255_short_examples_match_reference() {
        let values = [
            0i16, 249, 250, 499, 500, 749, 750, -1, -250, -500, -749, -750,
        ];

        for value in values {
            let mut bytes = Vec::new();
            write_255_short_reference(&mut bytes, value);
            let mut pos = 0usize;
            let actual = read_255_short(&bytes, &mut pos).expect("decode should succeed");
            assert_eq!(actual, value);
            assert_eq!(pos, bytes.len());
        }
    }

    #[test]
    fn decode_simple_triangle_glyph() {
        let glyf_stream = build_triangle_glyph_stream(0, 0);
        let decoded = decode_glyf(&glyf_stream, &[], &[], 0, 1).expect("decode should succeed");

        assert_eq!(decoded.glyf_data.len(), 20);
        assert_eq!(decoded.loca_data.len(), 4);
        assert_eq!(
            u16::from_be_bytes([decoded.loca_data[0], decoded.loca_data[1]]),
            0
        );
        assert_eq!(
            u16::from_be_bytes([decoded.loca_data[2], decoded.loca_data[3]]),
            10
        );
        assert_eq!(
            u16::from_be_bytes([decoded.glyf_data[0], decoded.glyf_data[1]]),
            1
        );
        assert_eq!(
            u16::from_be_bytes([decoded.glyf_data[10], decoded.glyf_data[11]]),
            2
        );
        assert_eq!(
            u16::from_be_bytes([decoded.glyf_data[12], decoded.glyf_data[13]]),
            0
        );
    }

    #[test]
    fn decode_rejects_trailing_stream_bytes() {
        let mut glyf_stream = build_triangle_glyph_stream(0, 0);
        glyf_stream.push(0xAA);

        let err = decode_glyf(&glyf_stream, &[], &[], 0, 1).unwrap_err();

        assert_eq!(err, GlyfDecodeError::CorruptData);
    }

    #[test]
    fn decode_rejects_truncated_push_stream() {
        let glyf_stream = build_triangle_glyph_stream(5, 0);
        let err = decode_glyf(&glyf_stream, &[11, 22], &[], 0, 1).unwrap_err();

        assert_eq!(err, GlyfDecodeError::CorruptData);
    }

    #[test]
    fn decode_rejects_truncated_code_stream() {
        let glyf_stream = build_triangle_glyph_stream(0, 1);
        let err = decode_glyf(&glyf_stream, &[], &[], 0, 1).unwrap_err();

        assert_eq!(err, GlyfDecodeError::CorruptData);
    }
}
