//! A clean-room `actool` for builds on Linux.
//!
//! Compiles asset catalogs the way rules_apple invokes Xcode's actool
//! (`--compile DIR ... --app-icon NAME --target-device ... CATALOG...`),
//! producing the bundle's icon PNGs, Assets.car and the partial Info.plist.
//! What is reproduced, and how it was established from Xcode's outputs, is in
//! docs/clean-room/actool.md. Inputs outside that are refused rather than
//! approximated.

mod bom;
mod car;
mod image;

use cf::Value;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Default)]
struct Options {
    compile: Option<PathBuf>,
    output_format: String,
    platform: String,
    minimum_deployment_target: String,
    app_icon: Option<String>,
    target_devices: Vec<String>,
    partial_info_plist: Option<PathBuf>,
    inputs: Vec<PathBuf>,
}

fn parse(args: Vec<String>) -> Result<Options, String> {
    let mut o = Options { output_format: "xml1".into(), ..Default::default() };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--compile" => o.compile = Some(PathBuf::from(value(&a)?)),
            "--output-format" => o.output_format = value(&a)?,
            "--platform" => o.platform = value(&a)?,
            "--minimum-deployment-target" => o.minimum_deployment_target = value(&a)?,
            "--app-icon" => o.app_icon = Some(value(&a)?),
            "--target-device" => o.target_devices.push(value(&a)?),
            "--output-partial-info-plist" => o.partial_info_plist = Some(PathBuf::from(value(&a)?)),
            "--errors" | "--warnings" | "--notices" | "--compress-pngs" => {}
            "--alternate-app-icon" | "--launch-image" | "--accent-color" | "--widget-background-color" | "--sticker-pack-identifier-prefix"
            | "--stickers-icon-role" | "--filter-for-device-model" | "--filter-for-device-os-version" | "--product-type" | "--development-region" => {
                let v = value(&a)?;
                return Err(format!("{a} {v} is not supported by this actool"));
            }
            s if s.starts_with("--") => return Err(format!("{s} is not supported by this actool")),
            _ => o.inputs.push(PathBuf::from(a)),
        }
    }
    if o.compile.is_none() {
        return Err("--compile is required".into());
    }
    Ok(o)
}

/// CoreUI's 16-bit name identifier for an asset name (docs/clean-room/actool.md):
/// a 64-bit djb2 with seed 100823, folded by evaluating its four 16-bit
/// limbs, low first, as a base-33 polynomial.
fn name_identifier(name: &str) -> u16 {
    let mut h: u64 = 100_823;
    for &c in name.as_bytes() {
        h = h.wrapping_mul(33).wrapping_add(c as u64);
    }
    let limb = |k: u32| (h >> (16 * k)) & 0xffff;
    (limb(0).wrapping_mul(35937).wrapping_add(limb(1) * 1089).wrapping_add(limb(2) * 33).wrapping_add(limb(3)) & 0xffff) as u16
}

struct IconImage {
    file: PathBuf,
    filename: String,
    idiom: String,
    platform: Option<String>,
    size: String,
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn find_app_icon(inputs: &[PathBuf], name: &str) -> Result<PathBuf, String> {
    for catalog in inputs {
        let dir = catalog.join(format!("{name}.appiconset"));
        if dir.is_dir() {
            return Ok(dir);
        }
    }
    Err(format!("none of the input catalogs contains an app icon set named \"{name}\""))
}

fn reject_other_assets(catalog: &Path, icon: Option<&Path>) -> Result<(), String> {
    for entry in std::fs::read_dir(catalog).map_err(|e| format!("{}: {e}", catalog.display()))? {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.file_name().is_some_and(|n| n == "Contents.json") || Some(p.as_path()) == icon {
            continue;
        }
        if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
            continue;
        }
        return Err(format!("{}: only a single app icon set is supported in this catalog for now", p.display()));
    }
    Ok(())
}

fn xcode_identity() -> String {
    // "Xcode 27.0 (27A266a)", from the Xcode whose SDK the build targets.
    let developer = std::env::var("DEVELOPER_DIR").unwrap_or_else(|_| "/Applications/Xcode.app/Contents/Developer".into());
    let plist = Path::new(&developer).join("../version.plist");
    let v = std::fs::read(&plist).ok().and_then(|b| Value::parse(&b).ok());
    let short = v.as_ref().and_then(|v| v.get("CFBundleShortVersionString")).and_then(|x| x.as_str()).unwrap_or("27.0").to_string();
    let build = v.as_ref().and_then(|v| v.get("ProductBuildVersion")).and_then(|x| x.as_str()).unwrap_or("27A266a").to_string();
    format!("Xcode {short} ({build})")
}

fn platform_name(platform: &str) -> Result<&'static str, String> {
    match platform {
        "iphoneos" | "iphonesimulator" => Ok("ios"),
        other => Err(format!("--platform {other} is not supported by this actool")),
    }
}

fn run(o: &Options) -> Result<Vec<PathBuf>, String> {
    let out_dir = o.compile.clone().unwrap();
    if !out_dir.is_dir() {
        return Err(format!("The output directory \"{}\" does not exist.", out_dir.display()));
    }
    let platform = platform_name(&o.platform)?;
    let devices: Vec<&str> = o.target_devices.iter().map(String::as_str).collect();
    let phone = devices.contains(&"iphone");
    let pad = devices.contains(&"ipad");
    let mut outputs = Vec::new();

    let icon_name = o.app_icon.clone().ok_or("a catalog without --app-icon is not supported by this actool yet")?;
    let icon_dir = find_app_icon(&o.inputs, &icon_name)?;
    for c in &o.inputs {
        reject_other_assets(c, Some(&icon_dir))?;
    }
    let contents = read_json(&icon_dir.join("Contents.json"))?;
    let images: Vec<IconImage> = contents["images"]
        .as_array()
        .ok_or("app icon set has no images")?
        .iter()
        .filter_map(|i| {
            Some(IconImage {
                filename: i["filename"].as_str()?.to_string(),
                file: icon_dir.join(i["filename"].as_str()?),
                idiom: i["idiom"].as_str().unwrap_or("universal").to_string(),
                platform: i["platform"].as_str().map(String::from),
                size: i["size"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect();
    let universal: Vec<&IconImage> = images.iter().filter(|i| i.idiom == "universal" && i.platform.as_deref() == Some("ios") && i.size == "1024x1024").collect();
    if universal.len() != 1 || images.len() != 1 {
        return Err("only a single universal 1024x1024 iOS icon is supported by this actool for now".into());
    }
    let icon = universal[0];
    let src = image::decode_png(&std::fs::read(&icon.file).map_err(|e| format!("{}: {e}", icon.file.display()))?)?;
    if src.width != 1024 || src.height != 1024 {
        return Err(format!("{}: is {}x{} but should be 1024x1024", icon.file.display(), src.width, src.height));
    }

    let bgra = src.to_bgra();

    // The bundle's flat icons: 60pt@2x for iPhone, 76pt@2x for iPad.
    let mut phone_files = Vec::new();
    let mut pad_files = Vec::new();
    if phone {
        let p = out_dir.join(format!("{icon_name}60x60@2x.png"));
        std::fs::write(&p, image::cgbi_png(&image::resize(&bgra, 120, 120))).map_err(|e| e.to_string())?;
        outputs.push(p);
        phone_files.push(format!("{icon_name}60x60"));
    }
    if pad {
        let p = out_dir.join(format!("{icon_name}76x76@2x~ipad.png"));
        std::fs::write(&p, image::cgbi_png(&image::resize(&bgra, 152, 152))).map_err(|e| e.to_string())?;
        outputs.push(p);
        pad_files.push(format!("{icon_name}60x60"));
        pad_files.push(format!("{icon_name}76x76"));
    }

    // Assets.car: per idiom, the 1024 px image and the index of its sizes.
    let id = name_identifier(&icon_name);
    let image_csi = car::icon_image_csi(&icon.filename, 1024, 1024, &bgra.data);
    let multi_csi = car::multisized_csi(&icon_name, &[(1024, 1024, 1)]);
    let rendition = |idiom: u16, part: u16, dim2: u16, csi: &Vec<u8>| car::Rendition {
        attrs: vec![(car::ATTR_SCALE, 1), (car::ATTR_IDIOM, idiom), (car::ATTR_DIMENSION2, dim2), (car::ATTR_IDENTIFIER, id), (car::ATTR_ELEMENT, car::ELEMENT_ICON), (car::ATTR_PART, part)],
        csi: csi.clone(),
    };
    let multi = |idiom| rendition(idiom, car::PART_MULTISIZED_IMAGE, 0, &multi_csi);
    let img = |idiom| rendition(idiom, car::PART_ICON_IMAGE, 1, &image_csi);
    // Allocation order observed for one icon set (docs/clean-room/actool.md).
    let renditions = match (phone, pad) {
        (true, true) => vec![multi(car::IDIOM_PAD), img(car::IDIOM_PHONE), multi(car::IDIOM_PHONE), img(car::IDIOM_PAD)],
        (true, false) => vec![img(car::IDIOM_PHONE), multi(car::IDIOM_PHONE)],
        (false, true) => vec![img(car::IDIOM_PAD), multi(car::IDIOM_PAD)],
        (false, false) => return Err("--target-device iphone and/or ipad is required".into()),
    };
    let catalog = car::Catalog {
        key_format: vec![car::ATTR_APPEARANCE, car::ATTR_LOCALIZATION, car::ATTR_SCALE, car::ATTR_IDIOM, car::ATTR_SUBTYPE, car::ATTR_DIMENSION2, car::ATTR_IDENTIFIER, car::ATTR_ELEMENT, car::ATTR_PART],
        facets: vec![car::Facet { name: icon_name.clone(), attrs: vec![(car::ATTR_ELEMENT, car::ELEMENT_ICON), (car::ATTR_PART, car::PART_ICON_IMAGE), (car::ATTR_IDENTIFIER, id)] }],
        appearances: vec![("UIAppearanceAny".into(), 0)],
        renditions,
        xcode: xcode_identity(),
        deployment_target: o.minimum_deployment_target.clone(),
        platform: platform.into(),
    };
    let car_path = out_dir.join("Assets.car");
    std::fs::write(&car_path, catalog.to_bytes()).map_err(|e| e.to_string())?;
    outputs.push(car_path);
    outputs.sort();

    if let Some(p) = &o.partial_info_plist {
        let primary = |files: &Vec<String>| {
            Value::dict([("CFBundlePrimaryIcon", Value::dict([
                ("CFBundleIconFiles", Value::Array(files.iter().map(|f| Value::string(f.clone())).collect())),
                ("CFBundleIconName", Value::string(icon_name.clone())),
            ]))])
        };
        let mut entries = Vec::new();
        if phone {
            entries.push(("CFBundleIcons", primary(&phone_files)));
        }
        if pad {
            entries.push(("CFBundleIcons~ipad", primary(&pad_files)));
        }
        std::fs::write(p, cf::xml::to_bytes(&Value::dict(entries))).map_err(|e| e.to_string())?;
        outputs.push(p.clone());
    }
    Ok(outputs)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        print!("{}", cf::xml::to_string(&Value::dict([("com.apple.actool.version", Value::dict([
            ("bundle-version", Value::string("24303")), ("short-bundle-version", Value::string("27.0"))]))])));
        return ExitCode::SUCCESS;
    }
    let o = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    match run(&o) {
        Ok(outputs) => {
            if o.output_format == "human-readable-text" {
                println!("/* com.apple.actool.compilation-results */");
                for p in outputs {
                    println!("{}", p.display());
                }
                println!();
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            if o.output_format == "human-readable-text" {
                println!("/* com.apple.actool.errors */");
                println!("{}: error: {e}", o.inputs.first().map(|p| p.display().to_string()).unwrap_or_default());
            }
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
