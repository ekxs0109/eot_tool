#include "lzcomp.h"

#include <stdlib.h>
#include <string.h>

#define PRELOAD_SIZE 7168
#define MAX_2BYTE_DIST 512
#define DIST_MIN 1
#define DIST_WIDTH 3
#define LEN_MIN 2
#define LEN_MIN3 3
#define LEN_WIDTH 3
#define BIT_RANGE (LEN_WIDTH - 1)
#define ROOT 1

typedef struct {
  const uint8_t *data;
  size_t length;
  size_t byte_pos;
  uint8_t bit_pos;
} bit_reader_t;

typedef struct {
  int16_t up;
  int16_t left;
  int16_t right;
  int16_t code;
  int weight;
} tree_node_t;

typedef struct {
  tree_node_t *tree;
  int16_t *symbol_index;
  int range;
  int bit_count2;
} huffman_decoder_t;

typedef struct {
  uint8_t *data;
  size_t size;
  size_t capacity;
  uint8_t byte_buf;
  uint8_t bit_count;
} bit_writer_t;

typedef struct {
  tree_node_t *tree;
  int16_t *symbol_index;
  int range;
  int bit_count2;
  bit_writer_t *bits;
} huffman_encoder_t;

typedef struct hash_node_t {
  int index;
  struct hash_node_t *next;
} hash_node_t;

typedef struct {
  bit_writer_t bits;
  int using_run_length;
  int length1;
  int max_copy_dist;
  huffman_encoder_t dist_encoder;
  huffman_encoder_t len_encoder;
  huffman_encoder_t sym_encoder;
  int num_dist_ranges;
  int dist_max;
  int dup2;
  int dup4;
  int dup6;
  int num_syms;
  uint8_t *buf;
  hash_node_t **hash_table;
  hash_node_t *hash_nodes;
  size_t hash_nodes_used;
} lzcomp_encoder_t;

static void bit_writer_init(bit_writer_t *writer) {
  writer->data = NULL;
  writer->size = 0;
  writer->capacity = 0;
  writer->byte_buf = 0;
  writer->bit_count = 0;
}

static void bit_writer_destroy(bit_writer_t *writer) {
  free(writer->data);
  writer->data = NULL;
  writer->size = 0;
  writer->capacity = 0;
  writer->byte_buf = 0;
  writer->bit_count = 0;
}

static eot_status_t bit_writer_reserve(bit_writer_t *writer, size_t capacity) {
  if (capacity <= writer->capacity) {
    return EOT_OK;
  }

  uint8_t *new_data = (uint8_t *)realloc(writer->data, capacity);
  if (new_data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  writer->data = new_data;
  writer->capacity = capacity;
  return EOT_OK;
}

static eot_status_t bit_writer_append_byte(bit_writer_t *writer, uint8_t value) {
  if (writer->size == writer->capacity) {
    size_t new_capacity = writer->capacity == 0 ? 256u : writer->capacity * 2u;
    eot_status_t status = bit_writer_reserve(writer, new_capacity);
    if (status != EOT_OK) {
      return status;
    }
  }

  writer->data[writer->size++] = value;
  return EOT_OK;
}

static eot_status_t bit_writer_write_bit(bit_writer_t *writer, int bit) {
  if (bit != 0) {
    writer->byte_buf |= (uint8_t)(1u << (7u - writer->bit_count));
  }
  writer->bit_count++;
  if (writer->bit_count == 8u) {
    eot_status_t status = bit_writer_append_byte(writer, writer->byte_buf);
    if (status != EOT_OK) {
      return status;
    }
    writer->byte_buf = 0;
    writer->bit_count = 0;
  }
  return EOT_OK;
}

static eot_status_t bit_writer_write_value(bit_writer_t *writer, int value,
                                           int num_bits) {
  for (int i = num_bits - 1; i >= 0; i--) {
    eot_status_t status = bit_writer_write_bit(writer, (value >> i) & 1);
    if (status != EOT_OK) {
      return status;
    }
  }
  return EOT_OK;
}

static eot_status_t bit_writer_flush(bit_writer_t *writer) {
  if (writer->bit_count == 0u) {
    return EOT_OK;
  }

  eot_status_t status = bit_writer_append_byte(writer, writer->byte_buf);
  if (status != EOT_OK) {
    return status;
  }

  writer->byte_buf = 0;
  writer->bit_count = 0;
  return EOT_OK;
}

static void bit_reader_init(bit_reader_t *reader, const uint8_t *data, size_t length) {
  reader->data = data;
  reader->length = length;
  reader->byte_pos = 0;
  reader->bit_pos = 0;
}

static int bit_reader_read_bit(bit_reader_t *reader) {
  if (reader->byte_pos >= reader->length) {
    return -1;
  }

  int bit = (reader->data[reader->byte_pos] >> (7 - reader->bit_pos)) & 1;
  reader->bit_pos++;
  if (reader->bit_pos >= 8) {
    reader->bit_pos = 0;
    reader->byte_pos++;
  }

  return bit;
}

static uint32_t bit_reader_read_value(bit_reader_t *reader, int num_bits) {
  uint32_t result = 0;
  for (int i = num_bits - 1; i >= 0; i--) {
    int bit = bit_reader_read_bit(reader);
    if (bit < 0) {
      return 0;
    }
    result |= ((uint32_t)bit << i);
  }
  return result;
}

static int bits_used(int x) {
  int i;
  for (i = 32; i > 1; i--) {
    if ((x & (1 << (i - 1))) != 0) {
      break;
    }
  }
  return i;
}

static int init_weight(huffman_decoder_t *decoder, int a) {
  if (decoder->tree[a].code < 0) {
    decoder->tree[a].weight = init_weight(decoder, decoder->tree[a].left) +
                               init_weight(decoder, decoder->tree[a].right);
  }
  return decoder->tree[a].weight;
}

static void swap_nodes(huffman_decoder_t *decoder, int a, int b) {
  int16_t upa = decoder->tree[a].up;
  int16_t upb = decoder->tree[b].up;
  tree_node_t tmp = decoder->tree[a];
  decoder->tree[a] = decoder->tree[b];
  decoder->tree[b] = tmp;
  decoder->tree[a].up = upa;
  decoder->tree[b].up = upb;

  int code = decoder->tree[a].code;
  if (code < 0) {
    decoder->tree[decoder->tree[a].left].up = (int16_t)a;
    decoder->tree[decoder->tree[a].right].up = (int16_t)a;
  } else {
    decoder->symbol_index[code] = (int16_t)a;
  }

  code = decoder->tree[b].code;
  if (code < 0) {
    decoder->tree[decoder->tree[b].left].up = (int16_t)b;
    decoder->tree[decoder->tree[b].right].up = (int16_t)b;
  } else {
    decoder->symbol_index[code] = (int16_t)b;
  }
}

static void update_weight(huffman_decoder_t *decoder, int a) {
  for (; a != ROOT; a = decoder->tree[a].up) {
    int weight_a = decoder->tree[a].weight;
    int b = a - 1;
    if (decoder->tree[b].weight == weight_a) {
      do {
        b--;
      } while (decoder->tree[b].weight == weight_a);
      b++;
      if (b > ROOT) {
        swap_nodes(decoder, a, b);
        a = b;
      }
    }
    weight_a++;
    decoder->tree[a].weight = weight_a;
  }
  decoder->tree[a].weight++;
}

static eot_status_t huffman_decoder_init(huffman_decoder_t *decoder, int range) {
  decoder->range = range;
  bits_used(range - 1);
  if (range > 256 && range < 512) {
    decoder->bit_count2 = bits_used(range - 257);
  } else {
    decoder->bit_count2 = 0;
  }

  decoder->symbol_index = (int16_t *)malloc(range * sizeof(int16_t));
  if (decoder->symbol_index == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  int limit = 2 * range;
  decoder->tree = (tree_node_t *)malloc(limit * sizeof(tree_node_t));
  if (decoder->tree == NULL) {
    free(decoder->symbol_index);
    decoder->symbol_index = NULL;
    return EOT_ERR_ALLOCATION;
  }

  for (int i = 0; i < limit; i++) {
    decoder->tree[i].up = 0;
    decoder->tree[i].left = 0;
    decoder->tree[i].right = 0;
    decoder->tree[i].code = 0;
    decoder->tree[i].weight = 0;
  }

  for (int i = 2; i < limit; i++) {
    decoder->tree[i].up = (int16_t)(i / 2);
    decoder->tree[i].weight = 1;
  }

  for (int i = 1; i < range; i++) {
    decoder->tree[i].left = (int16_t)(2 * i);
    decoder->tree[i].right = (int16_t)(2 * i + 1);
  }

  for (int i = 0; i < range; i++) {
    decoder->tree[i].code = -1;
    decoder->tree[range + i].code = (int16_t)i;
    decoder->tree[range + i].left = -1;
    decoder->tree[range + i].right = -1;
    decoder->symbol_index[i] = (int16_t)(range + i);
  }

  init_weight(decoder, ROOT);

  if (decoder->bit_count2 != 0) {
    update_weight(decoder, decoder->symbol_index[256]);
    update_weight(decoder, decoder->symbol_index[257]);
    for (int i = 0; i < 12; i++) {
      update_weight(decoder, decoder->symbol_index[range - 3]);
    }
    for (int i = 0; i < 6; i++) {
      update_weight(decoder, decoder->symbol_index[range - 2]);
    }
  } else {
    for (int j = 0; j < 2; j++) {
      for (int i = 0; i < range; i++) {
        update_weight(decoder, decoder->symbol_index[i]);
      }
    }
  }

  return EOT_OK;
}

static void huffman_decoder_destroy(huffman_decoder_t *decoder) {
  free(decoder->tree);
  free(decoder->symbol_index);
}

static int huffman_decoder_read_symbol(huffman_decoder_t *decoder, bit_reader_t *reader) {
  int a = ROOT;
  while (decoder->tree[a].code < 0) {
    int bit = bit_reader_read_bit(reader);
    if (bit < 0) {
      return -1;
    }
    a = bit ? decoder->tree[a].right : decoder->tree[a].left;
  }
  int symbol = decoder->tree[a].code;
  update_weight(decoder, a);
  return symbol;
}

static int init_encoder_weight(huffman_encoder_t *encoder, int a) {
  if (encoder->tree[a].code < 0) {
    encoder->tree[a].weight = init_encoder_weight(encoder, encoder->tree[a].left) +
                              init_encoder_weight(encoder, encoder->tree[a].right);
  }
  return encoder->tree[a].weight;
}

static void encoder_swap_nodes(huffman_encoder_t *encoder, int a, int b) {
  int16_t upa = encoder->tree[a].up;
  int16_t upb = encoder->tree[b].up;
  tree_node_t tmp = encoder->tree[a];
  encoder->tree[a] = encoder->tree[b];
  encoder->tree[b] = tmp;
  encoder->tree[a].up = upa;
  encoder->tree[b].up = upb;

  int code = encoder->tree[a].code;
  if (code < 0) {
    encoder->tree[encoder->tree[a].left].up = (int16_t)a;
    encoder->tree[encoder->tree[a].right].up = (int16_t)a;
  } else {
    encoder->symbol_index[code] = (int16_t)a;
  }

  code = encoder->tree[b].code;
  if (code < 0) {
    encoder->tree[encoder->tree[b].left].up = (int16_t)b;
    encoder->tree[encoder->tree[b].right].up = (int16_t)b;
  } else {
    encoder->symbol_index[code] = (int16_t)b;
  }
}

static void encoder_update_weight(huffman_encoder_t *encoder, int a) {
  for (; a != ROOT; a = encoder->tree[a].up) {
    int weight_a = encoder->tree[a].weight;
    int b = a - 1;
    if (encoder->tree[b].weight == weight_a) {
      do {
        b--;
      } while (encoder->tree[b].weight == weight_a);
      b++;
      if (b > ROOT) {
        encoder_swap_nodes(encoder, a, b);
        a = b;
      }
    }
    weight_a++;
    encoder->tree[a].weight = weight_a;
  }
  encoder->tree[a].weight++;
}

static eot_status_t huffman_encoder_init(huffman_encoder_t *encoder,
                                         bit_writer_t *bits, int range) {
  encoder->range = range;
  encoder->bits = bits;
  bits_used(range - 1);
  if (range > 256 && range < 512) {
    encoder->bit_count2 = bits_used(range - 257);
  } else {
    encoder->bit_count2 = 0;
  }

  encoder->symbol_index = (int16_t *)malloc((size_t)range * sizeof(int16_t));
  if (encoder->symbol_index == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  int limit = 2 * range;
  encoder->tree = (tree_node_t *)malloc((size_t)limit * sizeof(tree_node_t));
  if (encoder->tree == NULL) {
    free(encoder->symbol_index);
    encoder->symbol_index = NULL;
    return EOT_ERR_ALLOCATION;
  }

  for (int i = 0; i < limit; i++) {
    encoder->tree[i].up = 0;
    encoder->tree[i].left = 0;
    encoder->tree[i].right = 0;
    encoder->tree[i].code = 0;
    encoder->tree[i].weight = 0;
  }

  for (int i = 2; i < limit; i++) {
    encoder->tree[i].up = (int16_t)(i / 2);
    encoder->tree[i].weight = 1;
  }

  for (int i = 1; i < range; i++) {
    encoder->tree[i].left = (int16_t)(2 * i);
    encoder->tree[i].right = (int16_t)(2 * i + 1);
  }

  for (int i = 0; i < range; i++) {
    encoder->tree[i].code = -1;
    encoder->tree[range + i].code = (int16_t)i;
    encoder->tree[range + i].left = -1;
    encoder->tree[range + i].right = -1;
    encoder->symbol_index[i] = (int16_t)(range + i);
  }

  init_encoder_weight(encoder, ROOT);

  if (encoder->bit_count2 != 0) {
    encoder_update_weight(encoder, encoder->symbol_index[256]);
    encoder_update_weight(encoder, encoder->symbol_index[257]);
    for (int i = 0; i < 12; i++) {
      encoder_update_weight(encoder, encoder->symbol_index[range - 3]);
    }
    for (int i = 0; i < 6; i++) {
      encoder_update_weight(encoder, encoder->symbol_index[range - 2]);
    }
  } else {
    for (int j = 0; j < 2; j++) {
      for (int i = 0; i < range; i++) {
        encoder_update_weight(encoder, encoder->symbol_index[i]);
      }
    }
  }

  return EOT_OK;
}

static void huffman_encoder_destroy(huffman_encoder_t *encoder) {
  free(encoder->tree);
  free(encoder->symbol_index);
  encoder->tree = NULL;
  encoder->symbol_index = NULL;
}

static int huffman_encoder_write_symbol_cost(const huffman_encoder_t *encoder,
                                             int symbol) {
  int a = encoder->symbol_index[symbol];
  int depth = 0;
  do {
    depth++;
    a = encoder->tree[a].up;
  } while (a != ROOT);
  return depth << 16;
}

static eot_status_t huffman_encoder_write_symbol(huffman_encoder_t *encoder,
                                                 int symbol) {
  int a = encoder->symbol_index[symbol];
  int aa = a;
  int stack[64];
  int sp = 0;

  do {
    int up = encoder->tree[a].up;
    stack[sp++] = (encoder->tree[up].right == a);
    a = up;
  } while (a != ROOT);

  while (sp > 0) {
    eot_status_t status = bit_writer_write_bit(encoder->bits, stack[--sp]);
    if (status != EOT_OK) {
      return status;
    }
  }

  encoder_update_weight(encoder, aa);
  return EOT_OK;
}

static void initialize_preload_buffer(uint8_t *buf) {
  int i = 0;
  for (int k = 0; k < 32; k++) {
    for (int j = 0; j < 96; j++) {
      buf[i++] = (uint8_t)k;
      buf[i++] = (uint8_t)j;
    }
  }
  for (int j = 0; i < PRELOAD_SIZE && j < 256; j++) {
    buf[i++] = (uint8_t)j;
    buf[i++] = (uint8_t)j;
    buf[i++] = (uint8_t)j;
    buf[i++] = (uint8_t)j;
  }
}

static void lzcomp_encoder_destroy(lzcomp_encoder_t *encoder) {
  huffman_encoder_destroy(&encoder->dist_encoder);
  huffman_encoder_destroy(&encoder->len_encoder);
  huffman_encoder_destroy(&encoder->sym_encoder);
  free(encoder->buf);
  free(encoder->hash_table);
  free(encoder->hash_nodes);
  bit_writer_destroy(&encoder->bits);
}

static void lzcomp_encoder_update_model(lzcomp_encoder_t *encoder, int index) {
  if (index <= 0) {
    return;
  }

  hash_node_t *node = &encoder->hash_nodes[encoder->hash_nodes_used++];
  int pos = ((encoder->buf[index - 1] & 0xff) << 8) | (encoder->buf[index] & 0xff);
  node->index = index - 1;
  node->next = encoder->hash_table[pos];
  encoder->hash_table[pos] = node;
}

static void lzcomp_encoder_initialize_model(lzcomp_encoder_t *encoder) {
  int i = 0;
  for (int k = 0; k < 32; k++) {
    for (int j = 0; j < 96; j++) {
      encoder->buf[i] = (uint8_t)k;
      lzcomp_encoder_update_model(encoder, i++);
      encoder->buf[i] = (uint8_t)j;
      lzcomp_encoder_update_model(encoder, i++);
    }
  }
  for (int j = 0; i < PRELOAD_SIZE && j < 256; j++) {
    encoder->buf[i] = (uint8_t)j;
    lzcomp_encoder_update_model(encoder, i++);
    encoder->buf[i] = (uint8_t)j;
    lzcomp_encoder_update_model(encoder, i++);
    encoder->buf[i] = (uint8_t)j;
    lzcomp_encoder_update_model(encoder, i++);
    encoder->buf[i] = (uint8_t)j;
    lzcomp_encoder_update_model(encoder, i++);
  }
}

static void lzcomp_encoder_set_dist_range(lzcomp_encoder_t *encoder, int length) {
  encoder->num_dist_ranges = 1;
  encoder->dist_max = DIST_MIN + (1 << (DIST_WIDTH * encoder->num_dist_ranges)) - 1;
  while (encoder->dist_max < length) {
    encoder->num_dist_ranges++;
    encoder->dist_max = DIST_MIN + (1 << (DIST_WIDTH * encoder->num_dist_ranges)) - 1;
  }
  encoder->dup2 = 256 + (1 << LEN_WIDTH) * encoder->num_dist_ranges;
  encoder->dup4 = encoder->dup2 + 1;
  encoder->dup6 = encoder->dup4 + 1;
  encoder->num_syms = encoder->dup6 + 1;
}

static int lzcomp_get_num_dist_ranges(int dist) {
  int bits_needed = bits_used(dist - DIST_MIN);
  return (bits_needed + DIST_WIDTH - 1) / DIST_WIDTH;
}

static eot_status_t lzcomp_encode_distance(lzcomp_encoder_t *encoder, int value,
                                           int dist_ranges) {
  value -= DIST_MIN;
  const int mask = (1 << DIST_WIDTH) - 1;
  for (int i = (dist_ranges - 1) * DIST_WIDTH; i >= 0; i -= DIST_WIDTH) {
    eot_status_t status = huffman_encoder_write_symbol(
        &encoder->dist_encoder, (value >> i) & mask);
    if (status != EOT_OK) {
      return status;
    }
  }
  return EOT_OK;
}

static int lzcomp_encode_distance_cost(lzcomp_encoder_t *encoder, int value,
                                       int dist_ranges) {
  int cost = 0;
  value -= DIST_MIN;
  const int mask = (1 << DIST_WIDTH) - 1;
  for (int i = (dist_ranges - 1) * DIST_WIDTH; i >= 0; i -= DIST_WIDTH) {
    cost += huffman_encoder_write_symbol_cost(&encoder->dist_encoder,
                                              (value >> i) & mask);
  }
  return cost;
}

static eot_status_t lzcomp_encode_length(lzcomp_encoder_t *encoder, int value,
                                         int dist, int num_dist_ranges) {
  if (dist >= MAX_2BYTE_DIST) {
    value -= LEN_MIN3;
  } else {
    value -= LEN_MIN;
  }

  int bits_used_value = bits_used(value);
  int i = BIT_RANGE;
  while (i < bits_used_value) {
    i += BIT_RANGE;
  }

  int mask = 1 << (i - 1);
  int symbol = bits_used_value > BIT_RANGE ? 2 : 0;
  if ((value & mask) != 0) {
    symbol |= 1;
  }
  mask >>= 1;
  symbol <<= 1;
  if ((value & mask) != 0) {
    symbol |= 1;
  }
  mask >>= 1;

  eot_status_t status = huffman_encoder_write_symbol(
      &encoder->sym_encoder,
      256 + symbol + (num_dist_ranges - 1) * (1 << LEN_WIDTH));
  if (status != EOT_OK) {
    return status;
  }

  for (i = bits_used_value - BIT_RANGE; i >= 1; i -= BIT_RANGE) {
    symbol = i > BIT_RANGE ? 2 : 0;
    if ((value & mask) != 0) {
      symbol |= 1;
    }
    mask >>= 1;
    symbol <<= 1;
    if ((value & mask) != 0) {
      symbol |= 1;
    }
    mask >>= 1;
    status = huffman_encoder_write_symbol(&encoder->len_encoder, symbol);
    if (status != EOT_OK) {
      return status;
    }
  }

  return EOT_OK;
}

static int lzcomp_encode_length_cost(lzcomp_encoder_t *encoder, int value, int dist,
                                     int num_dist_ranges) {
  if (dist >= MAX_2BYTE_DIST) {
    value -= LEN_MIN3;
  } else {
    value -= LEN_MIN;
  }

  int bits_used_value = bits_used(value);
  int i = BIT_RANGE;
  while (i < bits_used_value) {
    i += BIT_RANGE;
  }

  int mask = 1 << (i - 1);
  int symbol = bits_used_value > BIT_RANGE ? 2 : 0;
  if ((value & mask) != 0) {
    symbol |= 1;
  }
  mask >>= 1;
  symbol <<= 1;
  if ((value & mask) != 0) {
    symbol |= 1;
  }
  mask >>= 1;

  int cost = huffman_encoder_write_symbol_cost(
      &encoder->sym_encoder,
      256 + symbol + (num_dist_ranges - 1) * (1 << LEN_WIDTH));

  for (i = bits_used_value - BIT_RANGE; i >= 1; i -= BIT_RANGE) {
    symbol = i > BIT_RANGE ? 2 : 0;
    if ((value & mask) != 0) {
      symbol |= 1;
    }
    mask >>= 1;
    symbol <<= 1;
    if ((value & mask) != 0) {
      symbol |= 1;
    }
    mask >>= 1;
    cost += huffman_encoder_write_symbol_cost(&encoder->len_encoder, symbol);
  }

  return cost;
}

static int lzcomp_find_match(lzcomp_encoder_t *encoder, int index, int *dist_out,
                             int *gain_out, int *cost_per_byte_out) {
  const int max_cost_cache_length = 32;
  int literal_cost_cache[max_cost_cache_length + 1];
  int max_index_minus_index = PRELOAD_SIZE + encoder->length1 - index;
  int best_length = 0;
  int best_dist = 0;
  int best_gain = 0;
  int best_copy_cost = 0;
  int max_computed_length = 0;

  literal_cost_cache[0] = 0;

  if (max_index_minus_index > 1) {
    int pos = ((encoder->buf[index] & 0xff) << 8) | (encoder->buf[index + 1] & 0xff);
    hash_node_t *prev_node = NULL;
    int hash_node_count = 0;

    for (hash_node_t *node = encoder->hash_table[pos]; node != NULL;
         prev_node = node, node = node->next) {
      int i = node->index;
      int dist = index - i;
      hash_node_count++;
      if (hash_node_count > 256 || dist > encoder->max_copy_dist ||
          dist > encoder->dist_max) {
        if (encoder->hash_table[pos] == node) {
          encoder->hash_table[pos] = NULL;
        } else if (prev_node != NULL) {
          prev_node->next = NULL;
        }
        break;
      }

      int max_len = index - i;
      if (max_index_minus_index < max_len) {
        max_len = max_index_minus_index;
      }
      if (max_len < LEN_MIN) {
        continue;
      }

      i += 2;
      int length;
      for (length = 2; length < max_len &&
           encoder->buf[i] == encoder->buf[index + length]; length++) {
        i++;
      }
      if (length < LEN_MIN) {
        continue;
      }

      dist = dist - length + 1;
      if (dist > encoder->dist_max || (length == 2 && dist >= MAX_2BYTE_DIST)) {
        continue;
      }
      if (length <= best_length && dist > best_dist) {
        if (length <= best_length - 2) {
          continue;
        }
        if (dist > (best_dist << DIST_WIDTH)) {
          if (length < best_length || dist > (best_dist << (DIST_WIDTH + 1))) {
            continue;
          }
        }
      }

      int literal_cost = 0;
      if (length > max_computed_length) {
        int limit = length;
        if (limit > max_cost_cache_length) {
          limit = max_cost_cache_length;
        }
        for (i = max_computed_length; i < limit; i++) {
          uint8_t c = encoder->buf[index + i];
          literal_cost_cache[i + 1] = literal_cost_cache[i] +
                                      huffman_encoder_write_symbol_cost(
                                          &encoder->sym_encoder, c & 0xff);
        }
        max_computed_length = limit;
        if (length > max_cost_cache_length) {
          literal_cost = literal_cost_cache[max_cost_cache_length];
          literal_cost += literal_cost / max_cost_cache_length *
                          (length - max_cost_cache_length);
        } else {
          literal_cost = literal_cost_cache[length];
        }
      } else {
        literal_cost = literal_cost_cache[length];
      }

      if (literal_cost > best_gain) {
        int dist_ranges = lzcomp_get_num_dist_ranges(dist);
        int copy_cost = lzcomp_encode_length_cost(encoder, length, dist,
                                                  dist_ranges);
        if (literal_cost - copy_cost - (dist_ranges << 16) > best_gain) {
          copy_cost += lzcomp_encode_distance_cost(encoder, dist, dist_ranges);
          int gain = literal_cost - copy_cost;
          if (gain > best_gain) {
            best_gain = gain;
            best_length = length;
            best_dist = dist;
            best_copy_cost = copy_cost;
          }
        }
      }
    }
  }

  *cost_per_byte_out = best_length > 0 ? best_copy_cost / best_length : 0;
  *dist_out = best_dist;
  *gain_out = best_gain;
  return best_length;
}

static int lzcomp_make_copy_decision(lzcomp_encoder_t *encoder, int index,
                                     int *best_dist) {
  int dist1 = 0;
  int gain1 = 0;
  int cost_per_byte1 = 0;
  int here = index;
  int len1 = lzcomp_find_match(encoder, index, &dist1, &gain1, &cost_per_byte1);

  lzcomp_encoder_update_model(encoder, index++);

  if (gain1 > 0) {
    int dist2 = 0;
    int gain2 = 0;
    int cost_per_byte2 = 0;
    int len2 = lzcomp_find_match(encoder, index, &dist2, &gain2, &cost_per_byte2);
    int symbol_cost = huffman_encoder_write_symbol_cost(&encoder->sym_encoder,
                                                        encoder->buf[here] & 0xff);

    if (gain2 >= gain1 &&
        cost_per_byte1 > (cost_per_byte2 * len2 + symbol_cost) / (len2 + 1)) {
      len1 = 0;
    } else if (len1 > 3) {
      len2 = lzcomp_find_match(encoder, here + len1, &dist2, &gain2,
                               &cost_per_byte2);
      if (len2 >= 2) {
        int dist3 = 0;
        int gain3 = 0;
        int cost_per_byte3 = 0;
        int len3 = lzcomp_find_match(encoder, here + len1 - 1, &dist3, &gain3,
                                     &cost_per_byte3);
        if (len3 > len2 && cost_per_byte3 < cost_per_byte2) {
          int dist_ranges = lzcomp_get_num_dist_ranges(dist1 + 1);
          int len_bit_count = lzcomp_encode_length_cost(encoder, len1 - 1,
                                                        dist1 + 1, dist_ranges);
          int dist_bit_count = lzcomp_encode_distance_cost(encoder, dist1 + 1,
                                                           dist_ranges);
          int cost1b = len_bit_count + dist_bit_count + cost_per_byte3 * len3;
          int cost1a = cost_per_byte1 * len1 + cost_per_byte2 * len2;
          if ((cost1a / (len1 + len2)) > (cost1b / (len1 - 1 + len3))) {
            len1--;
            dist1++;
          }
        }
      }
    }

    if (len1 == 2) {
      if (here >= 2 && encoder->buf[here] == encoder->buf[here - 2]) {
        int dup2_cost = huffman_encoder_write_symbol_cost(&encoder->sym_encoder,
                                                          encoder->dup2);
        if (cost_per_byte1 * 2 >
            dup2_cost + huffman_encoder_write_symbol_cost(
                            &encoder->sym_encoder, encoder->buf[here + 1] & 0xff)) {
          len1 = 0;
        }
      } else if (here >= 1 &&
                 here + 1 < PRELOAD_SIZE + encoder->length1 &&
                 encoder->buf[here + 1] == encoder->buf[here - 1]) {
        int dup2_cost = huffman_encoder_write_symbol_cost(&encoder->sym_encoder,
                                                          encoder->dup2);
        if (cost_per_byte1 * 2 > symbol_cost + dup2_cost) {
          len1 = 0;
        }
      }
    }
  }

  *best_dist = dist1;
  return len1;
}

static eot_status_t lzcomp_encoder_write(lzcomp_encoder_t *encoder,
                                         const uint8_t *data_in, size_t length) {
  memset(encoder, 0, sizeof(*encoder));
  bit_writer_init(&encoder->bits);
  encoder->using_run_length = 0;
  encoder->max_copy_dist = 0x7fffffff;
  encoder->length1 = (int)length;

  lzcomp_encoder_set_dist_range(encoder, encoder->length1);

  eot_status_t status = huffman_encoder_init(&encoder->dist_encoder, &encoder->bits,
                                             1 << DIST_WIDTH);
  if (status != EOT_OK) {
    lzcomp_encoder_destroy(encoder);
    return status;
  }
  status = huffman_encoder_init(&encoder->len_encoder, &encoder->bits,
                                1 << LEN_WIDTH);
  if (status != EOT_OK) {
    lzcomp_encoder_destroy(encoder);
    return status;
  }
  status = huffman_encoder_init(&encoder->sym_encoder, &encoder->bits,
                                encoder->num_syms);
  if (status != EOT_OK) {
    lzcomp_encoder_destroy(encoder);
    return status;
  }

  encoder->buf = (uint8_t *)malloc(PRELOAD_SIZE + length);
  encoder->hash_table = (hash_node_t **)calloc(0x10000u, sizeof(hash_node_t *));
  encoder->hash_nodes = (hash_node_t *)calloc(PRELOAD_SIZE + length,
                                              sizeof(hash_node_t));
  if (encoder->buf == NULL || encoder->hash_table == NULL ||
      encoder->hash_nodes == NULL) {
    lzcomp_encoder_destroy(encoder);
    return EOT_ERR_ALLOCATION;
  }

  memcpy(encoder->buf + PRELOAD_SIZE, data_in, length);
  lzcomp_encoder_initialize_model(encoder);

  status = bit_writer_write_bit(&encoder->bits, encoder->using_run_length);
  if (status != EOT_OK) {
    lzcomp_encoder_destroy(encoder);
    return status;
  }
  status = bit_writer_write_value(&encoder->bits, encoder->length1, 24);
  if (status != EOT_OK) {
    lzcomp_encoder_destroy(encoder);
    return status;
  }

  int limit = PRELOAD_SIZE + encoder->length1;
  for (int i = PRELOAD_SIZE; i < limit;) {
    int here = i;
    int dist = 0;
    int len = lzcomp_make_copy_decision(encoder, i++, &dist);
    if (len > 0) {
      int dist_ranges = lzcomp_get_num_dist_ranges(dist);
      status = lzcomp_encode_length(encoder, len, dist, dist_ranges);
      if (status != EOT_OK) {
        lzcomp_encoder_destroy(encoder);
        return status;
      }
      status = lzcomp_encode_distance(encoder, dist, dist_ranges);
      if (status != EOT_OK) {
        lzcomp_encoder_destroy(encoder);
        return status;
      }
      for (int j = 1; j < len; j++) {
        lzcomp_encoder_update_model(encoder, i++);
      }
    } else {
      uint8_t c = encoder->buf[here];
      if (here >= 2 && c == encoder->buf[here - 2]) {
        status = huffman_encoder_write_symbol(&encoder->sym_encoder,
                                              encoder->dup2);
      } else if (here >= 4 && c == encoder->buf[here - 4]) {
        status = huffman_encoder_write_symbol(&encoder->sym_encoder,
                                              encoder->dup4);
      } else if (here >= 6 && c == encoder->buf[here - 6]) {
        status = huffman_encoder_write_symbol(&encoder->sym_encoder,
                                              encoder->dup6);
      } else {
        status = huffman_encoder_write_symbol(&encoder->sym_encoder, c & 0xff);
      }
      if (status != EOT_OK) {
        lzcomp_encoder_destroy(encoder);
        return status;
      }
    }
  }

  status = bit_writer_flush(&encoder->bits);
  if (status != EOT_OK) {
    lzcomp_encoder_destroy(encoder);
    return status;
  }

  return EOT_OK;
}

static int decode_length(bit_reader_t *reader, huffman_decoder_t *len_decoder, int symbol,
                         int num_dist_ranges) {
  // Extract the length symbol from the main symbol
  int len_symbol = symbol - 256 - (num_dist_ranges - 1) * (1 << LEN_WIDTH);

  // len_symbol encodes: bit2=has_more, bit1 and bit0 are the top 2 bits of value
  int has_more = (len_symbol >> 2) & 1;
  int bit1 = (len_symbol >> 1) & 1;
  int bit0 = len_symbol & 1;

  int value = (bit1 << 1) | bit0;
  int bits_decoded = 2;

  // Determine how many total bits we need
  int total_bits = has_more ? (BIT_RANGE * 2) : BIT_RANGE;

  // Read additional bits if needed
  while (bits_decoded < total_bits) {
    int extra_symbol = huffman_decoder_read_symbol(len_decoder, reader);
    if (extra_symbol < 0) {
      return -1;
    }

    int extra_has_more = (extra_symbol >> 2) & 1;
    int extra_bit1 = (extra_symbol >> 1) & 1;
    int extra_bit0 = extra_symbol & 1;

    value = (value << 2) | (extra_bit1 << 1) | extra_bit0;
    bits_decoded += 2;

    if (extra_has_more) {
      total_bits += BIT_RANGE;
    }
  }

  // Add back the minimum length
  value += LEN_MIN;

  return value;
}

static int decode_distance(bit_reader_t *reader, huffman_decoder_t *dist_decoder,
                           int num_dist_ranges) {
  int dist = 0;
  for (int i = 0; i < num_dist_ranges; i++) {
    int dist_bits = huffman_decoder_read_symbol(dist_decoder, reader);
    if (dist_bits < 0) {
      return -1;
    }
    dist = (dist << DIST_WIDTH) | dist_bits;
  }
  return dist + DIST_MIN;
}

eot_status_t lzcomp_decompress(buffer_view_t compressed, uint8_t **out_data, size_t *out_size) {
  if (out_data == NULL || out_size == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (compressed.length < 4) {
    return EOT_ERR_TRUNCATED;
  }

  bit_reader_t reader;
  bit_reader_init(&reader, compressed.data, compressed.length);

  int using_run_length = bit_reader_read_bit(&reader);
  if (using_run_length < 0) {
    return EOT_ERR_DECOMPRESS_FAILED;
  }

  uint32_t decompressed_length = bit_reader_read_value(&reader, 24);
  if (decompressed_length > 100 * 1024 * 1024) {
    return EOT_ERR_DECOMPRESS_FAILED;
  }

  if (decompressed_length == 0) {
    *out_data = NULL;
    *out_size = 0;
    return EOT_OK;
  }

  uint8_t *output_buf = (uint8_t *)malloc(PRELOAD_SIZE + decompressed_length);
  if (output_buf == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  initialize_preload_buffer(output_buf);

  int num_dist_ranges = 1;
  int dist_max = DIST_MIN + (1 << (DIST_WIDTH * num_dist_ranges)) - 1;
  while (dist_max < (int)decompressed_length) {
    num_dist_ranges++;
    dist_max = DIST_MIN + (1 << (DIST_WIDTH * num_dist_ranges)) - 1;
  }

  int dup2 = 256 + (1 << LEN_WIDTH) * num_dist_ranges;
  int dup4 = dup2 + 1;
  int dup6 = dup4 + 1;
  int num_syms = dup6 + 1;

  huffman_decoder_t dist_decoder, len_decoder, sym_decoder;
  eot_status_t status;

  status = huffman_decoder_init(&dist_decoder, 1 << DIST_WIDTH);
  if (status != EOT_OK) {
    free(output_buf);
    return status;
  }

  status = huffman_decoder_init(&len_decoder, 1 << LEN_WIDTH);
  if (status != EOT_OK) {
    huffman_decoder_destroy(&dist_decoder);
    free(output_buf);
    return status;
  }

  status = huffman_decoder_init(&sym_decoder, num_syms);
  if (status != EOT_OK) {
    huffman_decoder_destroy(&dist_decoder);
    huffman_decoder_destroy(&len_decoder);
    free(output_buf);
    return status;
  }

  size_t output_pos = PRELOAD_SIZE;
  size_t output_limit = PRELOAD_SIZE + decompressed_length;

  while (output_pos < output_limit) {
    int symbol = huffman_decoder_read_symbol(&sym_decoder, &reader);
    if (symbol < 0) {
      huffman_decoder_destroy(&dist_decoder);
      huffman_decoder_destroy(&len_decoder);
      huffman_decoder_destroy(&sym_decoder);
      free(output_buf);
      return EOT_ERR_DECOMPRESS_FAILED;
    }

    if (symbol == dup2 && output_pos >= 2) {
      output_buf[output_pos] = output_buf[output_pos - 2];
      output_pos++;
    } else if (symbol == dup4 && output_pos >= 4) {
      output_buf[output_pos] = output_buf[output_pos - 4];
      output_pos++;
    } else if (symbol == dup6 && output_pos >= 6) {
      output_buf[output_pos] = output_buf[output_pos - 6];
      output_pos++;
    } else if (symbol < 256) {
      output_buf[output_pos] = (uint8_t)symbol;
      output_pos++;
    } else if (symbol >= 256 && symbol < dup2) {
      int len_symbol = symbol - 256;
      int dist_range_idx = len_symbol / (1 << LEN_WIDTH);
      size_t copy_start;

      int length = decode_length(&reader, &len_decoder, symbol, dist_range_idx + 1);
      if (length < 0) {
        huffman_decoder_destroy(&dist_decoder);
        huffman_decoder_destroy(&len_decoder);
        huffman_decoder_destroy(&sym_decoder);
        free(output_buf);
        return EOT_ERR_DECOMPRESS_FAILED;
      }

      int dist = decode_distance(&reader, &dist_decoder, dist_range_idx + 1);
      if (dist < 0) {
        huffman_decoder_destroy(&dist_decoder);
        huffman_decoder_destroy(&len_decoder);
        huffman_decoder_destroy(&sym_decoder);
        free(output_buf);
        return EOT_ERR_DECOMPRESS_FAILED;
      }

      if (dist >= MAX_2BYTE_DIST) {
        length += (LEN_MIN3 - LEN_MIN);
      }

      if (length > (int)(output_limit - output_pos)) {
        huffman_decoder_destroy(&dist_decoder);
        huffman_decoder_destroy(&len_decoder);
        huffman_decoder_destroy(&sym_decoder);
        free(output_buf);
        return EOT_ERR_DECOMPRESS_FAILED;
      }

      if ((size_t)(dist + length - 1) > output_pos) {
        huffman_decoder_destroy(&dist_decoder);
        huffman_decoder_destroy(&len_decoder);
        huffman_decoder_destroy(&sym_decoder);
        free(output_buf);
        return EOT_ERR_DECOMPRESS_FAILED;
      }

      copy_start = output_pos - (size_t)dist - (size_t)length + 1;
      for (int i = 0; i < length; i++) {
        output_buf[output_pos] = output_buf[copy_start + (size_t)i];
        output_pos++;
      }
    }
  }

  huffman_decoder_destroy(&dist_decoder);
  huffman_decoder_destroy(&len_decoder);
  huffman_decoder_destroy(&sym_decoder);

  uint8_t *result = (uint8_t *)malloc(decompressed_length);
  if (result == NULL) {
    free(output_buf);
    return EOT_ERR_ALLOCATION;
  }

  memcpy(result, output_buf + PRELOAD_SIZE, decompressed_length);
  free(output_buf);

  *out_data = result;
  *out_size = decompressed_length;

  return EOT_OK;
}

eot_status_t lzcomp_compress(const uint8_t *data, size_t length, uint8_t **out_data, size_t *out_size) {
  if (data == NULL || out_data == NULL || out_size == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  lzcomp_encoder_t encoder;
  eot_status_t status = lzcomp_encoder_write(&encoder, data, length);
  if (status != EOT_OK) {
    return status;
  }

  *out_data = encoder.bits.data;
  *out_size = encoder.bits.size;
  encoder.bits.data = NULL;
  encoder.bits.size = 0;
  encoder.bits.capacity = 0;

  lzcomp_encoder_destroy(&encoder);
  return EOT_OK;
}
