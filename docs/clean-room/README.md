# Clean-room Apple build tools

`tools/apple_tools` implements, for Linux execution platforms, the Apple
tools rules_apple runs while bundling: `actool`, `ibtool` and `codesign`.
They were written without disassembling or decompiling Apple's tools or
frameworks. What was used:

- the tools' command lines and documentation (`man ibtool`, `actool
  --help`, the flags rules_apple passes);
- the files Xcode 27.0 (27A266a)'s tools write, for inputs constructed to
  isolate one behaviour at a time;
- Apple's public APIs, called from small programs (`NSMutableSet`,
  `CGContextDrawImage`, libcompression) to observe behaviour the outputs
  imply;
- published sources: the reference LZFSE encoder, CoreFoundation's
  CFString hash and CFBasicHash tables, the Apple File System Reference.

Each page records what was observed, what was inferred from it, and the
limits of what is reproduced:

| Page | What |
| --- | --- |
| [actool.md](actool.md) | Asset catalogs: CgBI icons, resampling, Assets.car |
| [ibtool.md](ibtool.md) | Storyboards: NIB archives, the compiled storyboard's Info.plist |
| [codesign.md](codesign.md) | Ad hoc signatures and resource seals |
| [lzfse.md](lzfse.md) | The streaming LZFSE encoder renditions are compressed with |
| [foundation-hashing.md](foundation-hashing.md) | Hash-table orders CoreFoundation and Foundation enumerate in |

## The result

`tools/compare_ipa.sh` builds `examples/ios/CleanRoom` (SwiftUI, a universal
app icon, a launch storyboard, ad hoc signed for arm64 devices) on the Mac
with Xcode's tools and in the Linux VM with these, and compares the two
`.ipa` files. They are identical, byte for byte.

## Scope

Inputs outside what is reproduced are refused with an error rather than
approximated: an `.ipa` that Xcode would build differently is a defect this
design does not hide. In particular:

- actool compiles one universal 1024 px iOS app icon per catalog; image,
  colour and data sets, other platforms and Icon Composer documents are not
  supported yet;
- ibtool compiles launch-screen storyboards (a view controller with a root
  view, a frame, an autoresizing mask, a safe-area guide and a system
  background colour);
- codesign signs ad hoc only. Identity signatures carry a CMS signing time,
  so even two Xcode runs differ; they also need a private key this tool has
  no keychain to find.
