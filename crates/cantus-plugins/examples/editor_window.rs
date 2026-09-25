//! Porta's real editor in a real CLAP plug-in window: this program is a small host. It
//! makes a top-level window, loads the plug-in through its `clap_entry` (this library's,
//! or a bundled `.clap` given on the command line), asks the plug-in's GUI extension for
//! an embedded editor (`create("win32")`, `get_size`, `set_parent(HWND)`, `show`), feeds
//! it a snare-and-bleed signal block by block while pumping the window's messages, and
//! captures the window into a PNG.
//!
//! ```text
//! cargo run -p cantus-plugins --example editor_window -- target/shots/porta-clap.png 3
//! cargo run -p cantus-plugins --example editor_window -- out.png 3 "target/bundled/Cantus Porta.clap"
//! ```
//!
//! Windows only (the editor's first platform, ADR 0020).

#[path = "../tests/common/mod.rs"]
mod common;

#[cfg(windows)]
fn main() {
    win::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("the plug-in window example runs on Windows");
}

#[cfg(windows)]
mod win {
    use super::common::*;
    use cantus_core::EffectKind;
    use clap_sys::entry::clap_plugin_entry;
    use clap_sys::ext::gui::*;
    use std::ffi::c_void;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::Storage::Xps::PrintWindow;
    /// `PW_RENDERFULLCONTENT`: capture what the compositor shows, OpenGL children included.
    const PW_RENDERFULLCONTENT: u32 = 2;
    /// `PW_CLIENTONLY`: the client area, without the frame.
    const PW_CLIENTONLY: u32 = 1;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn load(path: &str) -> &'static clap_plugin_entry {
        unsafe {
            let lib = Box::leak(Box::new(
                libloading::Library::new(path).expect("load the plug-in"),
            ));
            let sym: libloading::Symbol<*const clap_plugin_entry> =
                lib.get(b"clap_entry\0").expect("clap_entry");
            &**sym
        }
    }

    /// A stored (uncompressed) RGBA PNG, as `cantus_ui::raster` writes them.
    fn write_png(path: &str, w: usize, h: usize, rgba: &[u8]) {
        fn crc(data: &[u8]) -> u32 {
            let mut c = 0xFFFF_FFFFu32;
            for b in data {
                c ^= *b as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 {
                        0xEDB8_8320 ^ (c >> 1)
                    } else {
                        c >> 1
                    };
                }
            }
            !c
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8], body: &[u8]) {
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            let mut all = kind.to_vec();
            all.extend_from_slice(body);
            out.extend_from_slice(&all);
            out.extend_from_slice(&crc(&all).to_be_bytes());
        }
        let mut raw = Vec::with_capacity((w * 4 + 1) * h);
        for y in 0..h {
            raw.push(0);
            raw.extend_from_slice(&rgba[y * w * 4..(y + 1) * w * 4]);
        }
        let mut z = vec![0x78, 0x01];
        let mut parts = raw.chunks(65_535).peekable();
        while let Some(c) = parts.next() {
            z.push(u8::from(parts.peek().is_none()));
            z.extend_from_slice(&(c.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
            z.extend_from_slice(c);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for x in &raw {
            a = (a + *x as u32) % 65_521;
            b = (b + a) % 65_521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = (w as u32).to_be_bytes().to_vec();
        ihdr.extend_from_slice(&(h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        if let Some(dir) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path, out).expect("write the capture");
    }

    /// The window's client area, as the screen would show it.
    unsafe fn capture(hwnd: HWND, w: i32, h: i32) -> Vec<u8> {
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(dc, bmp);
        PrintWindow(hwnd, dc, PW_CLIENTONLY | PW_RENDERFULLCONTENT);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // top-down
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        GetDIBits(
            dc,
            bmp,
            0,
            h as u32,
            bgra.as_mut_ptr() as *mut c_void,
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        for px in bgra.chunks_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        bgra
    }

    pub fn main() {
        let mut args = std::env::args().skip(1);
        let out = args
            .next()
            .unwrap_or_else(|| "target/shots/porta-clap.png".into());
        let seconds: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(3.0);
        let entry = match args.next() {
            Some(path) => load(&path),
            None => &cantus_plugins::clap::clap_entry,
        };
        let plug = Plug::new_from(entry, EffectKind::Gate);
        plug.flush(
            &[
                ("thr", -30.0f32),
                ("hyst", 6.0),
                ("hold", 40.0),
                ("look", 2.0),
                ("hpf", 150.0),
            ]
            .iter()
            .map(|(k, v)| {
                let (id, value) = clap_value(EffectKind::Gate, k, *v);
                (0, id, value)
            })
            .collect::<Vec<Ev>>(),
        );
        plug.activate(480);
        unsafe {
            let gui = (*plug.plugin).get_extension.unwrap()(plug.plugin, CLAP_EXT_GUI.as_ptr())
                as *const clap_plugin_gui;
            assert!(!gui.is_null(), "Porta offers an editor");
            let gui = &*gui;
            assert!(gui.is_api_supported.unwrap()(
                plug.plugin,
                CLAP_WINDOW_API_WIN32.as_ptr(),
                false
            ));
            assert!(gui.create.unwrap()(
                plug.plugin,
                CLAP_WINDOW_API_WIN32.as_ptr(),
                false
            ));
            let (mut w, mut h) = (0u32, 0u32);
            assert!(gui.get_size.unwrap()(plug.plugin, &mut w, &mut h));
            println!("editor size: {w} × {h} physical pixels");

            // The host's window, sized to hold the editor.
            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide("CantusTestHost");
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(DefWindowProcW),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: std::ptr::null_mut(),
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class.as_ptr(),
            };
            RegisterClassW(&wc);
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: w as i32,
                bottom: h as i32,
            };
            AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, 0);
            let title = wide("A CLAP host — Cantus Porta");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                40,
                40,
                rect.right - rect.left,
                rect.bottom - rect.top,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null(), "the host window");
            let window = clap_window {
                api: CLAP_WINDOW_API_WIN32.as_ptr(),
                specific: clap_window_handle { win32: hwnd },
            };
            assert!(
                gui.set_parent.unwrap()(plug.plugin, &window),
                "the editor embeds"
            );
            assert!(gui.show.unwrap()(plug.plugin));

            // Play: a block every 10 ms, the window's messages in between.
            let x = cantus_ui::views::fx::gate::demo_signal(4.0, SR);
            let none = EventList::new(&[]);
            let start = Instant::now();
            let mut block = 0usize;
            let mut msg: MSG = std::mem::zeroed();
            while start.elapsed() < Duration::from_secs_f64(seconds) {
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let due = (start.elapsed().as_secs_f64() * 100.0) as usize;
                while block < due {
                    let at = (block * 480) % (x.len() - 480);
                    let mut l = x[at..at + 480].to_vec();
                    let mut r = l.clone();
                    plug.process_list(&mut l, &mut r, None, &none);
                    block += 1;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            let pixels = capture(hwnd, w as i32, h as i32);
            write_png(&out, w as usize, h as usize, &pixels);
            let distinct: std::collections::HashSet<[u8; 4]> =
                pixels.chunks(4).map(|p| [p[0], p[1], p[2], p[3]]).collect();
            println!(
                "{} blocks played; captured {w} × {h} to {out}; {} distinct colours",
                block,
                distinct.len()
            );
            gui.destroy.unwrap()(plug.plugin);
            DestroyWindow(hwnd);
        }
        plug.deactivate();
    }
}
