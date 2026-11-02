//! The containers read back with our own box reader (no FFmpeg, every system): the MP4's
//! index in front and every table right, each sample's bytes where the index says; the MOV
//! and the PNG sequence bit-exact; the export's refusals, stops and file placing.

use cantus_video::audio::{to_i16, AudioCodec, Pcm};
use cantus_video::bmff::{self, Box as Bx};
use cantus_video::mp4::{AudioParams, Mp4Writer};
use cantus_video::{export, Container, Control, Quality, Request, Timing, VideoError};
use std::io::Cursor;
use tiny_skia::{Color, Pixmap};

/// Every sample of a track, by its `stsc`, chunk offsets and `stsz`.
fn samples(file: &[u8], trak: &Bx) -> Vec<Vec<u8>> {
    let stbl = trak.find("mdia/minf/stbl").unwrap();
    let stsz = stbl.child(b"stsz").unwrap();
    let (uniform, count) = (stsz.u32_at(4), stsz.u32_at(8));
    let size = |i: usize| {
        if uniform != 0 {
            uniform as usize
        } else {
            stsz.u32_at(12 + 4 * i) as usize
        }
    };
    let (offsets, wide) = match stbl.child(b"stco") {
        Some(b) => (b, false),
        None => (stbl.child(b"co64").unwrap(), true),
    };
    let chunks = offsets.u32_at(4) as usize;
    let at = |c: usize| {
        if wide {
            offsets.u64_at(8 + 8 * c) as usize
        } else {
            offsets.u32_at(8 + 4 * c) as usize
        }
    };
    let stsc = stbl.child(b"stsc").unwrap();
    let runs: Vec<(usize, usize)> = (0..stsc.u32_at(4) as usize)
        .map(|r| {
            (
                stsc.u32_at(8 + 12 * r) as usize,
                stsc.u32_at(12 + 12 * r) as usize,
            )
        })
        .collect();
    let mut out = Vec::new();
    for c in 0..chunks {
        let per = runs.iter().rev().find(|r| r.0 <= c + 1).unwrap().1;
        let mut pos = at(c);
        for _ in 0..per {
            let n = size(out.len());
            out.push(file[pos..pos + n].to_vec());
            pos += n;
        }
    }
    assert_eq!(out.len(), count as usize);
    out
}

fn traks<'a>(file: &'a [u8]) -> Vec<Bx<'a>> {
    let moov = bmff::top(file, b"moov").unwrap();
    moov.children()
        .into_iter()
        .filter(|b| &b.kind == b"trak")
        .collect()
}

fn annexb(nals: &[&[u8]]) -> Vec<u8> {
    nals.iter()
        .flat_map(|n| [&[0, 0, 0, 1][..], n].concat())
        .collect()
}

#[test]
fn the_mp4_index_is_in_front_and_every_table_is_right() {
    let t = Timing::new(30, 48_000, 25).unwrap();
    let params = AudioParams {
        codec: AudioCodec::Aac {
            config: vec![0x11, 0x90],
        },
        rate: 48_000,
        channels: 2,
        frame_len: 1_024,
        delay: 1_024,
    };
    let mut w = Mp4Writer::new(Cursor::new(Vec::new()), 64, 48, t, params).unwrap();
    let (sps, pps): (&[u8], &[u8]) = (&[0x67, 100, 0, 30, 0xac], &[0x68, 0xce]);
    let mut frames = Vec::new();
    let mut packets = Vec::new();
    for n in 0..25u8 {
        let slice = [if n % 10 == 0 { 0x65 } else { 0x41 }, n, n, 7];
        let au = if n == 0 {
            annexb(&[&[0x09, 0xf0], sps, pps, &slice])
        } else {
            annexb(&[&slice])
        };
        w.video(&au).unwrap();
        frames.push([&[0, 0, 0, 4][..], &slice].concat());
        // Audio packets of uneven sizes, about two a frame.
        for k in 0..(1 + n % 2) {
            let p = vec![n ^ k; 10 + n as usize + k as usize];
            w.audio(&p).unwrap();
            packets.push(p);
        }
    }
    let file = w.finish().unwrap().into_inner();
    let kinds: Vec<[u8; 4]> = bmff::parse(&file).iter().map(|b| b.kind).collect();
    assert_eq!(&kinds[..2], &[*b"ftyp", *b"moov"]);
    assert!(kinds.iter().position(|k| k == b"mdat") > Some(1));
    let moov = bmff::top(&file, b"moov").unwrap();
    let mvhd = moov.child(b"mvhd").unwrap();
    assert_eq!((mvhd.u32_at(12), mvhd.u32_at(16)), (30_000, 25_000));
    let tk = traks(&file);
    assert_eq!(tk.len(), 2);
    // Video: every frame 1 000 ticks of 30 000, key frames 1, 11, 21, the parameter sets in
    // avcC, BT.709 limited range, each sample where the index says.
    let v = &tk[0];
    let mdhd = v.find("mdia/mdhd").unwrap();
    assert_eq!((mdhd.u32_at(12), mdhd.u32_at(16)), (30_000, 25_000));
    let stbl = v.find("mdia/minf/stbl").unwrap();
    assert_eq!(
        &stbl.child(b"stts").unwrap().body[4..],
        &bmff::u32s(&[1, 25, 1_000])[..]
    );
    assert_eq!(
        &stbl.child(b"stss").unwrap().body[4..],
        &bmff::u32s(&[3, 1, 11, 21])[..]
    );
    assert!(stbl.child(b"ctts").is_none() && v.child(b"edts").is_none());
    let stsd = stbl.child(b"stsd").unwrap();
    let avc1 = &stsd.body[8..];
    assert_eq!(&avc1[4..8], b"avc1");
    let entry = bmff::parse(&avc1[8 + 78..avc1.len()]);
    let avcc = entry.iter().find(|b| &b.kind == b"avcC").unwrap();
    assert_eq!(avcc.body, cantus_video::h264::avcc(sps, pps).as_slice());
    let colr = entry.iter().find(|b| &b.kind == b"colr").unwrap();
    assert_eq!(colr.body, b"nclx\x00\x01\x00\x01\x00\x01\x00");
    assert_eq!(samples(&file, v), frames);
    // Audio: the edit list skips the priming and lasts exactly the video.
    let a = &tk[1];
    let elst = a.find("edts/elst").unwrap();
    assert_eq!(
        &elst.body[4..],
        &bmff::u32s(&[1, 25_000, 1_024, 0x1_0000])[..]
    );
    let amdhd = a.find("mdia/mdhd").unwrap();
    assert_eq!(amdhd.u32_at(12), 48_000);
    assert_eq!(amdhd.u32_at(16), packets.len() as u32 * 1_024);
    assert_eq!(samples(&file, a), packets);
    let astsd = a.find("mdia/minf/stbl/stsd").unwrap();
    let esds_at = astsd.body.windows(4).position(|w| w == b"esds").unwrap();
    let esds = &astsd.body[esds_at..];
    assert!(
        esds.windows(4).any(|w| w == [0x05, 2, 0x11, 0x90]),
        "the AudioSpecificConfig"
    );
}

#[test]
fn an_mp4_missing_frames_or_parameter_sets_is_refused() {
    let t = Timing::new(30, 48_000, 3).unwrap();
    let params = AudioParams {
        codec: AudioCodec::Mp3,
        rate: 48_000,
        channels: 2,
        frame_len: 1_152,
        delay: 1_105,
    };
    let mut w = Mp4Writer::new(Cursor::new(Vec::new()), 64, 48, t, params.clone()).unwrap();
    w.video(&annexb(&[&[0x67, 66, 0, 30], &[0x68, 1], &[0x65, 1]]))
        .unwrap();
    let e = w.finish().err().unwrap().to_string();
    assert!(e.contains("1 frames came out of 3"), "{e}");
    let mut w = Mp4Writer::new(Cursor::new(Vec::new()), 64, 48, t, params).unwrap();
    for _ in 0..3 {
        w.video(&annexb(&[&[0x65, 1]])).unwrap();
    }
    assert!(w.finish().is_err());
}

fn tiny_source(n: u32, pm: &mut Pixmap) {
    // A different, partly transparent picture each frame.
    let c = Color::from_rgba8((n * 37 % 256) as u8, 90, 200, (60 + n * 13 % 190) as u8);
    let r = tiny_skia::Rect::from_xywh(4.0 + n as f32, 3.0, 30.0, 20.0).unwrap();
    let mut p = tiny_skia::Paint::default();
    p.set_color(c);
    pm.fill_rect(r, &p, tiny_skia::Transform::identity(), None);
}

fn tiny_audio(frames: u32) -> Pcm {
    let len = (48_000 / 30 * frames) as usize - 300; // shorter than the video: silence after
    Pcm {
        rate: 48_000,
        channels: 2,
        samples: (0..len * 2)
            .map(|i| ((i % 97) as f32 / 97.0) - 0.5)
            .collect(),
    }
}

fn request<'a>(container: Container, frames: u32, audio: &'a Pcm) -> Request<'a> {
    Request {
        container,
        width: 64,
        height: 48,
        fps: 30,
        frames,
        quality: Quality::High,
        alpha: true,
        audio,
        workers: 3,
        replace: false,
        plan: None,
    }
}

fn straight(n: u32) -> Vec<u8> {
    let mut pm = Pixmap::new(64, 48).unwrap();
    tiny_source(n, &mut pm);
    let mut v = pm.data().to_vec();
    cantus_video::yuv::demultiply(&mut v);
    v
}

fn decode_png(bytes: &[u8]) -> Vec<u8> {
    let mut d = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    let mut buf = vec![0; d.output_buffer_size().unwrap()];
    d.next_frame(&mut buf).unwrap();
    buf
}

fn expected_pcm(audio: &Pcm, frames: u32) -> Vec<u8> {
    let mut f = Vec::new();
    audio.stereo(0..(48_000 / 30 * frames) as u64, &mut f);
    let mut i = Vec::new();
    to_i16(&f, &mut i);
    i.iter().flat_map(|s| s.to_le_bytes()).collect()
}

#[test]
fn a_png_mov_reads_back_bit_exact_with_its_audio() {
    let dir = tempfile::tempdir().unwrap();
    let audio = tiny_audio(12);
    let path = dir.path().join("layer.mov");
    let done = export(
        &request(Container::MovPng, 12, &audio),
        &tiny_source,
        &path,
        &Control::new(),
    )
    .unwrap();
    assert_eq!(done.frames, 12);
    assert!(!dir.path().join("layer.mov.part").exists());
    let file = std::fs::read(&path).unwrap();
    let tk = traks(&file);
    let pngs = samples(&file, &tk[0]);
    assert_eq!(pngs.len(), 12);
    for (n, p) in pngs.iter().enumerate() {
        assert_eq!(decode_png(p), straight(n as u32), "frame {n}");
    }
    let pcm: Vec<u8> = samples(&file, &tk[1]).concat();
    assert_eq!(pcm, expected_pcm(&audio, 12));
    let stsd = tk[0].find("mdia/minf/stbl/stsd").unwrap();
    assert_eq!(&stsd.body[12..16], b"png ");
}

#[test]
fn a_png_sequence_is_numbered_frames_and_the_song() {
    let dir = tempfile::tempdir().unwrap();
    let audio = tiny_audio(7);
    let path = dir.path().join("Dance");
    let req = request(Container::PngSequence, 7, &audio);
    export(&req, &tiny_source, &path, &Control::new()).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 8);
    assert_eq!(names[0], "Dance.wav");
    assert_eq!(names[1], "Dance_00000.png");
    assert_eq!(names[7], "Dance_00006.png");
    let f3 = std::fs::read(path.join("Dance_00003.png")).unwrap();
    assert_eq!(decode_png(&f3), straight(3));
    let wav = std::fs::read(path.join("Dance.wav")).unwrap();
    assert_eq!(&wav[44..], expected_pcm(&audio, 7).as_slice());
    // A second export of the same name is refused; nothing half-made is left.
    let e = export(&req, &tiny_source, &path, &Control::new()).unwrap_err();
    assert!(matches!(e, VideoError::Io(_)), "{e}");
    assert!(!dir.path().join("Dance.part").exists());
}

#[test]
fn what_the_export_refuses_it_says() {
    let audio = tiny_audio(3);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.mov");
    let ctl = Control::new();
    let mut r = request(Container::MovPng, 3, &audio);
    r.width = 63;
    let e = export(&r, &tiny_source, &p, &ctl).unwrap_err().to_string();
    assert!(e.contains("63 × 48"), "{e}");
    let mut r = request(Container::MovPng, 0, &audio);
    assert!(export(&r, &tiny_source, &p, &ctl).is_err());
    r.frames = 3;
    r.fps = 29;
    assert!(export(&r, &tiny_source, &p, &ctl)
        .unwrap_err()
        .to_string()
        .contains("29"));
    let loud = Pcm::silence(96_000, 2, 10);
    let r = request(Container::MovPng, 3, &loud);
    assert!(export(&r, &tiny_source, &p, &ctl)
        .unwrap_err()
        .to_string()
        .contains("96000"));
}

#[test]
fn a_file_there_is_kept_unless_replacing_is_asked() {
    let audio = tiny_audio(3);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.mov");
    std::fs::write(&p, b"mine").unwrap();
    let mut r = request(Container::MovPng, 3, &audio);
    assert!(export(&r, &tiny_source, &p, &Control::new()).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), b"mine");
    r.replace = true;
    export(&r, &tiny_source, &p, &Control::new()).unwrap();
    assert_ne!(std::fs::read(&p).unwrap(), b"mine");
}

#[test]
fn a_stop_leaves_nothing_behind() {
    let audio = tiny_audio(200);
    let dir = tempfile::tempdir().unwrap();
    for container in [Container::MovPng, Container::PngSequence] {
        let p = dir.path().join("stopped");
        let ctl = Control::new();
        let source = |n: u32, pm: &mut Pixmap| {
            if n == 40 {
                ctl.cancel();
            }
            tiny_source(n, pm);
        };
        let e = export(&request(container, 200, &audio), &source, &p, &ctl).unwrap_err();
        assert!(matches!(e, VideoError::Cancelled), "{e}");
        let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert!(left.is_empty(), "{container:?} left {left:?}");
        assert!(ctl.frames().0 < 200);
    }
}

/// The work in progress has a name of our own: a user's `x.mov.part` file and `Mix.part`
/// folder are never opened, replaced or removed (review 1 #7).
#[test]
fn what_the_user_keeps_beside_the_target_is_never_touched() {
    let audio = tiny_audio(4);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("x.mov.part"), b"mine").unwrap();
    std::fs::create_dir(dir.path().join("Mix.part")).unwrap();
    std::fs::write(dir.path().join("Mix.part").join("keep.txt"), b"mine too").unwrap();
    let ctl = Control::new();
    export(
        &request(Container::MovPng, 4, &audio),
        &tiny_source,
        &dir.path().join("x.mov"),
        &ctl,
    )
    .unwrap();
    let r = request(Container::PngSequence, 4, &audio);
    export(&r, &tiny_source, &dir.path().join("Mix"), &ctl).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("x.mov.part")).unwrap(),
        b"mine"
    );
    assert_eq!(
        std::fs::read(dir.path().join("Mix.part").join("keep.txt")).unwrap(),
        b"mine too"
    );
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Mix", "Mix.part", "x.mov", "x.mov.part"]);
}

/// Parts an ended Cantus left beside the target (a quit or a crash mid-export) are swept
/// at the next export's start; a running one's, another target's and the user's stay
/// (review 2 #2).
#[test]
fn the_parts_an_ended_export_left_are_swept_and_only_those() {
    let audio = tiny_audio(4);
    let dir = tempfile::tempdir().unwrap();
    let dead = 4_294_967_280u32; // no process has this id
    let me = std::process::id();
    let stale = dir.path().join(format!(".x.mov.cantus-part-{dead}-3"));
    std::fs::write(&stale, vec![0u8; 1000]).unwrap();
    let stale_dir = dir.path().join(format!(".x.mov.cantus-part-{dead}-4"));
    std::fs::create_dir(&stale_dir).unwrap();
    std::fs::write(stale_dir.join("frame.png"), b"x").unwrap();
    let running = dir.path().join(format!(".x.mov.cantus-part-{me}-999999"));
    std::fs::write(&running, b"live").unwrap();
    let other = dir.path().join(format!(".y.mov.cantus-part-{dead}-0"));
    std::fs::write(&other, b"another target").unwrap();
    let odd = dir.path().join(".x.mov.cantus-part-notapid-0");
    std::fs::write(&odd, b"not ours").unwrap();
    export(
        &request(Container::MovPng, 4, &audio),
        &tiny_source,
        &dir.path().join("x.mov"),
        &Control::new(),
    )
    .unwrap();
    assert!(
        !stale.exists() && !stale_dir.exists(),
        "an ended process's parts go"
    );
    assert!(running.exists(), "a running export's stays");
    assert!(other.exists() && odd.exists(), "nothing else is touched");
    assert!(!cantus_video::export::process_alive(dead));
    assert!(cantus_video::export::process_alive(me));
}
