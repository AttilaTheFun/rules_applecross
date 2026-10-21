//! Storyboards: the subset of the document format this compiler supports,
//! and the nibs Interface Builder's compiler makes of it
//! (docs/clean-room/ibtool.md).
//!
//! Supported: scenes with one view controller whose root view has a frame,
//! an autoresizing mask, a safe-area layout guide and a system background
//! color defined in the document's resources. Anything else is refused.

use crate::nib::{self, array, string, Object, Value};
use roxmltree::{Document, Node};

pub struct Compiled {
    /// (file name, contents) inside the .storyboardc directory.
    pub files: Vec<(String, Vec<u8>)>,
}

fn fail<T>(msg: impl Into<String>) -> Result<T, String> {
    Err(msg.into())
}

fn attr<'a>(n: Node<'a, '_>, name: &str) -> Result<&'a str, String> {
    n.attribute(name).ok_or_else(|| format!("<{}> has no {name} attribute", n.tag_name().name()))
}

fn children<'a, 'i>(n: Node<'a, 'i>, tag: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children().filter(move |c| c.is_element() && c.tag_name().name() == tag)
}

fn only_children(n: Node, allowed: &[&str]) -> Result<(), String> {
    for c in n.children().filter(|c| c.is_element()) {
        if !allowed.contains(&c.tag_name().name()) {
            return fail(format!("<{}> inside <{}> is not supported by this ibtool", c.tag_name().name(), n.tag_name().name()));
        }
    }
    Ok(())
}

fn number(s: &str) -> Result<f64, String> {
    s.parse::<f64>().map_err(|_| format!("not a number: {s}"))
}

/// A CGRect or CGPoint archived as NIB data: a 7 (doubles) followed by the
/// components in little-endian order.
fn doubles(v: &[f64]) -> Value {
    let mut d = vec![7u8];
    for x in v {
        d.extend_from_slice(&x.to_le_bytes());
    }
    Value::Data(d)
}

/// The shortest decimal that round-trips, as NSColor archives components.
fn component(x: f64) -> String {
    let s = format!("{x}");
    s.strip_suffix(".0").map(String::from).unwrap_or(s)
}

struct SystemColor {
    white: f64,
    alpha: f64,
}

fn system_colors(doc: &Document) -> Result<Vec<(String, SystemColor)>, String> {
    let mut out = Vec::new();
    for r in doc.root_element().children().filter(|c| c.has_tag_name("resources")) {
        for sc in children(r, "systemColor") {
            let name = attr(sc, "name")?.to_string();
            let color = children(sc, "color").next().ok_or("<systemColor> without <color>")?;
            if color.attribute("white").is_none() {
                return fail(format!("system color {name}: only white/alpha components are supported by this ibtool"));
            }
            if color.attribute("customColorSpace") != Some("genericGamma22GrayColorSpace") {
                return fail(format!("system color {name}: only the genericGamma22Gray color space is supported by this ibtool"));
            }
            out.push((name, SystemColor { white: number(attr(color, "white")?)?, alpha: number(attr(color, "alpha")?)? }));
        }
    }
    Ok(out)
}

fn autoresizing_mask(n: Node) -> Result<i64, String> {
    let mut m = 0;
    for (name, bit) in [("flexibleMinX", 1), ("widthSizable", 2), ("flexibleMaxX", 4), ("flexibleMinY", 8), ("heightSizable", 16), ("flexibleMaxY", 32)] {
        if n.attribute(name) == Some("YES") {
            m |= bit;
        }
    }
    Ok(m)
}

pub fn compile(xml: &str) -> Result<Compiled, String> {
    let doc = Document::parse(xml).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    if root.attribute("type") != Some("com.apple.InterfaceBuilder3.CocoaTouch.Storyboard.XIB") {
        return fail("not an iOS storyboard");
    }
    let colors = system_colors(&doc)?;
    let initial = attr(root, "initialViewController")?;
    let mut files = Vec::new();
    let mut identifiers = Vec::new();
    let scenes = children(root, "scenes").next().ok_or("storyboard has no scenes")?;
    for scene in children(scenes, "scene") {
        only_children(scene, &["objects", "point"])?;
        let objects = children(scene, "objects").next().ok_or("scene without objects")?;
        only_children(objects, &["viewController", "placeholder"])?;
        let vc = children(objects, "viewController").next().ok_or("scene without a view controller")?;
        if vc.attribute("customClass").is_some() {
            return fail("custom view controller classes are not supported by this ibtool");
        }
        only_children(vc, &["view"])?;
        let vc_id = attr(vc, "id")?;
        let view = children(vc, "view").next().ok_or("view controller without a view")?;
        only_children(view, &["rect", "autoresizingMask", "viewLayoutGuide", "color"])?;
        let view_id = attr(view, "id")?;
        if view.attribute("contentMode").is_some_and(|m| m != "scaleToFill") {
            return fail("content modes other than scaleToFill are not supported by this ibtool");
        }
        let rect = children(view, "rect").find(|r| r.attribute("key") == Some("frame")).ok_or("view without a frame")?;
        let (x, y, w, h) = (number(attr(rect, "x")?)?, number(attr(rect, "y")?)?, number(attr(rect, "width")?)?, number(attr(rect, "height")?)?);
        let mask = match children(view, "autoresizingMask").next() {
            Some(m) => autoresizing_mask(m)?,
            None => 0,
        };
        let color = children(view, "color").find(|c| c.attribute("key") == Some("backgroundColor")).ok_or("view without a background color")?;
        let color_name = attr(color, "systemColor")?;
        let sc = &colors.iter().find(|(n, _)| n == color_name).ok_or_else(|| format!("system color {color_name} is not defined in the document"))?.1;
        let has_safe_area = children(view, "viewLayoutGuide").any(|g| g.attribute("key") == Some("safeArea"));
        if !has_safe_area {
            return fail("views without a safe-area layout guide are not supported by this ibtool");
        }

        let vc_nib_name = format!("UIViewController-{vc_id}");
        let view_nib_name = format!("{vc_id}-view-{view_id}");
        let empty = Value::New(Object::new("NSArray", vec![("NSInlinedValue", Value::Bool(false))]).named("empty"));
        let root_values = |top: Vec<Value>, objs: Vec<Value>, connections: Vec<Value>, empty: Value| {
            vec![
                ("UINibTopLevelObjectsKey", array(top)),
                ("UINibObjectsKey", array(objs)),
                ("UINibConnectionsKey", array(connections)),
                ("UINibVisibleWindowsKey", empty),
                ("UINibAccessibilityConfigurationsKey", Value::Ref("empty")),
                ("UINibTraitStorageListsKey", Value::Ref("empty")),
                ("UINibKeyValuePairsKey", Value::Ref("empty")),
                ("IBUINibLNEVersionKey", Value::Int(1)),
            ]
        };
        let proxy = |id: &str, name: &'static str| Value::New(Object::new("UIProxyObject", vec![("UIProxiedObjectIdentifier", string(id))]).named(name));
        let outlet = |label: &str, source: &'static str, destination: &'static str| {
            Value::New(Object::new("UIRuntimeOutletConnection", vec![("UILabel", string(label)), ("UISource", Value::Ref(source)), ("UIDestination", Value::Ref(destination))]))
        };

        // The view controller's nib: the scene's placeholders and a class
        // swapper standing in for the controller, which loads the view nib.
        let class_name = Value::New(Object::new("NSString", vec![("NS.bytes", Value::Data(b"UIViewController".to_vec()))]).named("class"));
        let swapper = Value::New(Object::new("UIClassSwapper", vec![
            ("UIExternalObjectsTableForViewLoading", Value::New(Object::new("NSDictionary", vec![("NSInlinedValue", Value::Bool(false))]))),
            ("UINibName", string(&view_nib_name)),
            ("UIClassName", class_name),
            ("UIOriginalClassName", Value::Ref("class")),
        ]).named("controller"));
        let vc_root = Object::new("NSObject", root_values(
            vec![proxy("IBFilesOwner", "owner"), proxy("IBFirstResponder", "responder"), proxy("UIStoryboardPlaceholder", "storyboard"), swapper],
            vec![Value::Ref("owner"), Value::Ref("responder"), Value::Ref("storyboard"), Value::Ref("controller")],
            vec![outlet("storyboard", "controller", "storyboard"), outlet("sceneViewController", "owner", "controller")],
            empty,
        ));
        files.push((format!("{vc_nib_name}.nib"), nib::archive(vc_root)));

        // The view's nib.
        let ui_color = Object::new("UIColor", vec![
            ("UISystemColorName", string(color_name)),
            ("UIColorComponentCount", Value::Int(2)),
            ("UIWhite", Value::Float(sc.white as f32)),
            ("UIAlpha", Value::Float(sc.alpha as f32)),
            ("NSWhite", Value::Data(component(sc.white).into_bytes())),
            ("NSColorSpace", Value::Int(4)),
        ]).with_fallback("NSColor");
        let ui_view = Value::New(Object::new("UIView", vec![
            ("UIBounds", doubles(&[0.0, 0.0, w, h])),
            ("UICenter", doubles(&[x + w / 2.0, y + h / 2.0])),
            ("UIBackgroundColor", Value::New(ui_color)),
            ("UIDeepDrawRect", Value::Bool(false)),
            ("UIOpaque", Value::Bool(false)),
            ("UIAutoresizeSubviews", Value::Bool(false)),
            ("UIAutoresizingMask", Value::Int(mask)),
            ("UIViewSemanticContentAttribute", Value::Int(0)),
            ("UIViewLargeContentStoredProperties", Value::Nil),
        ]).named("view"));
        let safe_area = Value::New(Object::new("UILayoutGuide", vec![
            ("UILayoutGuideOwningView", Value::Nil),
            ("UILayoutGuideIdentifier", string("")),
            ("UILayoutGuideOwningViewIsLocked", Value::Bool(true)),
            ("UILayoutGuideShouldBeArchived", Value::Bool(true)),
            ("UILayoutGuideAllowsNegativeDimensions", Value::Bool(true)),
        ]));
        let empty = Value::New(Object::new("NSArray", vec![("NSInlinedValue", Value::Bool(false))]).named("empty"));
        let view_root = Object::new("NSObject", root_values(
            vec![proxy("IBFilesOwner", "owner"), proxy("IBFirstResponder", "responder"), ui_view],
            vec![Value::Ref("owner"), Value::Ref("responder"), Value::Ref("view"), safe_area],
            vec![outlet("view", "owner", "view")],
            empty,
        ));
        files.push((format!("{view_nib_name}.nib"), nib::archive(view_root)));
        identifiers.push((vc_nib_name.clone(), vc_id == initial));
    }
    let entry = identifiers.iter().find(|i| i.1).ok_or("the initial view controller is not in a supported scene")?;
    let info = cf::Value::dict([
        ("UIStoryboardDesignatedEntryPointIdentifier", cf::Value::string(entry.0.clone())),
        ("UIStoryboardVersion", cf::Value::Integer(1)),
        ("UIViewControllerIdentifiersToNibNames", cf::Value::dict(identifiers.iter().map(|(n, _)| (n.clone(), cf::Value::string(n.clone()))))),
    ]);
    files.push(("Info.plist".into(), cf::binary::to_bytes(&info)));
    Ok(Compiled { files })
}
