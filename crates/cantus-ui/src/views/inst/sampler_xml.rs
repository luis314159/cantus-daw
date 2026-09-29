//! A small XML reader, enough for Decent Sampler's `.dspreset`: elements, attributes
//! (quoted either way, with entities), comments, the prolog, CDATA and doctype skipped,
//! text ignored. No dependency, no DTDs, no namespaces.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
}

impl Element {
    /// An attribute by name (exact first, then ignoring case: libraries are inconsistent).
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .or_else(|| self.attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)))
            .map(|(_, v)| v.as_str())
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children
            .iter()
            .filter(move |c| c.name.eq_ignore_ascii_case(name))
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else {
            break;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                u32::from_str_radix(&ent[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn starts(&self, pat: &str) -> bool {
        self.s[self.i..].starts_with(pat.as_bytes())
    }
    fn skip_past(&mut self, pat: &str) -> Result<(), String> {
        let p = pat.as_bytes();
        while self.i + p.len() <= self.s.len() {
            if &self.s[self.i..self.i + p.len()] == p {
                self.i += p.len();
                return Ok(());
            }
            self.i += 1;
        }
        Err(format!("unterminated construct, expected {pat}"))
    }
    fn name(&mut self) -> String {
        let start = self.i;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_whitespace() || c == b'>' || c == b'/' || c == b'=' {
                break;
            }
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
    }
    /// Skip text, comments, processing instructions and declarations up to the next tag.
    fn seek_tag(&mut self) -> Result<bool, String> {
        loop {
            while self.i < self.s.len() && self.s[self.i] != b'<' {
                self.i += 1;
            }
            if self.i >= self.s.len() {
                return Ok(false);
            }
            if self.starts("<!--") {
                self.skip_past("-->")?;
            } else if self.starts("<![CDATA[") {
                self.skip_past("]]>")?;
            } else if self.starts("<?") {
                self.skip_past("?>")?;
            } else if self.starts("<!") {
                self.skip_past(">")?;
            } else {
                return Ok(true);
            }
        }
    }
    fn element(&mut self, depth: usize) -> Result<Element, String> {
        if depth > 64 {
            return Err("elements nested too deeply".into());
        }
        self.i += 1; // '<'
        let mut e = Element {
            name: self.name(),
            ..Element::default()
        };
        loop {
            self.skip_ws();
            if self.i >= self.s.len() {
                return Err(format!("<{}> is not closed", e.name));
            }
            match self.s[self.i] {
                b'/' => {
                    self.skip_past(">")?;
                    return Ok(e);
                }
                b'>' => {
                    self.i += 1;
                    break;
                }
                _ => {
                    let k = self.name();
                    self.skip_ws();
                    let mut v = String::new();
                    if self.i < self.s.len() && self.s[self.i] == b'=' {
                        self.i += 1;
                        self.skip_ws();
                        let q = *self.s.get(self.i).ok_or("attribute without value")?;
                        if q == b'"' || q == b'\'' {
                            self.i += 1;
                            let start = self.i;
                            while self.i < self.s.len() && self.s[self.i] != q {
                                self.i += 1;
                            }
                            v = unescape(&String::from_utf8_lossy(&self.s[start..self.i]));
                            self.i += 1;
                        } else {
                            v = self.name();
                        }
                    }
                    if k.is_empty() {
                        self.i += 1;
                    } else {
                        e.attrs.push((k, v));
                    }
                }
            }
        }
        // Children until our closing tag.
        loop {
            if !self.seek_tag()? {
                return Err(format!("<{}> is not closed", e.name));
            }
            if self.starts("</") {
                self.skip_past(">")?;
                return Ok(e);
            }
            e.children.push(self.element(depth + 1)?);
        }
    }
}

/// Parse a document; its root element.
pub fn parse(text: &str) -> Result<Element, String> {
    let text = text.trim_start_matches('\u{feff}');
    let mut p = P {
        s: text.as_bytes(),
        i: 0,
    };
    if !p.seek_tag()? {
        return Err("no element in the document".into());
    }
    p.element(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_elements_attributes_and_skips_the_rest() {
        let doc = r#"<?xml version="1.0"?>
<!-- a comment -->
<DecentSampler minVersion='1.0'>
  <groups volume="-3dB"><group name="A &amp; B">
    <sample path="Samples/c4.wav" rootNote="60"/>
    <![CDATA[ <ignored/> ]]>
  </group></groups>
</DecentSampler>"#;
        let root = parse(doc).unwrap();
        assert_eq!(root.name, "DecentSampler");
        assert_eq!(root.attr("minversion"), Some("1.0"));
        let g = root.child("groups").unwrap();
        assert_eq!(g.attr("volume"), Some("-3dB"));
        let group = g.child("group").unwrap();
        assert_eq!(group.attr("name"), Some("A & B"));
        assert_eq!(group.children.len(), 1);
        assert_eq!(group.children[0].attr("rootNote"), Some("60"));
        assert!(parse("<a><b></a").is_err());
    }
}
