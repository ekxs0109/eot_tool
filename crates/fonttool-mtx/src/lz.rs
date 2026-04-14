use core::fmt;

const PRELOAD_SIZE: usize = 7168;
const MAX_2BYTE_DIST: usize = 512;
const DIST_MIN: usize = 1;
const DIST_WIDTH: usize = 3;
const LEN_MIN: usize = 2;
const LEN_MIN3: usize = 3;
const LEN_WIDTH: usize = 3;
const BIT_RANGE: usize = LEN_WIDTH - 1;
const ROOT: usize = 1;
const MAX_DECOMPRESSED_LENGTH: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LzDecompressError {
    Truncated,
    OutputTooLarge,
    InvalidBackReference,
    MalformedStream,
}

impl fmt::Display for LzDecompressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LzDecompressError::Truncated => f.write_str("lz stream is truncated"),
            LzDecompressError::OutputTooLarge => f.write_str("lz output size is too large"),
            LzDecompressError::InvalidBackReference => {
                f.write_str("lz stream contains an invalid back-reference")
            }
            LzDecompressError::MalformedStream => f.write_str("lz stream is malformed"),
        }
    }
}

impl std::error::Error for LzDecompressError {}

#[derive(Debug, Clone)]
struct BitWriter {
    bytes: Vec<u8>,
    byte_buf: u8,
    bit_count: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            byte_buf: 0,
            bit_count: 0,
        }
    }

    fn write_bit(&mut self, bit: bool) {
        if bit {
            self.byte_buf |= 1 << (7 - self.bit_count);
        }
        self.bit_count += 1;
        if self.bit_count == 8 {
            self.bytes.push(self.byte_buf);
            self.byte_buf = 0;
            self.bit_count = 0;
        }
    }

    fn write_bits(&mut self, value: u32, count: usize) {
        for shift in (0..count).rev() {
            self.write_bit(((value >> shift) & 1) != 0);
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bit_count != 0 {
            self.bytes.push(self.byte_buf);
        }
        self.bytes
    }
}

#[derive(Debug, Clone)]
struct BitReader<'a> {
    bytes: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn read_bit(&mut self) -> Result<u8, LzDecompressError> {
        let byte = *self
            .bytes
            .get(self.byte_pos)
            .ok_or(LzDecompressError::Truncated)?;
        let bit = (byte >> (7 - self.bit_pos)) & 1;

        self.bit_pos += 1;
        if self.bit_pos >= 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }

        Ok(bit)
    }

    fn read_bits(&mut self, count: usize) -> Result<u32, LzDecompressError> {
        let mut result = 0u32;

        for shift in (0..count).rev() {
            result |= u32::from(self.read_bit()?) << shift;
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
struct TreeNode {
    up: usize,
    left: usize,
    right: usize,
    code: i32,
    weight: i32,
}

#[derive(Debug, Clone)]
struct HuffmanEncoder {
    tree: Vec<TreeNode>,
    symbol_index: Vec<usize>,
}

impl HuffmanEncoder {
    fn new(range: usize) -> Result<Self, LzDecompressError> {
        let bit_count2 = if range > 256 && range < 512 {
            bits_used(range - 257)
        } else {
            0
        };

        let mut tree = vec![TreeNode::default(); 2 * range];
        let mut symbol_index = vec![0usize; range];

        for (index, node) in tree.iter_mut().enumerate().skip(2) {
            node.up = index / 2;
            node.weight = 1;
        }

        for (index, node) in tree.iter_mut().enumerate().take(range).skip(1) {
            node.left = 2 * index;
            node.right = 2 * index + 1;
        }

        for index in 0..range {
            tree[index].code = -1;
            tree[range + index].code = index as i32;
            tree[range + index].left = usize::MAX;
            tree[range + index].right = usize::MAX;
            symbol_index[index] = range + index;
        }

        let mut encoder = Self { tree, symbol_index };
        encoder.init_weight(ROOT);

        if bit_count2 != 0 {
            encoder.update_weight(encoder.symbol_index[256]);
            encoder.update_weight(encoder.symbol_index[257]);
            for _ in 0..12 {
                encoder.update_weight(encoder.symbol_index[range - 3]);
            }
            for _ in 0..6 {
                encoder.update_weight(encoder.symbol_index[range - 2]);
            }
        } else {
            for _ in 0..2 {
                for symbol in 0..range {
                    encoder.update_weight(encoder.symbol_index[symbol]);
                }
            }
        }

        Ok(encoder)
    }

    fn init_weight(&mut self, index: usize) -> i32 {
        if self.tree[index].code < 0 {
            self.tree[index].weight =
                self.init_weight(self.tree[index].left) + self.init_weight(self.tree[index].right);
        }

        self.tree[index].weight
    }

    fn swap_nodes(&mut self, a: usize, b: usize) {
        let up_a = self.tree[a].up;
        let up_b = self.tree[b].up;
        self.tree.swap(a, b);
        self.tree[a].up = up_a;
        self.tree[b].up = up_b;

        let code_a = self.tree[a].code;
        if code_a < 0 {
            let left = self.tree[a].left;
            let right = self.tree[a].right;
            self.tree[left].up = a;
            self.tree[right].up = a;
        } else {
            self.symbol_index[code_a as usize] = a;
        }

        let code_b = self.tree[b].code;
        if code_b < 0 {
            let left = self.tree[b].left;
            let right = self.tree[b].right;
            self.tree[left].up = b;
            self.tree[right].up = b;
        } else {
            self.symbol_index[code_b as usize] = b;
        }
    }

    fn update_weight(&mut self, mut index: usize) {
        while index != ROOT {
            let mut weight = self.tree[index].weight;
            let mut candidate = index - 1;

            if self.tree[candidate].weight == weight {
                while self.tree[candidate].weight == weight {
                    candidate -= 1;
                }
                candidate += 1;

                if candidate > ROOT {
                    self.swap_nodes(index, candidate);
                    index = candidate;
                }
            }

            weight += 1;
            self.tree[index].weight = weight;
            index = self.tree[index].up;
        }

        self.tree[index].weight += 1;
    }

    fn write_symbol(
        &mut self,
        writer: &mut BitWriter,
        symbol: usize,
    ) -> Result<(), LzDecompressError> {
        if symbol >= self.symbol_index.len() {
            return Err(LzDecompressError::MalformedStream);
        }

        let mut index = self.symbol_index[symbol];
        let terminal_index = index;
        let mut stack = [false; 64];
        let mut depth = 0usize;

        while index != ROOT {
            let up = self.tree[index].up;
            stack[depth] = self.tree[up].right == index;
            depth += 1;
            index = up;
        }

        while depth > 0 {
            depth -= 1;
            writer.write_bit(stack[depth]);
        }

        self.update_weight(terminal_index);
        Ok(())
    }
}

impl Default for TreeNode {
    fn default() -> Self {
        Self {
            up: 0,
            left: 0,
            right: 0,
            code: 0,
            weight: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct HuffmanDecoder {
    tree: Vec<TreeNode>,
    symbol_index: Vec<usize>,
}

impl HuffmanDecoder {
    fn new(range: usize) -> Result<Self, LzDecompressError> {
        let bit_count2 = if range > 256 && range < 512 {
            bits_used(range - 257)
        } else {
            0
        };

        let mut tree = vec![TreeNode::default(); 2 * range];
        let mut symbol_index = vec![0usize; range];

        for (index, node) in tree.iter_mut().enumerate().skip(2) {
            node.up = index / 2;
            node.weight = 1;
        }

        for (index, node) in tree.iter_mut().enumerate().take(range).skip(1) {
            node.left = 2 * index;
            node.right = 2 * index + 1;
        }

        for index in 0..range {
            tree[index].code = -1;
            tree[range + index].code = index as i32;
            tree[range + index].left = usize::MAX;
            tree[range + index].right = usize::MAX;
            symbol_index[index] = range + index;
        }

        let mut decoder = Self { tree, symbol_index };
        decoder.init_weight(ROOT);

        if bit_count2 != 0 {
            decoder.update_weight(decoder.symbol_index[256]);
            decoder.update_weight(decoder.symbol_index[257]);
            for _ in 0..12 {
                decoder.update_weight(decoder.symbol_index[range - 3]);
            }
            for _ in 0..6 {
                decoder.update_weight(decoder.symbol_index[range - 2]);
            }
        } else {
            for _ in 0..2 {
                for symbol in 0..range {
                    decoder.update_weight(decoder.symbol_index[symbol]);
                }
            }
        }

        Ok(decoder)
    }

    fn init_weight(&mut self, index: usize) -> i32 {
        if self.tree[index].code < 0 {
            self.tree[index].weight =
                self.init_weight(self.tree[index].left) + self.init_weight(self.tree[index].right);
        }

        self.tree[index].weight
    }

    fn swap_nodes(&mut self, a: usize, b: usize) {
        let up_a = self.tree[a].up;
        let up_b = self.tree[b].up;
        self.tree.swap(a, b);
        self.tree[a].up = up_a;
        self.tree[b].up = up_b;

        let code_a = self.tree[a].code;
        if code_a < 0 {
            let left = self.tree[a].left;
            let right = self.tree[a].right;
            self.tree[left].up = a;
            self.tree[right].up = a;
        } else {
            self.symbol_index[code_a as usize] = a;
        }

        let code_b = self.tree[b].code;
        if code_b < 0 {
            let left = self.tree[b].left;
            let right = self.tree[b].right;
            self.tree[left].up = b;
            self.tree[right].up = b;
        } else {
            self.symbol_index[code_b as usize] = b;
        }
    }

    fn update_weight(&mut self, mut index: usize) {
        while index != ROOT {
            let mut weight = self.tree[index].weight;
            let mut candidate = index - 1;

            if self.tree[candidate].weight == weight {
                while self.tree[candidate].weight == weight {
                    candidate -= 1;
                }
                candidate += 1;

                if candidate > ROOT {
                    self.swap_nodes(index, candidate);
                    index = candidate;
                }
            }

            weight += 1;
            self.tree[index].weight = weight;
            index = self.tree[index].up;
        }

        self.tree[index].weight += 1;
    }

    fn read_symbol(&mut self, reader: &mut BitReader<'_>) -> Result<usize, LzDecompressError> {
        let mut index = ROOT;

        while self.tree[index].code < 0 {
            index = if reader.read_bit()? != 0 {
                self.tree[index].right
            } else {
                self.tree[index].left
            };
        }

        let symbol = self.tree[index].code as usize;
        self.update_weight(index);
        Ok(symbol)
    }
}

#[must_use]
pub fn decompress_lz(bytes: &[u8]) -> Result<Vec<u8>, LzDecompressError> {
    if bytes.len() < 4 {
        return Err(LzDecompressError::Truncated);
    }

    let mut reader = BitReader::new(bytes);
    let _ignored_run_length_flag = reader.read_bit()?;
    let decompressed_length = reader.read_bits(24)? as usize;

    if decompressed_length > MAX_DECOMPRESSED_LENGTH {
        return Err(LzDecompressError::OutputTooLarge);
    }

    if decompressed_length == 0 {
        return Ok(Vec::new());
    }

    let mut output = vec![0u8; PRELOAD_SIZE + decompressed_length];
    initialize_preload_buffer(&mut output[..PRELOAD_SIZE]);

    let num_dist_ranges = num_dist_ranges(decompressed_length);
    let dup2 = 256 + (1 << LEN_WIDTH) * num_dist_ranges;
    let dup4 = dup2 + 1;
    let dup6 = dup4 + 1;
    let num_syms = dup6 + 1;

    let mut dist_decoder = HuffmanDecoder::new(1 << DIST_WIDTH)?;
    let mut len_decoder = HuffmanDecoder::new(1 << LEN_WIDTH)?;
    let mut sym_decoder = HuffmanDecoder::new(num_syms)?;

    let mut output_pos = PRELOAD_SIZE;
    let output_limit = PRELOAD_SIZE + decompressed_length;

    while output_pos < output_limit {
        let symbol = sym_decoder.read_symbol(&mut reader)?;

        if symbol == dup2 {
            if output_pos < 2 {
                return Err(LzDecompressError::InvalidBackReference);
            }
            output[output_pos] = output[output_pos - 2];
            output_pos += 1;
            continue;
        }

        if symbol == dup4 {
            if output_pos < 4 {
                return Err(LzDecompressError::InvalidBackReference);
            }
            output[output_pos] = output[output_pos - 4];
            output_pos += 1;
            continue;
        }

        if symbol == dup6 {
            if output_pos < 6 {
                return Err(LzDecompressError::InvalidBackReference);
            }
            output[output_pos] = output[output_pos - 6];
            output_pos += 1;
            continue;
        }

        if symbol < 256 {
            output[output_pos] = symbol as u8;
            output_pos += 1;
            continue;
        }

        if symbol >= dup2 {
            return Err(LzDecompressError::MalformedStream);
        }

        let len_symbol = symbol - 256;
        let dist_range_index = len_symbol / (1 << LEN_WIDTH);
        let mut length =
            decode_length(&mut reader, &mut len_decoder, symbol, dist_range_index + 1)?;
        let dist = decode_distance(&mut reader, &mut dist_decoder, dist_range_index + 1)?;

        if dist >= MAX_2BYTE_DIST {
            length += LEN_MIN3 - LEN_MIN;
        }

        if length > output_limit - output_pos {
            return Err(LzDecompressError::MalformedStream);
        }

        if dist + length - 1 > output_pos {
            return Err(LzDecompressError::InvalidBackReference);
        }

        let copy_start = output_pos - dist - length + 1;
        for index in 0..length {
            output[output_pos] = output[copy_start + index];
            output_pos += 1;
        }
    }

    Ok(output[PRELOAD_SIZE..output_limit].to_vec())
}

#[must_use]
pub fn compress_lz_literals(bytes: &[u8]) -> Result<Vec<u8>, LzDecompressError> {
    let mut writer = BitWriter::new();
    writer.write_bit(false);
    writer.write_bits(
        u32::try_from(bytes.len()).map_err(|_| LzDecompressError::OutputTooLarge)?,
        24,
    );

    let num_dist_ranges = num_dist_ranges(bytes.len());
    let dup6 = 256 + (1 << LEN_WIDTH) * num_dist_ranges + 2;
    let num_syms = dup6 + 1;
    let mut sym_encoder = HuffmanEncoder::new(num_syms)?;

    for byte in bytes {
        sym_encoder.write_symbol(&mut writer, usize::from(*byte))?;
    }

    Ok(writer.finish())
}

fn initialize_preload_buffer(buffer: &mut [u8]) {
    let mut index = 0usize;

    for k in 0..32u8 {
        for j in 0..96u8 {
            buffer[index] = k;
            index += 1;
            buffer[index] = j;
            index += 1;
        }
    }

    for j in 0..=u8::MAX {
        if index >= PRELOAD_SIZE {
            break;
        }

        for _ in 0..4 {
            if index >= PRELOAD_SIZE {
                break;
            }
            buffer[index] = j;
            index += 1;
        }
    }
}

fn num_dist_ranges(length: usize) -> usize {
    let mut ranges = 1usize;
    let mut dist_max = DIST_MIN + (1 << (DIST_WIDTH * ranges)) - 1;

    while dist_max < length {
        ranges += 1;
        dist_max = DIST_MIN + (1 << (DIST_WIDTH * ranges)) - 1;
    }

    ranges
}

fn bits_used(x: usize) -> usize {
    for i in (2..=32).rev() {
        if x & (1 << (i - 1)) != 0 {
            return i;
        }
    }
    1
}

fn decode_length(
    reader: &mut BitReader<'_>,
    len_decoder: &mut HuffmanDecoder,
    symbol: usize,
    num_dist_ranges: usize,
) -> Result<usize, LzDecompressError> {
    let len_symbol = symbol - 256 - (num_dist_ranges - 1) * (1 << LEN_WIDTH);
    let has_more = (len_symbol >> 2) & 1;
    let bit1 = (len_symbol >> 1) & 1;
    let bit0 = len_symbol & 1;

    let mut value = (bit1 << 1) | bit0;
    let mut bits_decoded = 2usize;
    let mut total_bits = if has_more != 0 {
        BIT_RANGE * 2
    } else {
        BIT_RANGE
    };

    while bits_decoded < total_bits {
        let extra_symbol = len_decoder.read_symbol(reader)?;
        let extra_has_more = (extra_symbol >> 2) & 1;
        let extra_bit1 = (extra_symbol >> 1) & 1;
        let extra_bit0 = extra_symbol & 1;

        value = (value << 2) | (extra_bit1 << 1) | extra_bit0;
        bits_decoded += 2;

        if extra_has_more != 0 {
            total_bits += BIT_RANGE;
        }
    }

    Ok(value + LEN_MIN)
}

fn decode_distance(
    reader: &mut BitReader<'_>,
    dist_decoder: &mut HuffmanDecoder,
    num_dist_ranges: usize,
) -> Result<usize, LzDecompressError> {
    let mut dist = 0usize;

    for _ in 0..num_dist_ranges {
        let dist_bits = dist_decoder.read_symbol(reader)?;
        dist = (dist << DIST_WIDTH) | dist_bits;
    }

    Ok(dist + DIST_MIN)
}
