/*
LZFSE encoding that matches the output of a streaming encoder fed the whole
input at once.

lzfse_encode_buffer() hands lzfse_encode_base() the entire input as one window
(below 4 GiB). A streaming encoder works in windows instead, so its long
matches end before each window boundary and the output differs. Observed
against such an encoder (see docs/clean-room/lzfse.md), the windows are:

  - the first ends at 3 * 256 KiB;
  - each next one ends 2 * 256 KiB - 400 bytes after the previous end;
  - before each next window the state is translated by 2 * 256 KiB, keeping
    256 KiB of history (the maximum match distance is 262139).

Everything else, including the small-input LZVN and raw paths, is
lzfse_encode_buffer_with_scratch() unchanged.

This file is not part of the reference distribution; the reference sources
beside it are unmodified (lzfse e634ca58b4821d9f3d560cdc6df5dec02ffc93fd).
*/

#include "lzfse.h"
#include "lzfse_internal.h"

#include <string.h>

#define STREAM_BLOCK ((lzfse_offset)262144)
#define STREAM_FIRST_WINDOW (3 * STREAM_BLOCK)
#define STREAM_ADVANCE (2 * STREAM_BLOCK)
#define STREAM_SHORTFALL ((lzfse_offset)400)

size_t lzfse_encode_stream_buffer(uint8_t *__restrict dst_buffer,
                                  size_t dst_size,
                                  const uint8_t *__restrict src_buffer,
                                  size_t src_size,
                                  void *__restrict scratch_buffer) {
  // The stream path differs from the buffer path only once LZFSE (rather
  // than LZVN or a raw copy) encodes more than one window.
  if (src_size < LZFSE_ENCODE_LZVN_THRESHOLD ||
      (lzfse_offset)src_size <= STREAM_FIRST_WINDOW ||
      src_size >= 0xffffffffU)
    return lzfse_encode_buffer_with_scratch(dst_buffer, dst_size, src_buffer,
                                            src_size, scratch_buffer);

  lzfse_encoder_state *state = scratch_buffer;
  memset(state, 0x00, sizeof *state);
  if (lzfse_encode_init(state) != LZFSE_STATUS_OK)
    goto try_uncompressed;
  state->dst = dst_buffer;
  state->dst_begin = dst_buffer;
  state->dst_end = &dst_buffer[dst_size];
  state->src = src_buffer;
  state->src_encode_i = 0;

  {
    const lzfse_offset total = (lzfse_offset)src_size;
    lzfse_offset window_start = 0; // absolute offset of state->src[0]
    lzfse_offset window_end = STREAM_FIRST_WINDOW;
    state->src_end = window_end;
    while (window_end < total) {
      if (lzfse_encode_base(state) != LZFSE_STATUS_OK)
        goto try_uncompressed;
      lzfse_encode_translate(state, STREAM_ADVANCE);
      window_start += STREAM_ADVANCE;
      window_end += STREAM_ADVANCE - STREAM_SHORTFALL;
      if (window_end > total)
        window_end = total;
      state->src_end = window_end - window_start;
    }
  }
  if (lzfse_encode_base(state) != LZFSE_STATUS_OK)
    goto try_uncompressed;
  if (lzfse_encode_finish(state) != LZFSE_STATUS_OK)
    goto try_uncompressed;
  return state->dst - dst_buffer;

try_uncompressed:
  if (src_size + 12 <= dst_size && src_size < INT32_MAX) {
    // Same raw block as the reference encoder writes when compression fails.
    uncompressed_block_header header = {.magic = LZFSE_UNCOMPRESSED_BLOCK_MAGIC,
                                        .n_raw_bytes = (uint32_t)src_size};
    uint8_t *dst_end = dst_buffer;
    memcpy(dst_end, &header, sizeof header);
    dst_end += sizeof header;
    memcpy(dst_end, src_buffer, src_size);
    dst_end += src_size;
    store4(dst_end, LZFSE_ENDOFSTREAM_BLOCK_MAGIC);
    dst_end += 4;
    return dst_end - dst_buffer;
  }
  return 0;
}
