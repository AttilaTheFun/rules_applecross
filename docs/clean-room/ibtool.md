# ibtool

Reproduced: `ibtool --compilation-directory DIR ... X.storyboard` and
`ibtool --link DIR ... X.storyboardc` for launch-screen storyboards. For
`examples/ios/CleanRoom`'s, both nibs and the Info.plist are byte-identical
to Xcode 27.0's (toolchain compiler mode, the default). Linking copies.

## The compiled storyboard

`X.storyboardc/` holds `Info.plist` (binary: the designated entry point,
version 1, view controller identifiers to nib names), one nib per view
controller (`UIViewController-<id>.nib`: the scene's placeholders and a
`UIClassSwapper` standing for the controller, with outlets to the storyboard
placeholder and from the owner) and one per view (`<vc>-view-<view>.nib`:
the view, its colour and safe-area guide, the owner's `view` outlet).

## NIB archives

`NIBArchive`, format 1, coder 10; four tables (objects, keys, values,
classes), varints whose last byte has the high bit set, and an `LNE\0`
trailer. Observed from Xcode's nibs:

- objects are numbered depth first in encoding order; shared values (one
  empty array, a class name string) are one object;
- each object's values are contiguous, in encoding order; integers take the
  smallest signed width; rects and points are data: 7 then doubles;
- classes are listed in first use, then fallback classes (UIColor's
  NSColor), which the class entry references.

The key table's order is a hash table's. Comparing Apple's key tables with
containers filled through public APIs in candidate orders, `NSMutableSet`
filled with keys in depth-first first-use order reproduced all ten nibs
examined (five storyboards); CoreFoundation's CFSet and CFDictionary did
not. foundation-hashing.md describes that set's layout.

## Supported elements

A scene whose view controller (no custom class) has a root view with a
frame, an autoresizing mask, a safe-area layout guide and a background
colour that is a system colour defined in the document as white/alpha in
the generic gamma 2.2 grey space. Other elements are refused. The values
UIKit encodes for these classes (UIView's opaque, autoresizes-subviews and
semantic content attribute, UIColor's component encoding) are as Xcode
writes them for these inputs.
