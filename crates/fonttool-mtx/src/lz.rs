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
const MAX_HASH_CHAIN: usize = 96;
const MAX_LITERAL_COST_CACHE: usize = 32;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncodeOp {
    Literal(u8),
    Dup2,
    Dup4,
    Dup6,
    Copy { dist: usize, len: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CopyCandidate {
    dist: usize,
    len: usize,
    gain: usize,
    cost_per_byte: usize,
}

#[derive(Debug, Clone, Default)]
struct HashNode {
    index: usize,
    next: Option<usize>,
}

#[derive(Debug, Clone)]
struct MatchFinder {
    history: Vec<u8>,
    heads: Vec<Option<usize>>,
    nodes: Vec<HashNode>,
}

struct DecisionContext<'a> {
    sym_encoder: &'a HuffmanEncoder,
    dist_encoder: &'a HuffmanEncoder,
    len_encoder: &'a HuffmanEncoder,
    max_dist: usize,
}

impl<'a> DecisionContext<'a> {
    fn new(
        sym_encoder: &'a HuffmanEncoder,
        dist_encoder: &'a HuffmanEncoder,
        len_encoder: &'a HuffmanEncoder,
        total_len: usize,
    ) -> Self {
        Self {
            sym_encoder,
            dist_encoder,
            len_encoder,
            max_dist: max_supported_distance(total_len),
        }
    }
}

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

    fn write_symbol_cost(&self, symbol: usize) -> usize {
        let mut index = self.symbol_index[symbol];
        let mut depth = 0usize;

        while index != ROOT {
            depth += 1;
            index = self.tree[index].up;
        }

        depth << 16
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

        let copy_span = dist
            .checked_add(length)
            .and_then(|value| value.checked_sub(1))
            .ok_or(LzDecompressError::InvalidBackReference)?;
        let copy_start = output_pos
            .checked_sub(copy_span)
            .ok_or(LzDecompressError::InvalidBackReference)?;
        for index in 0..length {
            output[output_pos] = output[copy_start + index];
            output_pos += 1;
        }
    }

    Ok(output[PRELOAD_SIZE..output_limit].to_vec())
}

#[must_use]
pub fn compress_lz(bytes: &[u8]) -> Result<Vec<u8>, LzDecompressError> {
    let literal_only = compress_lz_literals(bytes)?;
    let backreferences = match compress_lz_backreferences(bytes) {
        Ok(backreferences) => backreferences,
        Err(_) => return Ok(literal_only),
    };

    if backreferences.len() < literal_only.len() {
        Ok(backreferences)
    } else {
        Ok(literal_only)
    }
}

fn compress_lz_backreferences(bytes: &[u8]) -> Result<Vec<u8>, LzDecompressError> {
    let mut writer = BitWriter::new();
    writer.write_bit(false);
    writer.write_bits(
        u32::try_from(bytes.len()).map_err(|_| LzDecompressError::OutputTooLarge)?,
        24,
    );

    if bytes.is_empty() {
        return Ok(writer.finish());
    }

    let total_dist_ranges = num_dist_ranges(bytes.len());
    let dup2 = 256 + (1 << LEN_WIDTH) * total_dist_ranges;
    let dup4 = dup2 + 1;
    let dup6 = dup4 + 1;
    let num_syms = dup6 + 1;
    let mut sym_encoder = HuffmanEncoder::new(num_syms)?;
    let mut dist_encoder = HuffmanEncoder::new(1 << DIST_WIDTH)?;
    let mut len_encoder = HuffmanEncoder::new(1 << LEN_WIDTH)?;
    let mut finder = MatchFinder::new();

    let mut pos = 0usize;
    while pos < bytes.len() {
        let context = DecisionContext::new(&sym_encoder, &dist_encoder, &len_encoder, bytes.len());
        let op = choose_encode_op(&bytes[pos..], &finder, &context);
        write_encode_op(
            &mut writer,
            &mut sym_encoder,
            &mut dist_encoder,
            &mut len_encoder,
            total_dist_ranges,
            op,
        )?;
        let advance = encode_op_advance(op);
        finder.extend(&bytes[pos..pos + advance]);
        pos += advance;
    }

    Ok(writer.finish())
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

impl MatchFinder {
    fn new() -> Self {
        let mut history = vec![0u8; PRELOAD_SIZE];
        initialize_preload_buffer(&mut history);
        let mut finder = Self {
            history,
            heads: vec![None; 1 << 16],
            nodes: Vec::new(),
        };

        for index in 1..finder.history.len() {
            finder.push_existing_index(index);
        }

        finder
    }

    fn push_existing_index(&mut self, index: usize) {
        let key = self.hash_key(index - 1);
        let head = self.heads[key];
        self.nodes.push(HashNode {
            index: index - 1,
            next: head,
        });
        self.heads[key] = Some(self.nodes.len() - 1);
    }

    fn extend(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let next_index = self.history.len();
            self.history.push(byte);
            if next_index > 0 {
                self.push_existing_index(next_index);
            }
        }
    }

    fn hash_key(&self, index: usize) -> usize {
        ((usize::from(self.history[index]) << 8) | usize::from(self.history[index + 1])) & 0xFFFF
    }
}

fn choose_encode_op(bytes: &[u8], finder: &MatchFinder, context: &DecisionContext<'_>) -> EncodeOp {
    if let Some(copy) = choose_copy_candidate(bytes, finder, context) {
        return EncodeOp::Copy {
            dist: copy.dist,
            len: copy.len,
        };
    }

    if matches_dup(bytes, finder, 2) {
        return EncodeOp::Dup2;
    }
    if matches_dup(bytes, finder, 4) {
        return EncodeOp::Dup4;
    }
    if matches_dup(bytes, finder, 6) {
        return EncodeOp::Dup6;
    }

    EncodeOp::Literal(bytes[0])
}

fn matches_dup(bytes: &[u8], finder: &MatchFinder, step: usize) -> bool {
    bytes
        .first()
        .zip(
            finder
                .history
                .get(finder.history.len().saturating_sub(step)),
        )
        .is_some_and(|(&current, &previous)| current == previous)
}

fn choose_copy_candidate(
    bytes: &[u8],
    finder: &MatchFinder,
    context: &DecisionContext<'_>,
) -> Option<CopyCandidate> {
    let mut first = find_best_candidate(bytes, finder, context)?;
    let literal_cost = context.sym_encoder.write_symbol_cost(usize::from(bytes[0]));

    if bytes.len() > 1 {
        if let Some(next) =
            find_best_candidate_with_prefix(&bytes[1..], finder, &bytes[..1], context)
        {
            if next.gain >= first.gain
                && first.cost_per_byte
                    > (next.cost_per_byte * next.len + literal_cost) / (next.len + 1)
            {
                return None;
            }
        }
    }

    if first.len > 3 && bytes.len() > first.len {
        if let Some(after) = find_best_candidate_with_prefix(
            &bytes[first.len..],
            finder,
            &bytes[..first.len],
            context,
        ) {
            if let Some(shortened) = find_best_candidate_with_prefix(
                &bytes[first.len - 1..],
                finder,
                &bytes[..first.len - 1],
                context,
            ) {
                let current_cost =
                    first.cost_per_byte * first.len + after.cost_per_byte * after.len;
                let shortened_dist = first.dist + 1;
                let shortened_cost = copy_symbol_cost(
                    shortened_dist,
                    first.len - 1,
                    context.sym_encoder,
                    context.dist_encoder,
                    context.len_encoder,
                ) + shortened.cost_per_byte * shortened.len;
                if shortened.len > after.len && shortened_cost < current_cost {
                    first.len -= 1;
                    first.dist = shortened_dist;
                }
            }
        }
    }

    Some(first)
}

fn find_best_candidate(
    bytes: &[u8],
    finder: &MatchFinder,
    context: &DecisionContext<'_>,
) -> Option<CopyCandidate> {
    find_best_candidate_with_prefix(bytes, finder, &[], context)
}

fn find_best_candidate_with_prefix(
    bytes: &[u8],
    finder: &MatchFinder,
    prefix: &[u8],
    context: &DecisionContext<'_>,
) -> Option<CopyCandidate> {
    if bytes.len() < 2 {
        return None;
    }

    let base_len = finder.history.len();
    let current_index = base_len + prefix.len();
    let key = ((usize::from(bytes[0]) << 8) | usize::from(bytes[1])) & 0xFFFF;
    let mut node = finder.heads[key];
    let mut visited = 0usize;
    let mut best: Option<CopyCandidate> = None;
    let mut literal_cache = [0usize; MAX_LITERAL_COST_CACHE + 1];

    while let Some(node_index) = node {
        let candidate_index = finder.nodes[node_index].index;
        let raw_dist = current_index - candidate_index;
        visited += 1;

        if visited > MAX_HASH_CHAIN || raw_dist > context.max_dist {
            break;
        }

        consider_virtual_candidate(
            bytes,
            finder,
            prefix,
            context,
            &mut literal_cache,
            current_index,
            candidate_index,
            &mut best,
        );
        node = finder.nodes[node_index].next;
    }

    if !prefix.is_empty() {
        for candidate_index in base_len.saturating_sub(1)..current_index.saturating_sub(1) {
            if virtual_hash_key(finder, prefix, candidate_index) != key {
                continue;
            }
            consider_virtual_candidate(
                bytes,
                finder,
                prefix,
                context,
                &mut literal_cache,
                current_index,
                candidate_index,
                &mut best,
            );
        }
    }

    best
}

fn consider_virtual_candidate(
    bytes: &[u8],
    finder: &MatchFinder,
    prefix: &[u8],
    context: &DecisionContext<'_>,
    literal_cache: &mut [usize; MAX_LITERAL_COST_CACHE + 1],
    current_index: usize,
    candidate_index: usize,
    best: &mut Option<CopyCandidate>,
) {
    let raw_dist = current_index - candidate_index;
    let max_len = raw_dist.min(bytes.len());
    if max_len < LEN_MIN {
        return;
    }

    let mut matched = 2usize;
    while matched < max_len
        && virtual_history_byte(finder, prefix, candidate_index + matched) == bytes[matched]
    {
        matched += 1;
    }

    let dist = raw_dist - matched + 1;
    let min_len = min_copy_length(dist);
    if matched < min_len {
        return;
    }

    let literal_cost = literal_run_cost(bytes, 0, matched, context.sym_encoder, literal_cache);
    let copy_cost = copy_symbol_cost(
        dist,
        matched,
        context.sym_encoder,
        context.dist_encoder,
        context.len_encoder,
    );
    if literal_cost <= copy_cost {
        return;
    }

    let candidate = CopyCandidate {
        dist,
        len: matched,
        gain: literal_cost - copy_cost,
        cost_per_byte: copy_cost / matched,
    };
    if best.as_ref().is_none_or(|old| candidate.gain > old.gain) {
        *best = Some(candidate);
    }
}

fn virtual_history_byte(finder: &MatchFinder, prefix: &[u8], index: usize) -> u8 {
    if index < finder.history.len() {
        finder.history[index]
    } else {
        prefix[index - finder.history.len()]
    }
}

fn virtual_hash_key(finder: &MatchFinder, prefix: &[u8], index: usize) -> usize {
    ((usize::from(virtual_history_byte(finder, prefix, index)) << 8)
        | usize::from(virtual_history_byte(finder, prefix, index + 1)))
        & 0xFFFF
}

fn literal_run_cost(
    bytes: &[u8],
    start: usize,
    len: usize,
    sym_encoder: &HuffmanEncoder,
    cache: &mut [usize; MAX_LITERAL_COST_CACHE + 1],
) -> usize {
    let capped = len.min(MAX_LITERAL_COST_CACHE);
    for index in 0..capped {
        if cache[index + 1] == 0 {
            cache[index + 1] =
                cache[index] + sym_encoder.write_symbol_cost(usize::from(bytes[start + index]));
        }
    }

    if len > MAX_LITERAL_COST_CACHE {
        let base = cache[MAX_LITERAL_COST_CACHE];
        base + (base / MAX_LITERAL_COST_CACHE) * (len - MAX_LITERAL_COST_CACHE)
    } else {
        cache[len]
    }
}

fn copy_symbol_cost(
    dist: usize,
    len: usize,
    sym_encoder: &HuffmanEncoder,
    dist_encoder: &HuffmanEncoder,
    len_encoder: &HuffmanEncoder,
) -> usize {
    let dist_ranges = num_dist_ranges_for_distance(dist);
    let encoded_len = if dist >= MAX_2BYTE_DIST {
        len - (LEN_MIN3 - LEN_MIN)
    } else {
        len
    };
    let value = encoded_len - LEN_MIN;
    let group_count = value_group_count(value, BIT_RANGE);
    let top_shift = BIT_RANGE * (group_count - 1);
    let top_bits = (value >> top_shift) & ((1 << BIT_RANGE) - 1);
    let symbol =
        256 + (dist_ranges - 1) * (1 << LEN_WIDTH) + ((group_count > 1) as usize) * 4 + top_bits;

    let mut cost = sym_encoder.write_symbol_cost(symbol);
    if group_count > 1 {
        for group_index in (0..(group_count - 1)).rev() {
            let bits = (value >> (group_index * BIT_RANGE)) & ((1 << BIT_RANGE) - 1);
            let symbol = ((group_index != 0) as usize) << (LEN_WIDTH - 1) | bits;
            cost += len_encoder.write_symbol_cost(symbol);
        }
    }

    let dist_value = dist - DIST_MIN;
    for group_index in (0..dist_ranges).rev() {
        let bits = (dist_value >> (group_index * DIST_WIDTH)) & ((1 << DIST_WIDTH) - 1);
        cost += dist_encoder.write_symbol_cost(bits);
    }

    cost
}

fn encode_op_advance(op: EncodeOp) -> usize {
    match op {
        EncodeOp::Literal(_) | EncodeOp::Dup2 | EncodeOp::Dup4 | EncodeOp::Dup6 => 1,
        EncodeOp::Copy { len, .. } => len,
    }
}

fn write_encode_op(
    writer: &mut BitWriter,
    sym_encoder: &mut HuffmanEncoder,
    dist_encoder: &mut HuffmanEncoder,
    len_encoder: &mut HuffmanEncoder,
    num_dist_ranges: usize,
    op: EncodeOp,
) -> Result<(), LzDecompressError> {
    let dup2 = 256 + (1 << LEN_WIDTH) * num_dist_ranges;
    let dup4 = dup2 + 1;
    let dup6 = dup4 + 1;

    match op {
        EncodeOp::Literal(byte) => sym_encoder.write_symbol(writer, usize::from(byte))?,
        EncodeOp::Dup2 => sym_encoder.write_symbol(writer, dup2)?,
        EncodeOp::Dup4 => sym_encoder.write_symbol(writer, dup4)?,
        EncodeOp::Dup6 => sym_encoder.write_symbol(writer, dup6)?,
        EncodeOp::Copy { dist, len } => {
            write_copy_op(writer, sym_encoder, dist_encoder, len_encoder, dist, len)?
        }
    }

    Ok(())
}

fn write_copy_op(
    writer: &mut BitWriter,
    sym_encoder: &mut HuffmanEncoder,
    dist_encoder: &mut HuffmanEncoder,
    len_encoder: &mut HuffmanEncoder,
    dist: usize,
    len: usize,
) -> Result<(), LzDecompressError> {
    let dist_ranges = num_dist_ranges_for_distance(dist);
    let encoded_len = if dist >= MAX_2BYTE_DIST {
        len.checked_sub(LEN_MIN3 - LEN_MIN)
            .ok_or(LzDecompressError::MalformedStream)?
    } else {
        len
    };
    let value = encoded_len
        .checked_sub(LEN_MIN)
        .ok_or(LzDecompressError::MalformedStream)?;
    let group_count = value_group_count(value, BIT_RANGE);
    let top_shift = BIT_RANGE * (group_count - 1);
    let top_bits = (value >> top_shift) & ((1 << BIT_RANGE) - 1);
    let symbol =
        256 + (dist_ranges - 1) * (1 << LEN_WIDTH) + ((group_count > 1) as usize) * 4 + top_bits;

    sym_encoder.write_symbol(writer, symbol)?;
    write_length_bits(writer, len_encoder, value, group_count)?;
    write_distance_bits(writer, dist_encoder, dist, dist_ranges)?;
    Ok(())
}

fn write_length_bits(
    writer: &mut BitWriter,
    len_encoder: &mut HuffmanEncoder,
    value: usize,
    group_count: usize,
) -> Result<(), LzDecompressError> {
    if group_count <= 1 {
        return Ok(());
    }

    for group_index in (0..(group_count - 1)).rev() {
        let bits = (value >> (group_index * BIT_RANGE)) & ((1 << BIT_RANGE) - 1);
        let symbol = ((group_index != 0) as usize) << (LEN_WIDTH - 1) | bits;
        len_encoder.write_symbol(writer, symbol)?;
    }

    Ok(())
}

fn write_distance_bits(
    writer: &mut BitWriter,
    dist_encoder: &mut HuffmanEncoder,
    dist: usize,
    dist_ranges: usize,
) -> Result<(), LzDecompressError> {
    let value = dist
        .checked_sub(DIST_MIN)
        .ok_or(LzDecompressError::MalformedStream)?;

    for group_index in (0..dist_ranges).rev() {
        let bits = (value >> (group_index * DIST_WIDTH)) & ((1 << DIST_WIDTH) - 1);
        dist_encoder.write_symbol(writer, bits)?;
    }

    Ok(())
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

fn max_supported_distance(total_len: usize) -> usize {
    DIST_MIN + (1 << (DIST_WIDTH * num_dist_ranges(total_len))) - 1
}

fn min_copy_length(dist: usize) -> usize {
    if dist >= MAX_2BYTE_DIST {
        LEN_MIN3
    } else {
        LEN_MIN
    }
}

fn num_dist_ranges_for_distance(dist: usize) -> usize {
    let value = dist.saturating_sub(DIST_MIN);
    value_group_count(value, DIST_WIDTH)
}

fn value_group_count(value: usize, bits_per_group: usize) -> usize {
    bits_used(value).div_ceil(bits_per_group).max(1)
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
