/* One raw-deflate stream over several parts, a full flush after each part
 * but the last, which finishes the stream. Settings are those of the CgBI
 * PNGs Xcode's actool writes: level 9, 32 KiB window, memLevel 9,
 * Z_FILTERED. */
#include <stdlib.h>
#include <string.h>
#include <zlib.h>

/* out_sizes[i] receives part i's compressed size; returns the total, or -1. */
long deflate_parts(const unsigned char *const *parts, const size_t *sizes, int n,
                   unsigned char *out, size_t out_cap, size_t *out_sizes) {
  z_stream s;
  memset(&s, 0, sizeof s);
  if (deflateInit2(&s, 9, Z_DEFLATED, -15, 9, Z_FILTERED) != Z_OK) return -1;
  size_t total = 0;
  for (int i = 0; i < n; i++) {
    int flush = i + 1 == n ? Z_FINISH : Z_FULL_FLUSH;
    s.next_in = (unsigned char *)parts[i];
    s.avail_in = (uInt)sizes[i];
    size_t before = total;
    for (;;) {
      s.next_out = out + total;
      s.avail_out = (uInt)(out_cap - total);
      int rc = deflate(&s, flush);
      total = out_cap - s.avail_out;
      if (flush == Z_FINISH ? rc == Z_STREAM_END : (rc == Z_OK && s.avail_in == 0 && s.avail_out != 0)) break;
      if (rc != Z_OK && rc != Z_BUF_ERROR) { deflateEnd(&s); return -1; }
      if (s.avail_out == 0) { deflateEnd(&s); return -1; }
    }
    out_sizes[i] = total - before;
  }
  deflateEnd(&s);
  return (long)total;
}
