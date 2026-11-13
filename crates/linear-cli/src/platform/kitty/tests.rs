use std::num::NonZeroU32;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;

use super::{
    CellSize, InlineImages, LoadError, Placement, Png, PngError, image_id, place, placeholder_row,
    supported, transfer,
};

fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let length = u32::try_from(body.len()).expect("test chunk fits u32");
    let mut out = length.to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    // The CRC is not checked; the terminal decodes the image.
    out.extend_from_slice(&[0; 4]);
    out
}

fn header(width: u32, height: u32, depth: u8, color: u8) -> Vec<u8> {
    let mut body = width.to_be_bytes().to_vec();
    body.extend_from_slice(&height.to_be_bytes());
    body.extend_from_slice(&[depth, color, 0, 0, 0]);
    body
}

/// A PNG made of `chunks` after the signature.
fn file(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
    for chunk in chunks {
        out.extend_from_slice(chunk);
    }
    out
}

/// A structurally valid RGBA PNG; `extra` is carried in an ancillary chunk.
fn png_bytes(width: u32, height: u32, extra: &[u8]) -> Vec<u8> {
    file(&[
        chunk(b"IHDR", &header(width, height, 8, 6)),
        chunk(b"tEXt", extra),
        chunk(b"IDAT", &[0x78, 0x9c, 0x63, 0x00, 0x00]),
        chunk(b"IEND", &[]),
    ])
}

fn png(width: u32, height: u32) -> Png {
    Png::parse(png_bytes(width, height, b"")).expect("valid png")
}

fn cell(width: u32, height: u32) -> CellSize {
    CellSize {
        width: NonZeroU32::new(width).expect("nonzero"),
        height: NonZeroU32::new(height).expect("nonzero"),
    }
}

#[test]
fn a_valid_png_reports_its_size() {
    let image = png(640, 480);
    assert_eq!((image.width.get(), image.height.get()), (640, 480));
}

#[test]
fn other_formats_are_not_png() {
    for bytes in [&b"\xff\xd8\xff\xe0JFIF"[..], b"GIF89a", b"", b"\x89PNG"] {
        assert_eq!(
            Png::parse(bytes.to_vec()),
            Err(PngError::NotPng),
            "{bytes:?}"
        );
    }
}

#[test]
fn malformed_pngs_are_refused() {
    let ihdr = chunk(b"IHDR", &header(4, 4, 8, 6));
    let idat = chunk(b"IDAT", &[1]);
    let iend = chunk(b"IEND", &[]);
    let mut truncated = png_bytes(4, 4, b"");
    truncated.truncate(truncated.len() - 3);
    let mut trailing = png_bytes(4, 4, b"");
    trailing.push(0);
    let cases = [
        ("truncated", truncated),
        ("data after IEND", trailing),
        ("no IEND", file(&[ihdr.clone(), idat.clone()])),
        ("no IDAT", file(&[ihdr.clone(), iend.clone()])),
        (
            "IHDR not first",
            file(&[idat.clone(), ihdr.clone(), iend.clone()]),
        ),
        (
            "two IHDR",
            file(&[ihdr.clone(), ihdr.clone(), idat.clone(), iend.clone()]),
        ),
        (
            "zero width",
            file(&[
                chunk(b"IHDR", &header(0, 4, 8, 6)),
                idat.clone(),
                iend.clone(),
            ]),
        ),
        (
            "depth 4 RGB",
            file(&[
                chunk(b"IHDR", &header(4, 4, 4, 2)),
                idat.clone(),
                iend.clone(),
            ]),
        ),
        (
            "color 5",
            file(&[
                chunk(b"IHDR", &header(4, 4, 8, 5)),
                idat.clone(),
                iend.clone(),
            ]),
        ),
        (
            "short IHDR",
            file(&[chunk(b"IHDR", &[0; 12]), idat.clone(), iend.clone()]),
        ),
    ];
    for (name, bytes) in cases {
        assert!(
            matches!(Png::parse(bytes), Err(PngError::Malformed(_))),
            "{name} was accepted"
        );
    }
}

#[test]
fn huge_pngs_are_too_large() {
    for (width, height) in [(16_385, 1), (1, 16_385), (8_000, 8_000)] {
        assert_eq!(
            Png::parse(png_bytes(width, height, b"")),
            Err(PngError::TooLarge),
            "{width}x{height}"
        );
    }
    assert!(Png::parse(png_bytes(16_384, 2_000, b"")).is_ok());
}

#[test]
fn transfers_are_chunked_quiet_and_end_with_a_virtual_placement() {
    // About 9,000 base64 characters: three chunks.
    let image = Png::parse(png_bytes(8, 4, &[7; 6_700])).expect("valid png");
    let placement = Placement {
        columns: 3,
        rows: 2,
    };
    let sent = transfer(&image, 42, placement);
    let sequences: Vec<&str> = sent
        .split("\x1b\\")
        .filter(|part| !part.is_empty())
        .map(|part| part.strip_prefix("\x1b_G").expect("APC sequence"))
        .collect();
    let (controls, payloads): (Vec<&str>, Vec<&str>) = sequences
        .iter()
        .map(|sequence| sequence.split_once(';').unwrap_or((sequence, "")))
        .unzip();
    assert_eq!(
        controls,
        [
            "a=t,f=100,t=d,i=42,q=2,m=1",
            "q=2,m=1",
            "q=2,m=0",
            "a=p,U=1,i=42,c=3,r=2,q=2"
        ]
    );
    assert!(payloads.iter().all(|payload| payload.len() <= 4096));
    let expected = BASE64_STANDARD.encode(png_bytes(8, 4, &[7; 6_700]));
    assert_eq!(payloads.concat(), expected);
}

#[test]
fn placeholder_rows_name_the_image_row_and_column() {
    assert_eq!(
        placeholder_row(42, 0, 2),
        "\x1b[38;2;0;0;42m\u{10eeee}\u{305}\u{305}\u{10eeee}\u{305}\u{30d}\x1b[39m"
    );
    assert_eq!(
        placeholder_row(0x01_02_03, 2, 1),
        "\x1b[38;2;1;2;3m\u{10eeee}\u{30e}\u{305}\x1b[39m"
    );
}

#[test]
fn placements_keep_the_natural_size_when_it_fits() {
    assert_eq!(
        place(&png(100, 40), cell(10, 20), 80, 30),
        Some(Placement {
            columns: 10,
            rows: 2
        })
    );
}

#[test]
fn placements_shrink_to_fit_keeping_the_aspect_ratio() {
    // Wide: limited by columns.
    assert_eq!(
        place(&png(1000, 100), cell(10, 20), 40, 30),
        Some(Placement {
            columns: 40,
            rows: 2
        })
    );
    // Tall: limited by rows, so narrower too.
    assert_eq!(
        place(&png(100, 2000), cell(10, 20), 80, 10),
        Some(Placement {
            columns: 1,
            rows: 10
        })
    );
    // A wide terminal is still limited to the diacritics table.
    assert_eq!(
        place(&png(16_000, 10), cell(1, 1), 1_000, 1_000).map(|p| p.columns),
        Some(297)
    );
}

#[test]
fn no_room_means_no_placement() {
    assert_eq!(place(&png(10, 10), cell(10, 20), 0, 10), None);
    assert_eq!(place(&png(10, 10), cell(10, 20), 10, 0), None);
}

#[test]
fn cell_size_comes_from_the_window_when_it_reports_pixels() {
    assert_eq!(
        CellSize::from_window((1600, 900), (160, 45)),
        Some(cell(10, 20))
    );
    assert_eq!(CellSize::from_window((0, 0), (160, 45)), None);
    assert_eq!(CellSize::from_window((1600, 900), (0, 0)), None);
}

#[test]
fn image_ids_are_nonzero_and_differ_by_size() {
    let image = png(10, 10);
    let small = image_id(
        &image,
        Placement {
            columns: 1,
            rows: 1,
        },
    );
    let large = image_id(
        &image,
        Placement {
            columns: 2,
            rows: 1,
        },
    );
    assert_ne!(small, 0);
    assert!(small < 1 << 24);
    assert_ne!(small, large);
    assert_eq!(
        small,
        image_id(
            &image,
            Placement {
                columns: 1,
                rows: 1
            }
        )
    );
}

#[test]
fn drawn_images_are_sent_once_per_size() {
    let images = InlineImages::new(cell(10, 20), 10);
    images.insert("/a.png", png(40, 20));
    assert_eq!(images.draw("/missing.png", 80), None);
    assert_eq!(images.transfers(), "");
    let wide = images.draw("/a.png", 80).expect("drawn");
    assert_eq!(wide, images.draw("/a.png", 80).expect("drawn again"));
    assert_eq!(images.transfers().matches("a=t,").count(), 1);
    // Narrower, so another placement of the same image.
    images.draw("/a.png", 2).expect("drawn narrow");
    let sent = images.transfers();
    assert_eq!(sent.matches("a=t,").count(), 2, "{sent:?}");
    assert!(
        sent.contains("c=4,r=1,") && sent.contains("c=2,r=1,"),
        "{sent:?}"
    );
}

#[test]
fn drawing_stops_at_the_byte_limit() {
    let image = png(40, 20);
    let size = image.size();
    let images = InlineImages::with_limit(cell(10, 20), 10, size * 2);
    images.insert("/a.png", image);
    assert!(images.draw("/a.png", 80).is_some());
    assert!(images.draw("/a.png", 3).is_some());
    // A third size would send the file a third time.
    assert_eq!(images.draw("/a.png", 2), None);
    // Sizes already sent still draw.
    assert!(images.draw("/a.png", 80).is_some());
    assert_eq!(images.transfers().matches("a=t,").count(), 2);
}

#[test]
fn loading_reads_pngs_within_the_limit_and_skips_other_files() {
    let dir = std::env::temp_dir().join(format!("linear-kitty-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let write = |name: &str, bytes: &[u8]| {
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write");
        path.display().to_string()
    };
    let small = write("small.png", &png_bytes(4, 4, b""));
    let big = write("big.png", &png_bytes(4, 4, &[0; 200]));
    let gif = write("not.png", b"GIF89a");
    let gone = dir.join("gone.png").display().to_string();
    let images = InlineImages::with_limit(cell(10, 20), 10, 150);
    assert_eq!(images.load(&small).expect("small"), Some(()));
    assert_eq!(images.load(&gif).expect("gif"), None);
    assert!(matches!(
        images.load(&big),
        Err(LoadError::Png(PngError::TooLarge))
    ));
    assert!(matches!(images.load(&gone), Err(LoadError::Read(_))));
    assert!(images.draw(&small, 80).is_some());
    std::fs::remove_dir_all(&dir).expect("clean up");
}

#[test]
fn only_kitty_ghostty_and_rex_outside_multiplexers_are_supported() {
    let cases: [(&[(&str, &str)], bool); 14] = [
        (
            &[("TERM_PROGRAM", "WezTerm"), ("KITTY_WINDOW_ID", "3")],
            false,
        ),
        (
            &[("TERM_PROGRAM", "vscode"), ("TERM", "xterm-kitty")],
            false,
        ),
        (&[("TERM_PROGRAM", "rex"), ("TERM", "xterm-256color")], true),
        (&[("TERM_PROGRAM", "ghostty")], true),
        (&[("TERM", "xterm-ghostty")], true),
        (&[("TERM", "xterm-kitty")], true),
        (
            &[("KITTY_WINDOW_ID", "1"), ("TERM", "xterm-256color")],
            true,
        ),
        (&[("KITTY_WINDOW_ID", "")], false),
        (&[("TERM_PROGRAM", "iTerm.app")], false),
        (&[("TERM_PROGRAM", "Apple_Terminal")], false),
        (&[], false),
        (
            &[
                ("TERM_PROGRAM", "ghostty"),
                ("TMUX", "/tmp/tmux-501/default,1,0"),
            ],
            false,
        ),
        (
            &[("TERM", "screen-256color"), ("KITTY_WINDOW_ID", "1")],
            false,
        ),
        (&[("TERM", "dumb"), ("TERM_PROGRAM", "rex")], false),
    ];
    for (env, expected) in cases {
        let lookup = |name: &str| {
            env.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
        };
        assert_eq!(supported(lookup), expected, "{env:?}");
    }
}
