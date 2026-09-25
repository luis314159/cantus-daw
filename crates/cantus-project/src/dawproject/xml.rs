//! Just enough XML writing for `project.xml` and `metadata.xml`: elements, attributes in
//! the order given, text, escaping, two-space indentation.

/// Escape text for an attribute value or element content.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // XML 1.0 cannot carry most control characters at all; a name with one in it
            // loses the character rather than making the file unreadable.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// A number as `xs:double` reads it: the shortest text that parses back to the same
/// value, never `NaN` or an exponent-free infinity.
pub fn num(v: f64) -> String {
    if v.is_finite() {
        // `{}` on f64 is the shortest round-tripping form, and has no exponent for the
        // magnitudes a project holds.
        let s = format!("{v}");
        if s.contains('.') || s.contains('e') {
            s
        } else {
            format!("{s}.0")
        }
    } else {
        "0.0".to_string()
    }
}

pub struct XmlWriter {
    out: String,
    stack: Vec<&'static str>,
    /// The element just opened has had no child yet: close it as `/>` if none comes.
    open_empty: bool,
}

impl XmlWriter {
    pub fn new() -> Self {
        XmlWriter {
            out: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n"),
            stack: Vec::new(),
            open_empty: false,
        }
    }

    fn indent(&mut self) {
        for _ in 0..self.stack.len() {
            self.out.push_str("  ");
        }
    }

    fn close_start(&mut self) {
        if self.open_empty {
            self.out.push_str(">\n");
            self.open_empty = false;
        }
    }

    /// Open an element with attributes; `None` values are left out.
    pub fn open(&mut self, name: &'static str, attrs: &[(&str, Option<String>)]) {
        self.close_start();
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        for (k, v) in attrs {
            if let Some(v) = v {
                self.out.push(' ');
                self.out.push_str(k);
                self.out.push_str("=\"");
                self.out.push_str(&escape(v));
                self.out.push('"');
            }
        }
        self.stack.push(name);
        self.open_empty = true;
    }

    /// An element with attributes and nothing inside.
    pub fn empty(&mut self, name: &'static str, attrs: &[(&str, Option<String>)]) {
        self.open(name, attrs);
        self.close();
    }

    /// An element holding only text.
    pub fn text(&mut self, name: &'static str, text: &str) {
        self.close_start();
        self.indent();
        self.out
            .push_str(&format!("<{name}>{}</{name}>\n", escape(text)));
    }

    pub fn close(&mut self) {
        let name = self.stack.pop().expect("close matches an open");
        if self.open_empty {
            self.out.push_str("/>\n");
            self.open_empty = false;
        } else {
            self.indent();
            self.out.push_str("</");
            self.out.push_str(name);
            self.out.push_str(">\n");
        }
    }

    pub fn finish(mut self) -> String {
        while !self.stack.is_empty() {
            self.close();
        }
        self.out
    }
}

impl Default for XmlWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// `Some(value.to_string())`, for attribute lists.
pub fn a(v: impl ToString) -> Option<String> {
    Some(v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elements_nest_escape_and_close_empty() {
        let mut w = XmlWriter::new();
        w.open("Project", &[("version", a("1.0"))]);
        w.empty(
            "Application",
            &[("name", a("A & B <\"x\">")), ("skip", None)],
        );
        w.text("Title", "Tom's");
        w.close();
        let xml = w.finish();
        assert!(xml.contains("<Application name=\"A &amp; B &lt;&quot;x&quot;&gt;\"/>"));
        assert!(xml.contains("<Title>Tom&apos;s</Title>"));
        assert!(xml.trim_end().ends_with("</Project>"));
        assert!(!xml.contains("skip"));
    }

    #[test]
    fn numbers_read_back_exactly() {
        for v in [0.0, 1.0, 0.5, 113.5, 1.0 / 960.0, 12345.678, 1e-9] {
            assert_eq!(num(v).parse::<f64>().unwrap(), v);
        }
        assert_eq!(num(4.0), "4.0");
        assert_eq!(num(f64::NAN), "0.0");
    }
}
