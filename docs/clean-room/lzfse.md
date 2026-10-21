# LZFSE as CoreUI compresses renditions

Observed: a rendition chunk decoded and re-encoded with libcompression's
streaming encoder (`compression_stream`, the whole input in one call)
reproduces actool's bytes; the buffer encoder and the reference encoder
(which agree with each other) do not.

Differences, found by tracing the decoded (length, match, distance)
symbols of both encodings and then by randomized comparison with the
streaming encoder:

- Windows. The streaming encoder holds at most 3 x 256 KiB. In a window that
  does not end the input it starts matches only up to 400 bytes before the
  data it holds, though matches extend into those bytes. It then keeps 256
  KiB of history behind the position encoding stopped at, and refills: the
  next window ends 512 KiB after that position.
- Blocks. When a match's literal run is split into pieces, a piece that no
  longer fits the block flushes it and continues in the next; the reference
  reverts the whole match first.

`crates/lzfse/vendor` is the reference implementation with those two
changes, marked. Every rendition chunk of the catalogs examined re-encodes
byte for byte; randomized structured inputs up to 1.5 MB still differ in
about 4% of cases, so other differences remain.
