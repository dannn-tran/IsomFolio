use std::collections::HashMap;
use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct XmpCore {
    pub rating: Option<i32>,
    pub label: Option<String>,
    pub create_date: Option<String>,
    pub modify_date: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct DublinCore {
    pub title: Option<String>,
    pub description: Option<String>,
    pub creator: Vec<String>,
    pub subject: Vec<String>,
    pub rights: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct XmpMetadata {
    pub core: XmpCore,
    pub dublin_core: DublinCore,
}

const NS_XMP: &str = "http://ns.adobe.com/xap/1.0/";
const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
const NS_RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const NS_XML: &str = "http://www.w3.org/XML/1998/namespace";

/// Map declared XML namespace prefixes found in the packet.
fn build_ns_map(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    // Defaults
    map.insert("xmp".to_string(), NS_XMP.to_string());
    map.insert("dc".to_string(), NS_DC.to_string());
    map.insert("rdf".to_string(), NS_RDF.to_string());
    map.insert("xml".to_string(), NS_XML.to_string());

    // Extract xmlns:prefix="uri" declarations via simple scan
    let mut rest = xml;
    while let Some(pos) = rest.find("xmlns:") {
        rest = &rest[pos + 6..];
        if let Some(eq) = rest.find('=') {
            let prefix = rest[..eq].trim().to_string();
            rest = &rest[eq + 1..].trim_start_matches(|c| c == ' ' || c == '\t');
            if rest.starts_with('"') || rest.starts_with('\'') {
                let quote = rest.chars().next().unwrap();
                rest = &rest[1..];
                if let Some(end) = rest.find(quote) {
                    let uri = rest[..end].to_string();
                    rest = &rest[end + 1..];
                    map.insert(prefix, uri);
                }
            }
        }
    }
    map
}

fn resolve_prefix<'a>(name: &'a str, ns_map: &'a HashMap<String, String>) -> (&'a str, &'a str) {
    if let Some(colon) = name.find(':') {
        let prefix = &name[..colon];
        let local = &name[colon + 1..];
        let ns = ns_map.get(prefix).map(|s| s.as_str()).unwrap_or("");
        (ns, local)
    } else {
        ("", name)
    }
}

#[derive(Default)]
struct ParseState {
    // rdf:Description attributes have been processed
    in_description: bool,
    // current dc/xmp element name we're inside (e.g. "dc:subject")
    current_prop: Option<(String, String)>, // (ns, local)
    // rdf:Alt, rdf:Seq, rdf:Bag context
    in_container: bool,
    // rdf:li state
    in_li: bool,
    li_is_xdefault: bool,
    li_text: String,
    // collected values
    core: XmpCore,
    dc: DublinCore,
    // list accumulator for dc:creator / dc:subject
    list_items: Vec<String>,
    // alt text accumulator (x-default)
    alt_xdefault: Option<String>,
}

pub fn parse_xmp_xml(xml: &str) -> XmpMetadata {
    let ns_map = build_ns_map(xml);
    let mut state = ParseState::default();

    // Element stack: (ns, local_name)
    let mut stack: Vec<(String, String)> = Vec::new();

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                let (ns, local) = resolve_prefix(&name, &ns_map);
                let ns = ns.to_string();
                let local = local.to_string();

                // Handle rdf:Description → read attribute-form properties
                if ns == NS_RDF && local == "Description" {
                    state.in_description = true;
                    for attr in e.attributes().flatten() {
                        let attr_name = std::str::from_utf8(attr.key.as_ref()).unwrap_or("").to_string();
                        let val = decode_entities(attr.value.as_ref());
                        let (ans, alocal) = resolve_prefix(&attr_name, &ns_map);
                        match (ans, alocal) {
                            (ns, "Rating") if ns == NS_XMP => {
                                state.core.rating = val.parse().ok();
                            }
                            (ns, "Label") if ns == NS_XMP => {
                                state.core.label = Some(val);
                            }
                            (ns, "CreateDate") if ns == NS_XMP => {
                                state.core.create_date = Some(val);
                            }
                            (ns, "ModifyDate") if ns == NS_XMP => {
                                state.core.modify_date = Some(val);
                            }
                            _ => {}
                        }
                    }
                }

                // Track which DC/XMP property element we're in
                if state.in_description && stack.last().map(|(n, _)| n == NS_RDF).unwrap_or(false) {
                    // Direct child of rdf:Description
                }
                if state.in_description && (ns == NS_DC || ns == NS_XMP) {
                    // entering a property element
                    state.current_prop = Some((ns.clone(), local.clone()));
                    state.list_items.clear();
                    state.alt_xdefault = None;
                    state.in_container = false;
                }

                // rdf:Alt / rdf:Seq / rdf:Bag
                if ns == NS_RDF && (local == "Alt" || local == "Seq" || local == "Bag") {
                    state.in_container = true;
                }

                // rdf:li
                if ns == NS_RDF && local == "li" {
                    state.in_li = true;
                    state.li_text.clear();
                    // Check xml:lang="x-default"
                    state.li_is_xdefault = e.attributes().flatten().any(|a| {
                        let k = std::str::from_utf8(a.key.as_ref()).unwrap_or("");
                        let v = std::str::from_utf8(a.value.as_ref()).unwrap_or_default();
                        let (ans, al) = resolve_prefix(k, &ns_map);
                        ans == NS_XML && al == "lang" && v == "x-default"
                    });
                }

                // Inline xmp element (e.g. <xmp:Rating>5</xmp:Rating>)
                if ns == NS_XMP && state.in_description && !state.in_li {
                    state.current_prop = Some((ns.clone(), local.clone()));
                    state.li_text.clear();
                }

                stack.push((ns, local));
            }

            Ok(Event::End(_)) => {
                let popped = stack.pop();

                let (top_ns, top_local) = match &popped {
                    Some(p) => (p.0.as_str(), p.1.as_str()),
                    None => continue,
                };

                // Closing rdf:li
                if top_ns == NS_RDF && top_local == "li" {
                    let text = std::mem::take(&mut state.li_text);
                    if state.in_li {
                        if state.li_is_xdefault {
                            state.alt_xdefault = Some(text.clone());
                        }
                        if !text.is_empty() {
                            state.list_items.push(text);
                        }
                    }
                    state.in_li = false;
                    continue;
                }

                // Closing a DC/XMP property element
                if let Some((ref prop_ns, ref prop_local)) = state.current_prop.clone() {
                    if top_ns == prop_ns && top_local == prop_local {
                        let items = std::mem::take(&mut state.list_items);
                        let alt = state.alt_xdefault.take();
                        let inline_text = std::mem::take(&mut state.li_text);

                        match (prop_ns.as_str(), prop_local.as_str()) {
                            (ns, "Rating") if ns == NS_XMP => {
                                state.core.rating = inline_text.parse().ok();
                            }
                            (ns, "Label") if ns == NS_XMP => {
                                if !inline_text.is_empty() {
                                    state.core.label = Some(inline_text);
                                }
                            }
                            (ns, "CreateDate") if ns == NS_XMP => {
                                if !inline_text.is_empty() {
                                    state.core.create_date = Some(inline_text);
                                }
                            }
                            (ns, "ModifyDate") if ns == NS_XMP => {
                                if !inline_text.is_empty() {
                                    state.core.modify_date = Some(inline_text);
                                }
                            }
                            (ns, "title") if ns == NS_DC => {
                                state.dc.title = alt.or_else(|| items.into_iter().next());
                            }
                            (ns, "description") if ns == NS_DC => {
                                state.dc.description = alt.or_else(|| items.into_iter().next());
                            }
                            (ns, "creator") if ns == NS_DC => {
                                state.dc.creator = items;
                            }
                            (ns, "subject") if ns == NS_DC => {
                                state.dc.subject = items;
                            }
                            (ns, "rights") if ns == NS_DC => {
                                state.dc.rights = alt.or_else(|| items.into_iter().next());
                            }
                            _ => {}
                        }
                        state.current_prop = None;
                        state.in_container = false;
                    }
                }

                if top_ns == NS_RDF && top_local == "Description" {
                    state.in_description = false;
                }
            }

            Ok(Event::Text(t)) => {
                let raw = std::str::from_utf8(t.as_ref()).unwrap_or_default();
                let text = quick_xml::escape::unescape(raw).unwrap_or(std::borrow::Cow::Borrowed(raw));
                if state.in_li {
                    state.li_text.push_str(&text);
                } else if state.current_prop.is_some() && !state.in_container {
                    state.li_text.push_str(&text);
                }
            }

            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    XmpMetadata {
        core: state.core,
        dublin_core: state.dc,
    }
}

fn find_xmp_packet(data: &[u8]) -> Option<&[u8]> {
    let start_marker = b"<x:xmpmeta";
    let alt_marker = b"<xmpmeta";
    let end_marker1 = b"</x:xmpmeta>";
    let end_marker2 = b"</xmpmeta>";

    let start = find_bytes(data, start_marker)
        .or_else(|| find_bytes(data, alt_marker))?;

    let after = &data[start..];
    let end_offset = find_bytes(after, end_marker1)
        .map(|p| p + end_marker1.len())
        .or_else(|| find_bytes(after, end_marker2).map(|p| p + end_marker2.len()))?;

    Some(&data[start..start + end_offset])
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ArrayKind {
    Bag,
    Seq,
}

#[derive(Debug, Clone)]
enum PropValue {
    Simple(String),
    LangAlt(Vec<(String, String)>), // (xml:lang, value); "x-default" first
    Array(ArrayKind, Vec<String>),
    Raw(String), // verbatim element XML for shapes we don't model (preserved as-is)
}

#[derive(Debug, Clone)]
struct XmpProp {
    raw_name: String, // qualified name as written, e.g. "dc:title"
    ns: String,       // resolved namespace URI
    local: String,    // local name
    value: PropValue,
}

/// A round-trippable model of an XMP packet's `rdf:Description` properties. Every
/// property is preserved (typed where understood, verbatim `Raw` otherwise) so a
/// write merges edits on top of an existing sidecar without losing unmanaged
/// fields or namespaces.
#[derive(Debug, Clone, Default)]
pub struct XmpDoc {
    namespaces: Vec<(String, String)>, // (prefix, uri)
    props: Vec<XmpProp>,
}

impl XmpDoc {
    fn with_default_ns() -> Self {
        XmpDoc {
            namespaces: vec![
                ("xmp".to_string(), NS_XMP.to_string()),
                ("dc".to_string(), NS_DC.to_string()),
            ],
            props: Vec::new(),
        }
    }

    fn ensure_ns(&mut self, prefix: &str, uri: &str) {
        if !self.namespaces.iter().any(|(p, _)| p == prefix) {
            self.namespaces.push((prefix.to_string(), uri.to_string()));
        }
    }

    fn remove(&mut self, ns: &str, local: &str) {
        self.props.retain(|p| !(p.ns == ns && p.local == local));
    }

    /// Set a managed property to `value`, or remove it when `None`/empty.
    fn set(&mut self, raw_name: &str, ns: &str, local: &str, value: Option<PropValue>) {
        self.remove(ns, local);
        if let Some(v) = value {
            self.props.push(XmpProp {
                raw_name: raw_name.to_string(),
                ns: ns.to_string(),
                local: local.to_string(),
                value: v,
            });
        }
    }

    pub fn serialize(&self) -> String {
        let mut ns_decl = String::new();
        for (prefix, uri) in &self.namespaces {
            if prefix == "rdf" || prefix == "x" {
                continue;
            }
            ns_decl.push_str(&format!("\n    xmlns:{prefix}=\"{}\"", xml_escape(uri)));
        }
        let mut body = String::new();
        for p in &self.props {
            body.push_str(&serialize_prop(p));
        }
        format!(
            "<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
 <rdf:RDF xmlns:rdf=\"{NS_RDF}\">\n\
  <rdf:Description rdf:about=\"\"{ns_decl}>\n\
{body}  </rdf:Description>\n\
 </rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>\n"
        )
    }
}

fn serialize_prop(p: &XmpProp) -> String {
    match &p.value {
        PropValue::Raw(s) => format!("   {}\n", s.trim()),
        PropValue::Simple(v) => format!("   <{0}>{1}</{0}>\n", p.raw_name, xml_escape(v)),
        PropValue::LangAlt(items) => {
            let mut s = format!("   <{}><rdf:Alt>\n", p.raw_name);
            for (lang, v) in items {
                s.push_str(&format!(
                    "    <rdf:li xml:lang=\"{}\">{}</rdf:li>\n",
                    xml_escape(lang),
                    xml_escape(v)
                ));
            }
            s.push_str(&format!("   </rdf:Alt></{}>\n", p.raw_name));
            s
        }
        PropValue::Array(kind, items) => {
            let container = match kind {
                ArrayKind::Bag => "rdf:Bag",
                ArrayKind::Seq => "rdf:Seq",
            };
            let mut s = format!("   <{}><{container}>\n", p.raw_name);
            for v in items {
                s.push_str(&format!("    <rdf:li>{}</rdf:li>\n", xml_escape(v)));
            }
            s.push_str(&format!("   </{container}></{}>\n", p.raw_name));
            s
        }
    }
}

/// Attribute-form properties on an `rdf:Description` (e.g. `xmp:Rating="5"`),
/// skipping namespace declarations and `rdf:about`. Values are entity-decoded.
fn description_attr_props(
    e: &quick_xml::events::BytesStart<'_>,
    ns_map: &HashMap<String, String>,
) -> Vec<XmpProp> {
    let mut out = Vec::new();
    for attr in e.attributes().flatten() {
        let k = std::str::from_utf8(attr.key.as_ref()).unwrap_or("").to_string();
        if k == "rdf:about" || k == "xmlns" || k.starts_with("xmlns:") {
            continue;
        }
        let (ans, al) = resolve_prefix(&k, ns_map);
        if ans.is_empty() {
            continue;
        }
        out.push(XmpProp {
            raw_name: k.clone(),
            ns: ans.to_string(),
            local: al.to_string(),
            value: PropValue::Simple(decode_entities(attr.value.as_ref())),
        });
    }
    out
}

/// Parse a full XMP packet into a round-trippable document (all properties).
pub fn parse_xmp_doc(xml: &str) -> XmpDoc {
    let ns_map = build_ns_map(xml);
    let mut namespaces: Vec<(String, String)> = ns_map.iter().map(|(p, u)| (p.clone(), u.clone())).collect();
    namespaces.sort();
    let mut props: Vec<XmpProp> = Vec::new();

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Advance to the first rdf:Description, capturing its attribute-form props.
    let mut in_desc = false;
    loop {
        // `rdf:Description` may be a Start tag (with child-element props) or a
        // self-closing Empty tag (attribute-form props only). Handle both, or a
        // purely-attribute Description's fields are silently dropped on merge.
        let (e, is_empty) = match reader.read_event() {
            Ok(Event::Start(e)) => (e, false),
            Ok(Event::Empty(e)) => (e, true),
            Ok(Event::Eof) | Err(_) => break,
            _ => continue,
        };
        let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
        let (ns, local) = resolve_prefix(&name, &ns_map);
        if ns == NS_RDF && local == "Description" {
            props.extend(description_attr_props(&e, &ns_map));
            // Start form has child-element props to iterate next; Empty form does not.
            in_desc = !is_empty;
            break;
        }
    }

    // Iterate the direct children of rdf:Description.
    if in_desc {
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    let raw_name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                    let (ns, local) = {
                        let (n, l) = resolve_prefix(&raw_name, &ns_map);
                        (n.to_string(), l.to_string())
                    };
                    // Collect the whole subtree (owned) so we can classify it.
                    let mut events: Vec<Event<'static>> = vec![Event::Start(e.into_owned())];
                    let mut depth = 1;
                    while depth > 0 {
                        match reader.read_event() {
                            Ok(Event::Start(s)) => { depth += 1; events.push(Event::Start(s.into_owned())); }
                            Ok(Event::End(en)) => { depth -= 1; events.push(Event::End(en.into_owned())); }
                            Ok(Event::Eof) | Err(_) => break,
                            Ok(ev) => events.push(ev.into_owned()),
                        }
                    }
                    let value = classify_prop(&events, &ns_map, &raw_name);
                    props.push(XmpProp { raw_name, ns, local, value });
                }
                Ok(Event::Empty(e)) => {
                    let raw_name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                    let (ns, local) = {
                        let (n, l) = resolve_prefix(&raw_name, &ns_map);
                        (n.to_string(), l.to_string())
                    };
                    props.push(XmpProp { raw_name, ns, local, value: PropValue::Simple(String::new()) });
                }
                Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    XmpDoc { namespaces, props }
}

fn decode_text(t: &quick_xml::events::BytesText<'_>) -> String {
    decode_entities(t.as_ref())
}

/// Decode XML entities in a raw attribute/text byte slice (`&amp;` → `&`, etc.).
fn decode_entities(raw: &[u8]) -> String {
    let s = std::str::from_utf8(raw).unwrap_or_default();
    quick_xml::escape::unescape(s)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| s.to_string())
}

fn classify_prop(events: &[Event<'static>], ns_map: &HashMap<String, String>, raw_name: &str) -> PropValue {
    // Detect a container (rdf:Alt/Bag/Seq) anywhere inside.
    let mut container: Option<&'static str> = None;
    let mut nested_element = false;
    for ev in &events[1..events.len().saturating_sub(1)] {
        if let Event::Start(s) | Event::Empty(s) = ev {
            let n = std::str::from_utf8(s.name().as_ref()).unwrap_or("").to_string();
            let (ns, local) = resolve_prefix(&n, ns_map);
            if ns == NS_RDF && local == "Alt" { container = Some("Alt"); }
            else if ns == NS_RDF && local == "Bag" { container = Some("Bag"); }
            else if ns == NS_RDF && local == "Seq" { container = Some("Seq"); }
            else if ns == NS_RDF && local == "li" { /* item */ }
            else { nested_element = true; }
        }
    }

    if let Some(kind) = container {
        // Collect rdf:li (lang + text).
        let mut items: Vec<(String, String)> = Vec::new();
        let mut cur_lang: Option<String> = None;
        let mut in_li = false;
        let mut text = String::new();
        for ev in events {
            match ev {
                Event::Start(s) => {
                    let n = std::str::from_utf8(s.name().as_ref()).unwrap_or("").to_string();
                    let (ns, local) = resolve_prefix(&n, ns_map);
                    if ns == NS_RDF && local == "li" {
                        in_li = true;
                        text.clear();
                        cur_lang = s.attributes().flatten().find_map(|a| {
                            let k = std::str::from_utf8(a.key.as_ref()).unwrap_or("").to_string();
                            let (ans, al) = resolve_prefix(&k, ns_map);
                            if ans == NS_XML && al == "lang" {
                                Some(std::str::from_utf8(a.value.as_ref()).unwrap_or_default().to_string())
                            } else { None }
                        });
                    }
                }
                Event::Text(t) if in_li => {
                    text.push_str(&decode_text(t));
                }
                Event::End(en) => {
                    let n = std::str::from_utf8(en.name().as_ref()).unwrap_or("").to_string();
                    let (ns, local) = resolve_prefix(&n, ns_map);
                    if ns == NS_RDF && local == "li" && in_li {
                        items.push((cur_lang.take().unwrap_or_default(), std::mem::take(&mut text)));
                        in_li = false;
                    }
                }
                _ => {}
            }
        }
        if kind == "Alt" {
            return PropValue::LangAlt(items);
        }
        let ak = if kind == "Seq" { ArrayKind::Seq } else { ArrayKind::Bag };
        return PropValue::Array(ak, items.into_iter().map(|(_, v)| v).collect());
    }

    if !nested_element {
        // Simple: text between start and end.
        let mut text = String::new();
        for ev in events {
            if let Event::Text(t) = ev {
                text.push_str(&decode_text(t));
            }
        }
        return PropValue::Simple(text);
    }

    // Unknown nested structure → preserve verbatim by re-serializing the events.
    let _ = raw_name;
    let mut writer = quick_xml::Writer::new(Vec::<u8>::new());
    for ev in events {
        let _ = writer.write_event(ev.clone());
    }
    let raw = String::from_utf8(writer.into_inner()).unwrap_or_default();
    PropValue::Raw(raw)
}

/// Merge IsomFolio-managed metadata into an existing sidecar (if any), preserving
/// every other property and namespace. A field that is `None`/empty is removed
/// from the output, so what another app reads matches the catalog exactly.
pub fn merge_sidecar(existing: Option<&str>, meta: &XmpMetadata, subjects: &[String]) -> String {
    let mut doc = match existing {
        Some(x) if !x.trim().is_empty() => parse_xmp_doc(x),
        _ => XmpDoc::with_default_ns(),
    };
    doc.ensure_ns("xmp", NS_XMP);
    doc.ensure_ns("dc", NS_DC);

    doc.set("xmp:Rating", NS_XMP, "Rating", meta.core.rating.map(|r| PropValue::Simple(r.to_string())));
    doc.set("xmp:Label", NS_XMP, "Label", meta.core.label.clone().map(PropValue::Simple));

    let langalt = |v: &Option<String>| {
        v.clone().filter(|s| !s.is_empty()).map(|s| PropValue::LangAlt(vec![("x-default".to_string(), s)]))
    };
    doc.set("dc:title", NS_DC, "title", langalt(&meta.dublin_core.title));
    doc.set("dc:description", NS_DC, "description", langalt(&meta.dublin_core.description));
    doc.set("dc:rights", NS_DC, "rights", langalt(&meta.dublin_core.rights));

    let creator = if meta.dublin_core.creator.is_empty() {
        None
    } else {
        Some(PropValue::Array(ArrayKind::Seq, meta.dublin_core.creator.clone()))
    };
    doc.set("dc:creator", NS_DC, "creator", creator);

    let subj = if subjects.is_empty() {
        None
    } else {
        Some(PropValue::Array(ArrayKind::Bag, subjects.to_vec()))
    };
    doc.set("dc:subject", NS_DC, "subject", subj);

    doc.serialize()
}

/// Serialize an XMP sidecar from scratch (no existing file to merge into).
pub fn serialize_sidecar(meta: &XmpMetadata, subjects: &[String]) -> String {
    merge_sidecar(None, meta, subjects)
}

pub fn parse_sidecar(xmp_path: &str) -> Option<XmpMetadata> {
    let content = std::fs::read_to_string(xmp_path).ok()?;
    let meta = parse_xmp_xml(&content);
    if meta == XmpMetadata::default() { None } else { Some(meta) }
}

pub fn parse_embedded(image_path: &str) -> Option<XmpMetadata> {
    let data = std::fs::read(image_path).ok()?;
    parse_embedded_from_bytes(&data)
}

/// Parse an embedded XMP packet from an in-memory image buffer, so the bytes can
/// be shared with the EXIF parser instead of re-reading the file.
pub fn parse_embedded_from_bytes(data: &[u8]) -> Option<XmpMetadata> {
    // Quick check: XMP namespace marker must be present
    if !data.windows(28).any(|w| w == b"http://ns.adobe.com/xap/1.0/") {
        return None;
    }
    let packet = find_xmp_packet(data)?;
    let text = std::str::from_utf8(packet).ok()?;
    let meta = parse_xmp_xml(text);
    if meta == XmpMetadata::default() { None } else { Some(meta) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_sidecar_round_trips_through_parser() {
        let meta = XmpMetadata {
            core: XmpCore { rating: Some(4), label: Some("Red".into()), ..Default::default() },
            dublin_core: DublinCore {
                title: Some("Harbour at dawn".into()),
                description: Some("Fishing boats leaving port".into()),
                creator: vec!["Jane Doe".into()],
                subject: vec![],
                rights: Some("(c) 2024 Jane Doe".into()),
            },
        };
        let xml = serialize_sidecar(&meta, &["nature".to_string(), "travel".to_string()]);
        let parsed = parse_xmp_xml(&xml);
        assert_eq!(parsed.core.rating, Some(4));
        assert_eq!(parsed.core.label.as_deref(), Some("Red"));
        assert_eq!(parsed.dublin_core.title.as_deref(), Some("Harbour at dawn"));
        assert_eq!(parsed.dublin_core.description.as_deref(), Some("Fishing boats leaving port"));
        assert_eq!(parsed.dublin_core.creator, vec!["Jane Doe".to_string()]);
        assert_eq!(parsed.dublin_core.subject, vec!["nature".to_string(), "travel".to_string()]);
        assert_eq!(parsed.dublin_core.rights.as_deref(), Some("(c) 2024 Jane Doe"));
    }

    #[test]
    fn merge_preserves_unmanaged_fields_and_drops_emptied() {
        // An existing sidecar from another app: GPS (unmanaged, nested), a custom
        // namespace property, plus a rating and a caption IsomFolio manages.
        let existing = r#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:exif="http://ns.adobe.com/exif/1.0/"
    xmp:Rating="2">
   <exif:GPSLatitude>51,30.5N</exif:GPSLatitude>
   <dc:description><rdf:Alt><rdf:li xml:lang="x-default">old caption</rdf:li></rdf:Alt></dc:description>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

        // IsomFolio's catalog: rating 5, caption cleared, a creator added, tags.
        let meta = XmpMetadata {
            core: XmpCore { rating: Some(5), ..Default::default() },
            dublin_core: DublinCore {
                creator: vec!["Jane Doe".into()],
                ..Default::default()
            },
        };
        let merged = merge_sidecar(Some(existing), &meta, &["nature".to_string()]);

        // Unmanaged GPS survives verbatim.
        assert!(merged.contains("exif:GPSLatitude"));
        assert!(merged.contains("51,30.5N"));
        assert!(merged.contains("xmlns:exif="));
        // Managed rating updated 2 → 5; old caption dropped (cleared in catalog).
        let parsed = parse_xmp_doc(&merged);
        let get = |local: &str| parsed.props.iter().find(|p| p.ns == NS_DC && p.local == local || p.ns == NS_XMP && p.local == local);
        assert!(matches!(get("Rating").map(|p| &p.value), Some(PropValue::Simple(s)) if s == "5"));
        assert!(get("description").is_none(), "emptied caption must not be written");
        assert!(get("creator").is_some());
        assert!(get("subject").is_some());
    }

    #[test]
    fn serialize_sidecar_xml_escapes_specials() {
        let meta = XmpMetadata {
            core: XmpCore::default(),
            dublin_core: DublinCore { title: Some("A & B <c>".into()), ..Default::default() },
        };
        let xml = serialize_sidecar(&meta, &[]);
        assert!(xml.contains("A &amp; B &lt;c&gt;"));
    }

    #[test]
    fn parse_rating_and_label_as_attributes() {
        let xml = r#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/"
                     xmp:Rating="4" xmp:Label="Red"/>
  </rdf:RDF>
</x:xmpmeta>"#;
        let meta = parse_xmp_xml(xml);
        assert_eq!(meta.core.rating, Some(4));
        assert_eq!(meta.core.label.as_deref(), Some("Red"));
    }

    #[test]
    fn parse_decodes_entities_in_attribute_values() {
        // A text field stored as an rdf:Description attribute with XML entities.
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/"
                     xmp:Label="Sun &amp; Sea &lt;draft&gt;"/>
  </rdf:RDF>
</x:xmpmeta>"#;
        let meta = parse_xmp_xml(xml);
        assert_eq!(meta.core.label.as_deref(), Some("Sun & Sea <draft>"));
    }

    #[test]
    fn merge_preserves_attribute_form_props_on_self_closing_description() {
        // Some apps write a purely-attribute, self-closing Description. Its
        // unmanaged fields must survive our merge-write, not be dropped.
        let existing = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
                     xmlns:xmp="http://ns.adobe.com/xap/1.0/"
                     photoshop:City="Paris" xmp:Rating="2"/>
  </rdf:RDF>
</x:xmpmeta>"#;
        let meta = XmpMetadata {
            core: XmpCore { rating: Some(5), ..Default::default() },
            ..Default::default()
        };
        let merged = merge_sidecar(Some(existing), &meta, &[]);
        assert!(merged.contains("Paris"), "unmanaged attribute prop must survive");
        let parsed = parse_xmp_doc(&merged);
        let rating = parsed.props.iter().find(|p| p.local == "Rating").unwrap();
        assert!(matches!(&rating.value, PropValue::Simple(s) if s == "5"), "managed prop updated");
    }

    #[test]
    fn parse_doc_decodes_entities_in_attribute_values() {
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
                     photoshop:Headline="A &amp; B"/>
  </rdf:RDF>
</x:xmpmeta>"#;
        let doc = parse_xmp_doc(xml);
        let headline = doc.props.iter().find(|p| p.local == "Headline").unwrap();
        assert!(matches!(&headline.value, PropValue::Simple(s) if s == "A & B"));
    }

    #[test]
    fn parse_dc_subject_bag() {
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
           xmlns:dc="http://purl.org/dc/elements/1.1/">
    <rdf:Description>
      <dc:subject>
        <rdf:Bag>
          <rdf:li>nature</rdf:li>
          <rdf:li>travel</rdf:li>
        </rdf:Bag>
      </dc:subject>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>"#;
        let meta = parse_xmp_xml(xml);
        assert_eq!(meta.dublin_core.subject, vec!["nature", "travel"]);
    }

    #[test]
    fn parse_dc_title_alt() {
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
           xmlns:dc="http://purl.org/dc/elements/1.1/"
           xmlns:xml="http://www.w3.org/XML/1998/namespace">
    <rdf:Description>
      <dc:title>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">My Photo</rdf:li>
        </rdf:Alt>
      </dc:title>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>"#;
        let meta = parse_xmp_xml(xml);
        assert_eq!(meta.dublin_core.title.as_deref(), Some("My Photo"));
    }

    #[test]
    fn parse_dc_creator_seq() {
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
           xmlns:dc="http://purl.org/dc/elements/1.1/">
    <rdf:Description>
      <dc:creator>
        <rdf:Seq>
          <rdf:li>Jane Doe</rdf:li>
        </rdf:Seq>
      </dc:creator>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>"#;
        let meta = parse_xmp_xml(xml);
        assert_eq!(meta.dublin_core.creator, vec!["Jane Doe"]);
    }
}
