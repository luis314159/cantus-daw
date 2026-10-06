//! Release notes: GitHub's Markdown, read into lines the app can draw with its own text
//! kit — headings, bullets, paragraphs — with links reduced to their text and emphasis
//! marks dropped. No HTML is ever rendered and nothing in the notes is fetched.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LineKind {
    Heading,
    Bullet,
    Text,
    /// A paragraph break.
    Gap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteLine {
    pub kind: LineKind,
    pub text: String,
}

/// `[text](url)` → `text`, `![alt](src)` → `alt`, `**b**`/`__b__`/`*i*`/`` `c` `` → their
/// text, `\*` → `*`.
pub fn inline(md: &str) -> String {
    let chars: Vec<char> = md.chars().collect();
    let mut out = String::with_capacity(md.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() => {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            '!' if chars.get(i + 1) == Some(&'[') => {
                i += 1;
                continue;
            }
            '[' => {
                // A link: keep the text, drop the target.
                if let Some(close) = chars[i + 1..].iter().position(|&c| c == ']') {
                    let close = i + 1 + close;
                    if chars.get(close + 1) == Some(&'(') {
                        if let Some(end) = chars[close + 2..].iter().position(|&c| c == ')') {
                            out.extend(&chars[i + 1..close]);
                            i = close + 2 + end + 1;
                            continue;
                        }
                    }
                }
                out.push(c);
            }
            '*' | '`' => {}
            '_' if chars.get(i + 1) == Some(&'_') => {
                i += 2;
                continue;
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Read release notes into lines, at most `max` of them (the rest is on the release page).
pub fn read(md: &str, max: usize) -> Vec<NoteLine> {
    let mut lines: Vec<NoteLine> = Vec::new();
    let mut para = String::new();
    let flush = |para: &mut String, lines: &mut Vec<NoteLine>| {
        if !para.trim().is_empty() {
            lines.push(NoteLine {
                kind: LineKind::Text,
                text: inline(para),
            });
        }
        para.clear();
    };
    let mut in_code = false;
    for raw in md.lines() {
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            flush(&mut para, &mut lines);
            lines.push(NoteLine {
                kind: LineKind::Text,
                text: trimmed.to_owned(),
            });
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut para, &mut lines);
            if lines.last().is_some_and(|l| l.kind != LineKind::Gap) {
                lines.push(NoteLine {
                    kind: LineKind::Gap,
                    text: String::new(),
                });
            }
            continue;
        }
        if trimmed.starts_with('#') {
            flush(&mut para, &mut lines);
            lines.push(NoteLine {
                kind: LineKind::Heading,
                text: inline(trimmed.trim_start_matches('#')),
            });
            continue;
        }
        let bullet = ["* ", "- ", "+ "]
            .iter()
            .find_map(|m| trimmed.strip_prefix(m))
            .or_else(|| {
                // `1. text`
                let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
                (digits > 0)
                    .then(|| trimmed[digits..].strip_prefix(". "))
                    .flatten()
            });
        if let Some(item) = bullet {
            flush(&mut para, &mut lines);
            lines.push(NoteLine {
                kind: LineKind::Bullet,
                text: inline(item),
            });
            continue;
        }
        if trimmed.starts_with('>') {
            para.push_str(trimmed.trim_start_matches('>'));
            para.push(' ');
            continue;
        }
        // A wrapped bullet continues on an indented line.
        if line.starts_with("  ") && lines.last().is_some_and(|l| l.kind == LineKind::Bullet) && para.is_empty() {
            let last = lines.last_mut().expect("checked");
            last.text.push(' ');
            last.text.push_str(&inline(trimmed));
            continue;
        }
        para.push_str(trimmed);
        para.push(' ');
    }
    flush(&mut para, &mut lines);
    while lines.last().is_some_and(|l| l.kind == LineKind::Gap) {
        lines.pop();
    }
    lines.truncate(max);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_emphasis_keep_only_their_text() {
        assert_eq!(
            inline("[PERF #3293](https://example.com/3293): **faster** `scan` *now*"),
            "PERF #3293: faster scan now"
        );
        assert_eq!(inline("a \\* b, __strong__, ![logo](x.png)"), "a * b, strong, logo");
        assert_eq!(inline("[not a link] (x)"), "[not a link] (x)");
    }

    #[test]
    fn notes_become_headings_bullets_and_paragraphs() {
        let md = "## What is new\r\n\r\nThe mixer\r\nis faster.\r\n\r\n* one\r\n  wrapped\r\n- two\r\n1. three\r\n\r\n```\r\ncode\r\n```\r\n";
        let lines = read(md, 100);
        let kinds: Vec<LineKind> = lines.iter().map(|l| l.kind).collect();
        use LineKind::*;
        assert_eq!(kinds, [Heading, Gap, Text, Gap, Bullet, Bullet, Bullet, Gap, Text]);
        assert_eq!(lines[0].text, "What is new");
        assert_eq!(lines[2].text, "The mixer is faster.");
        assert_eq!(lines[4].text, "one wrapped");
        assert_eq!(lines[6].text, "three");
        assert_eq!(lines[8].text, "code");
        assert_eq!(read(md, 2).len(), 2);
    }
}
