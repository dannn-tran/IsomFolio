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
                        let val = attr.unescape_value().unwrap_or_default().into_owned();
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
                        let v = a.unescape_value().unwrap_or_default();
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
                let text = t.unescape().unwrap_or_default();
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

pub fn parse_sidecar(xmp_path: &str) -> Option<XmpMetadata> {
    let content = std::fs::read_to_string(xmp_path).ok()?;
    let meta = parse_xmp_xml(&content);
    if meta == XmpMetadata::default() { None } else { Some(meta) }
}

pub fn parse_embedded(image_path: &str) -> Option<XmpMetadata> {
    let data = std::fs::read(image_path).ok()?;
    // Quick check: XMP namespace marker must be present
    if !data.windows(28).any(|w| w == b"http://ns.adobe.com/xap/1.0/") {
        return None;
    }
    let packet = find_xmp_packet(&data)?;
    let text = std::str::from_utf8(packet).ok()?;
    let meta = parse_xmp_xml(text);
    if meta == XmpMetadata::default() { None } else { Some(meta) }
}

#[cfg(test)]
mod tests {
    use super::*;

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
