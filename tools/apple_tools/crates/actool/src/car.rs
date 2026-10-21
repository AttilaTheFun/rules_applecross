//! Assets.car: CoreUI's compiled asset catalog, as Xcode 27's actool writes
//! it for the catalogs described in docs/clean-room/actool.md.
//!
//! A car is a BOM with these variables, allocated in this order:
//! CARHEADER; the RENDITIONS, FACETKEYS and APPEARANCEKEYS trees; the
//! appearance and facet keys with their values; KEYFORMAT; each rendition's
//! key and CSI value; EXTENDED_METADATA; the BITMAPKEYS tree and its values.

use crate::bom::{Bom, Tree};

pub const ATTR_ELEMENT: u16 = 1;
pub const ATTR_PART: u16 = 2;
pub const ATTR_APPEARANCE: u16 = 7;
pub const ATTR_DIMENSION2: u16 = 9;
pub const ATTR_SCALE: u16 = 12;
pub const ATTR_LOCALIZATION: u16 = 13;
pub const ATTR_IDIOM: u16 = 15;
pub const ATTR_SUBTYPE: u16 = 16;
pub const ATTR_IDENTIFIER: u16 = 17;

pub const IDIOM_PHONE: u16 = 1;
pub const IDIOM_PAD: u16 = 2;

pub const ELEMENT_ICON: u16 = 85;
pub const PART_MULTISIZED_IMAGE: u16 = 218;
pub const PART_ICON_IMAGE: u16 = 220;

const LAYOUT_ICON_IMAGE: u16 = 12;
const LAYOUT_MULTISIZED_IMAGE: u16 = 1010;

/// The versions of the CoreUI that Xcode 27.0's actool writes with.
pub const COREUI_VERSION: u32 = 1010;
pub const STORAGE_VERSION: u32 = 17;
pub const MAIN_VERSION: &str = "@(#)PROGRAM:CoreUI  PROJECT:CoreUI-1010 [LAR]";
pub const AUTHORING_TOOL: &str = "@(#)PROGRAM:CoreThemeDefinition  PROJECT:CoreThemeDefinition-664 [LAR] [IIO-2851][LAR] AppleJPEG[1][32]  CMPhoto[1][219]] [IR-120(hw)]";

/// A rendition: its key attributes and its CSI value.
pub struct Rendition {
    pub attrs: Vec<(u16, u16)>,
    pub csi: Vec<u8>,
}

pub struct Facet {
    pub name: String,
    pub attrs: Vec<(u16, u16)>,
}

pub struct Catalog {
    /// Attributes of the key format, in order.
    pub key_format: Vec<u16>,
    pub facets: Vec<Facet>,
    pub appearances: Vec<(String, u16)>,
    /// Renditions in the order their blocks are allocated.
    pub renditions: Vec<Rendition>,
    /// "Xcode 27.0 (27A266a)".
    pub xcode: String,
    pub deployment_target: String,
    pub platform: String,
}

fn fixed(s: &str, n: usize) -> Vec<u8> {
    let mut b = s.as_bytes().to_vec();
    b.resize(n, 0);
    b
}

fn le32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn le16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

impl Catalog {
    fn key(&self, r: &Rendition) -> Vec<u8> {
        let mut k = Vec::with_capacity(self.key_format.len() * 2);
        for a in &self.key_format {
            let v = r.attrs.iter().find(|(x, _)| x == a).map_or(0, |p| p.1);
            le16(&mut k, v);
        }
        k
    }

    fn key_values(&self, r: &Rendition) -> Vec<u16> {
        self.key_format.iter().map(|a| r.attrs.iter().find(|(x, _)| x == a).map_or(0, |p| p.1)).collect()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bom = Bom::new();
        let header = bom.alloc(Vec::new());
        let renditions_tree = Tree::reserve(&mut bom, 4096, false);
        let facets_tree = Tree::reserve(&mut bom, 4096, false);
        let appearances_tree = Tree::reserve(&mut bom, 4096, false);

        let mut appearance_entries = Vec::new();
        for (name, value) in &self.appearances {
            let k = bom.alloc(name.as_bytes().to_vec());
            let v = bom.alloc(value.to_le_bytes().to_vec());
            appearance_entries.push((k, v, name.as_bytes().to_vec()));
        }
        let mut facet_entries = Vec::new();
        for f in &self.facets {
            let k = bom.alloc(f.name.as_bytes().to_vec());
            let mut v = Vec::new();
            le16(&mut v, 0); // hotspot x
            le16(&mut v, 0); // hotspot y
            le16(&mut v, f.attrs.len() as u16);
            for (a, x) in &f.attrs {
                le16(&mut v, *a);
                le16(&mut v, *x);
            }
            let vi = bom.alloc(v);
            facet_entries.push((k, vi, f.name.as_bytes().to_vec()));
        }

        let mut kf = b"tmfk".to_vec();
        le32(&mut kf, 0);
        le32(&mut kf, self.key_format.len() as u32);
        for a in &self.key_format {
            le32(&mut kf, *a as u32);
        }
        let key_format = bom.alloc(kf);

        let mut rendition_entries = Vec::new();
        for r in &self.renditions {
            let key = self.key(r);
            let k = bom.alloc(key.clone());
            let v = bom.alloc(r.csi.clone());
            rendition_entries.push((self.key_values(r), (k, v, key)));
        }
        rendition_entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut meta = b"META".to_vec();
        meta.extend(fixed("", 256));
        meta.extend(fixed(&self.deployment_target, 256));
        meta.extend(fixed(&self.platform, 256));
        meta.extend(fixed(AUTHORING_TOOL, 256));
        let extended = bom.alloc(meta);

        let bitmap_tree = Tree::reserve(&mut bom, 1024, true);
        let mut bitmap_entries = Vec::new();
        let mut identifiers: Vec<u16> = Vec::new();
        for r in &self.renditions {
            let id = r.attrs.iter().find(|a| a.0 == ATTR_IDENTIFIER).map_or(0, |a| a.1);
            if !identifiers.contains(&id) {
                identifiers.push(id);
            }
        }
        identifiers.sort();
        for id in identifiers {
            let mut v = Vec::new();
            le32(&mut v, 1);
            le32(&mut v, 0);
            le32(&mut v, 4 + 4 * self.key_format.len() as u32);
            le32(&mut v, self.key_format.len() as u32);
            for a in &self.key_format {
                let tracked = matches!(*a, ATTR_LOCALIZATION | ATTR_SCALE | ATTR_IDIOM | ATTR_SUBTYPE | ATTR_DIMENSION2);
                let mask = if tracked {
                    self.renditions
                        .iter()
                        .filter(|r| r.attrs.iter().any(|x| x.0 == ATTR_IDENTIFIER && x.1 == id))
                        .map(|r| 1u32 << r.attrs.iter().find(|x| x.0 == *a).map_or(0, |x| x.1))
                        .fold(0, |m, b| m | b)
                } else {
                    0xffff_ffff
                };
                le32(&mut v, mask);
            }
            let vi = bom.alloc(v);
            bitmap_entries.push((id as u32, vi, Vec::new()));
        }

        let mut h = b"RATC".to_vec();
        le32(&mut h, COREUI_VERSION);
        le32(&mut h, STORAGE_VERSION);
        le32(&mut h, 0); // storage timestamp
        le32(&mut h, self.renditions.len() as u32);
        h.extend(fixed(MAIN_VERSION, 128));
        h.extend(fixed(&format!("{} via AssetCatalogAgent-AssetRuntime", self.xcode), 256));
        h.extend([0u8; 16]); // uuid
        le32(&mut h, 0); // associated checksum
        le32(&mut h, 2); // schema version
        le32(&mut h, 1); // color space
        le32(&mut h, 2); // key semantics
        bom.set(header, h);

        let sorted: Vec<(u32, u32, Vec<u8>)> = rendition_entries.into_iter().map(|e| e.1).collect();
        renditions_tree.finish(&mut bom, &sorted);
        facets_tree.finish(&mut bom, &facet_entries);
        appearances_tree.finish(&mut bom, &appearance_entries);
        bitmap_tree.finish(&mut bom, &bitmap_entries);

        bom.var("CARHEADER", header);
        bom.var("RENDITIONS", renditions_tree.header);
        bom.var("FACETKEYS", facets_tree.header);
        bom.var("APPEARANCEKEYS", appearances_tree.header);
        bom.var("KEYFORMAT", key_format);
        bom.var("EXTENDED_METADATA", extended);
        bom.var("BITMAPKEYS", bitmap_tree.header);
        bom.to_bytes()
    }
}

/// CSI ("ISTC") header fields.
pub struct CsiHeader<'a> {
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub pixel_format: &'a [u8; 4],
    pub color_space: u32,
    pub layout: u16,
    pub name: &'a str,
}

fn tlv(out: &mut Vec<u8>, kind: u32, data: &[u8]) {
    le32(out, kind);
    le32(out, data.len() as u32);
    out.extend_from_slice(data);
}

fn csi(h: &CsiHeader, tlvs: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = b"ISTC".to_vec();
    le32(&mut out, 1);
    le32(&mut out, 0); // flags
    le32(&mut out, h.width);
    le32(&mut out, h.height);
    le32(&mut out, h.scale);
    out.extend_from_slice(h.pixel_format);
    le32(&mut out, h.color_space);
    le32(&mut out, 0); // modification time
    le16(&mut out, h.layout);
    le16(&mut out, 0);
    out.extend(fixed(h.name, 128));
    le32(&mut out, tlvs.len() as u32);
    le32(&mut out, 1); // bitmap count
    le32(&mut out, 0);
    le32(&mut out, payload.len() as u32);
    out.extend_from_slice(tlvs);
    out.extend_from_slice(payload);
    out
}

/// Rows per LZFSE chunk: as many as fit in a third of 4 MiB.
fn rows_per_chunk(bytes_per_row: usize) -> usize {
    std::cmp::max(1, (4 << 20) / 3 / bytes_per_row)
}

/// The payload of an LZFSE-compressed ARGB rendition: an MLEC header and one
/// KCBC chunk per group of rows, each its own LZFSE stream.
fn lzfse_bitmap(bgra: &[u8], width: usize, height: usize) -> Vec<u8> {
    let stride = width * 4;
    let per = rows_per_chunk(stride);
    let mut chunks = Vec::new();
    let mut y = 0;
    while y < height {
        let rows = std::cmp::min(per, height - y);
        chunks.push((rows, lzfse::encode_stream(&bgra[y * stride..(y + rows) * stride])));
        y += rows;
    }
    let mut out = b"MLEC".to_vec();
    le32(&mut out, 3);
    le32(&mut out, 4); // LZFSE
    le32(&mut out, chunks.len() as u32);
    for (rows, data) in chunks {
        out.extend_from_slice(b"KCBC");
        le32(&mut out, 0);
        le32(&mut out, 0);
        le32(&mut out, rows as u32);
        le32(&mut out, data.len() as u32);
        out.extend_from_slice(&data);
    }
    out
}

/// An app icon's image at one size (part 220).
pub fn icon_image_csi(name: &str, width: u32, height: u32, bgra: &[u8]) -> Vec<u8> {
    let mut t = Vec::new();
    let mut slices = Vec::new();
    for v in [1u32, 0, 0, width, height] {
        le32(&mut slices, v);
    }
    tlv(&mut t, 0x3e9, &slices);
    let mut metrics = Vec::new();
    for v in [1u32, 0, 0, 0, 0, width, height] {
        le32(&mut metrics, v);
    }
    tlv(&mut t, 0x3eb, &metrics);
    let mut composition = Vec::new();
    le32(&mut composition, 0);
    composition.extend_from_slice(&1.0f32.to_le_bytes());
    tlv(&mut t, 0x3ec, &composition);
    tlv(&mut t, 0x3ee, &1u32.to_le_bytes());
    tlv(&mut t, 0x3ef, &(width * 4).to_le_bytes());
    let payload = lzfse_bitmap(bgra, width as usize, height as usize);
    csi(&CsiHeader { width, height, scale: 100, pixel_format: b"BGRA", color_space: 1, layout: LAYOUT_ICON_IMAGE, name }, &t, &payload)
}

/// An app icon's index of its sizes (part 218).
pub fn multisized_csi(name: &str, sizes: &[(u32, u32, u32)]) -> Vec<u8> {
    let mut t = Vec::new();
    let mut composition = Vec::new();
    le32(&mut composition, 0);
    composition.extend_from_slice(&0.0f32.to_le_bytes());
    tlv(&mut t, 0x3ec, &composition);
    tlv(&mut t, 0x3ee, &1u32.to_le_bytes());
    let mut payload = b"SISM".to_vec();
    le32(&mut payload, 1);
    le32(&mut payload, sizes.len() as u32);
    for (w, h, index) in sizes {
        le32(&mut payload, *w);
        le32(&mut payload, *h);
        le32(&mut payload, *index);
    }
    csi(&CsiHeader { width: 0, height: 0, scale: 0, pixel_format: &[0; 4], color_space: 0, layout: LAYOUT_MULTISIZED_IMAGE, name }, &t, &payload)
}
