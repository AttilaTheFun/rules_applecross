# actool

Reproduced: `actool --compile DIR --platform iphoneos
--minimum-deployment-target V --app-icon NAME --target-device
iphone|ipad --output-partial-info-plist P CATALOG` for a catalog holding one
`.appiconset` with a single universal 1024x1024 iOS image.

Outputs, all byte-identical to Xcode 27.0's for `examples/ios/CleanRoom`:
`NAME60x60@2x.png` (iPhone), `NAME76x76@2x~ipad.png` (iPad), `Assets.car`
and the partial Info.plist (`CFBundleIcons`, `CFBundleIcons~ipad`). Xcode's
output is deterministic: repeated runs are identical, and the timestamp
`assetutil --info` prints is the file's modification time, not content.

## Resampling

Observed: the 120 and 152 px icons are downscaled from the 1024 px image.
Step-edge and impulse images (black, grey and coloured, every amplitude
1-255) measured the filter: Lanczos-2, sampling output pixel *i* at
`c = i·(S−1)/(D−1)` with the kernel stretched by the same factor, separable,
horizontal pass first, through an 8-bit intermediate, with 14-bit
fixed-point weights.

Established through a public API: drawing the source with
`CGContextDrawImage` at `kCGInterpolationHigh` reproduces actool's pixels
exactly, while vImage's, ImageIO's thumbnails and Core Image's Lanczos do
not. Drawing impulses into a 32-bit float context then gave the weights to
nine digits: the kernel's value at every tap except the one nearest `c`,
which weighs 1; normalized over the taps (taps beyond an edge count for the
edge pixel); the rounding residual of the 14-bit weights on that nearest tap.
This reproduces both icons with no differing sample.

## CgBI PNGs

Observed chunk sequence: `CgBI` (0x50002006), `IHDR` (8-bit RGBA),
`gAMA` 45455, `sRGB` 0, `cHRM` (sRGB primaries), `eXIf` (ColorSpace 1 and
the pixel dimensions), `iDOT`, two `IDAT`, `IEND`. Pixels are premultiplied
BGRA, every row filter 0. The two IDATs are one raw deflate stream with a
full flush between halves; searching zlib's parameters against five icons
leaves one setting: level 9, window 15, memLevel 9, `Z_FILTERED`. `iDOT`
records the halves' row counts and their IDAT offsets.

## Assets.car

A BOM store (big-endian): 512-byte header area, blocks 16-byte aligned in
allocation order, the variables table, then the index (at least 256 entries,
else the block count plus one rounded up to 8) and an empty free list.
Variables, in order: `CARHEADER`, `RENDITIONS`, `FACETKEYS`,
`APPEARANCEKEYS`, `KEYFORMAT`, `EXTENDED_METADATA`, `BITMAPKEYS`.

Trees are a 29-byte header (`tree`, version 1, page block, block size, entry
count, an inline-keys flag, the longest key) and one leaf page: entries
(value block, key block), four zero bytes, then a copy of every key; the
page's length is the block size plus the keys' lengths. `BITMAPKEYS` stores
its keys (name identifiers) inline in a 1024-byte page.

For an app icon: per idiom, an Icon Image rendition (part 220, dimension2
1: the 1024 px image as LZFSE-compressed premultiplied BGRA, named after
the source file) and a MultiSized Image rendition (part 218: a `SISM` list
of the sizes). Renditions are keyed by
(appearance, localization, scale, idiom, subtype, dimension2, identifier,
element 85, part); the tree lists them by key, and their blocks are
allocated, for one icon set, pad-multi, phone-icon, phone-multi, pad-icon
(observed; the rule behind other catalogs' orders is not established).
Image payloads are an `MLEC` header and `KCBC` chunks of up to a third of
4 MiB of rows, each an LZFSE stream (lzfse.md).

The name identifier: observed for 2,300 names, a 64-bit djb2 of the UTF-8
name with seed 100823, folded by evaluating its four 16-bit limbs, low first,
as a base-33 polynomial, modulo 2^16. Names that collide are renumbered (the
one later in byte order); that renumbering is not reproduced and is refused.

Versions recorded in the car are Xcode 27.0's (CoreUI 1010, storage 17,
CoreThemeDefinition 664); the Xcode name and build come from the SDK's
`version.plist`.

## Icon Composer documents

rules_apple passes an `.icon` document as an input beside any catalogs
(`actool ... --app-icon Visor CATALOG... Visor.icon`). For Visor's document
(a solid fill, one group of three SVG layers with a neutral shadow and
translucency), Xcode 27.0 writes the same bundle PNGs and partial Info.plist
as for an icon set, and an Assets.car holding, besides Icon Image and
MultiSized Image renditions for phone and pad, the document's colours, a
named gradient, its SVGs as Vector renditions, IconGroup and IconImageStack
renditions per appearance (light, dark, tintable), and flattened 1024 px
Icon Images for the Any, Dark and Tintable appearances. Two runs of Xcode's
actool on the same document differ: the Icon Image renditions are named
with a fresh UUID each time
(`Visor1024x1024_UIAppearanceAny_<UUID>-<pid>-<time>.png`).

Not reproducible, then, and this actool approximates: it renders the
document's default appearance flat (the fill, then each group's layers back
to front with a blurred, downward-offset shadow of the group's opacity, no
glass, specular or translucency; SVG layers through resvg, PNG layers
scaled about the centre) and compiles that image exactly as an icon set's
1024 px image, named `NAME1024x1024.png`. The result has the renditions an
icon set gives, which iOS before 26 draws icons from; how iOS 26 draws it
(presumably as an icon set's, without glass) has not been checked. Xcode's flattened image is square and opaque, as this
one is.
