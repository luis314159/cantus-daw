//! Fixtures: packs made on the spot in a temporary folder, served from `file://` or from a
//! tiny HTTP server on the loopback interface. No test here touches the network.

#![allow(dead_code)]

use cantus_content::fetch::{file_url, hex};
use cantus_content::manifest::{
    Attribution, Category, Credit, FileEntry, Manifest, Pack, Source, Text, ZipSource,
};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;

pub fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// A few bytes that start like a WAV file.
pub fn wav(seed: u8, len: usize) -> Vec<u8> {
    let mut v = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
    v.extend((0..len).map(|i| (i as u8).wrapping_mul(seed).wrapping_add(seed)));
    v
}

pub fn pack(id: &str, licence_file: &str, source: Source) -> Pack {
    Pack {
        id: id.into(),
        version: 1,
        name: Text {
            en: format!("Pack {id}"),
            es: format!("Paquete {id}"),
        },
        description: Text {
            en: "A test pack".into(),
            es: "Un paquete de prueba".into(),
        },
        category: Category::Drums,
        recommended: false,
        licence: "CC0-1.0".into(),
        licence_url: "https://creativecommons.org/publicdomain/zero/1.0/legalcode".into(),
        licence_file: licence_file.into(),
        credits: vec![Credit {
            name: "Tests".into(),
            url: "https://example.org".into(),
            licence_url: "https://example.org/LICENSE".into(),
        }],
        checked: "2026-10-05".into(),
        installed_size: 0,
        source,
        attribution: None,
        instruments: vec![],
        build: None,
    }
}

/// `p` made a CC BY pack (`version` "3.0" or "4.0") with a credit line of its own.
pub fn cc_by(mut p: Pack, version: &str) -> Pack {
    p.licence = format!("CC-BY-{version}");
    p.licence_url = format!("https://creativecommons.org/licenses/by/{version}/legalcode");
    p.attribution = Some(Attribution {
        title: format!("Upright {}", p.id),
        author: "Ana Example".into(),
        url: format!("https://example.org/{}", p.id),
        line: String::new(),
        note: String::new(),
    });
    p
}

/// Write `files` (`(to, bytes)`) under `src` and return a files-pack over them.
pub fn files_pack(id: &str, src: &Path, files: &[(&str, Vec<u8>)]) -> Pack {
    let mut list = Vec::new();
    for (i, (to, bytes)) in files.iter().enumerate() {
        let p = src.join(format!("{id}-{i}.bin"));
        std::fs::write(&p, bytes).unwrap();
        list.push(FileEntry {
            url: file_url(&p),
            to: (*to).into(),
            size: bytes.len() as u64,
            sha256: sha(bytes),
        });
    }
    let licence = files
        .iter()
        .find(|(to, _)| to.contains("LICENSE"))
        .map(|f| f.0)
        .unwrap_or("LICENSE");
    pack(id, licence, Source::Files(list))
}

/// Build a zip (stored, no compression) with `entries`.
pub fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in entries {
        w.start_file(*name, opts).unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap().into_inner()
}

pub fn zip_pack(
    id: &str,
    url: String,
    bytes: &[u8],
    strip: &str,
    include: &[&str],
    licence: &str,
) -> Pack {
    pack(
        id,
        licence,
        Source::Zip(ZipSource {
            url,
            size: bytes.len() as u64,
            sha256: sha(bytes),
            strip: strip.into(),
            include: include.iter().map(|s| s.to_string()).collect(),
            exclude: vec![],
        }),
    )
}

pub fn manifest(packs: Vec<Pack>) -> Manifest {
    Manifest {
        format: cantus_content::manifest::FORMAT,
        updated: "2026-10-05".into(),
        packs,
    }
}

/// A loopback HTTP server answering each request with `respond(path, range_start)`, which
/// returns the raw response bytes. Serves until the test ends.
pub fn serve(respond: impl Fn(&str, Option<u64>) -> Vec<u8> + Send + 'static) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut req = Vec::new();
            let mut buf = [0u8; 1024];
            while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => req.extend_from_slice(&buf[..n]),
                }
            }
            let text = String::from_utf8_lossy(&req).to_string();
            let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
            let range = text.lines().find_map(|l| {
                let l = l.to_ascii_lowercase();
                l.strip_prefix("range: bytes=").and_then(|r| {
                    r.trim_end_matches('-')
                        .trim_end_matches(|c: char| !c.is_ascii_digit())
                        .parse()
                        .ok()
                })
            });
            let _ = s.write_all(&respond(&path, range));
            let _ = s.flush();
        }
    });
    format!("http://127.0.0.1:{port}")
}

pub fn response(status: &str, content_type: &str, body: &[u8], declared_len: usize) -> Vec<u8> {
    let mut r = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {declared_len}\r\nConnection: close\r\n\r\n"
    )
    .into_bytes();
    r.extend_from_slice(body);
    r
}

/// One raw tar entry, written exactly as given (so a test can write what no honest tool
/// would: `../` names, absolute paths, links, sizes that lie).
pub struct TarEntry<'a> {
    pub name: &'a str,
    /// `b'0'` file, `b'5'` folder, `b'1'` hard link, `b'2'` symbolic link, `b'L'` GNU
    /// long name, `b'x'` pax header.
    pub kind: u8,
    pub data: &'a [u8],
    /// The size the header claims, when it is not the data's.
    pub claims: Option<u64>,
    pub link: &'a str,
}

pub fn file<'a>(name: &'a str, data: &'a [u8]) -> TarEntry<'a> {
    TarEntry {
        name,
        kind: b'0',
        data,
        claims: None,
        link: "",
    }
}

fn octal(field: &mut [u8], v: u64) {
    let s = format!("{v:0w$o}", w = field.len() - 1);
    field[..s.len()].copy_from_slice(s.as_bytes());
    field[field.len() - 1] = 0;
}

/// A ustar archive of `entries`, ended by two zero blocks.
pub fn tar_bytes(entries: &[TarEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    for e in entries {
        let mut h = [0u8; 512];
        h[..e.name.len()].copy_from_slice(e.name.as_bytes());
        octal(&mut h[100..108], 0o644);
        octal(&mut h[108..116], 0);
        octal(&mut h[116..124], 0);
        let size = e.claims.unwrap_or(e.data.len() as u64);
        if size >= 0o77777777777 {
            h[124] = 0x80;
            h[128..136].copy_from_slice(&size.to_be_bytes());
        } else {
            octal(&mut h[124..136], size);
        }
        octal(&mut h[136..148], 0);
        h[156] = e.kind;
        h[157..157 + e.link.len()].copy_from_slice(e.link.as_bytes());
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        h[148..156].copy_from_slice(b"        ");
        let sum: u32 = h.iter().map(|&b| b as u32).sum();
        let s = format!("{sum:06o}\0 ");
        h[148..156].copy_from_slice(s.as_bytes());
        out.extend_from_slice(&h);
        out.extend_from_slice(e.data);
        out.resize(out.len().div_ceil(512) * 512, 0);
    }
    out.resize(out.len() + 1024, 0);
    out
}

pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(bytes).unwrap();
    e.finish().unwrap()
}

/// A `.tar.gz` pack over `bytes` at `url`.
pub fn tar_pack(
    id: &str,
    url: String,
    bytes: &[u8],
    strip: &str,
    include: &[&str],
    licence: &str,
) -> Pack {
    let mut p = zip_pack(id, url, bytes, strip, include, licence);
    if let Source::Zip(z) = p.source {
        p.source = Source::TarGz(z);
    }
    p
}
