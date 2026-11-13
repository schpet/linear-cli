//! Inline PNG images through the kitty graphics protocol, for kitty, Ghostty
//! and Rex.
//!
//! Images are drawn with Unicode placeholders: the image is sent to the
//! terminal once, with a virtual placement `c` columns by `r` rows, and the
//! text holds a grid of U+10EEEE cells whose foreground color names the image
//! and whose combining marks give each cell's row and column. Placeholder
//! text survives a pager, so only the transfers themselves (APC escape
//! sequences, which a pager would mangle) must reach the terminal directly.
//! [`InlineImages`] records what a view drew so they can be sent apart from
//! its text.
//!
//! The terminal decodes the PNG; this module only checks its structure and
//! reads its size. Other formats are not shown.
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::num::NonZeroU32;
use std::sync::Mutex;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use sha2::{Digest, Sha256};

/// The placeholder character cells of an image are drawn with.
const PLACEHOLDER: char = '\u{10EEEE}';

/// Combining marks naming placeholder rows and columns: the n-th mark is row
/// or column n. Kitty's `gen/rowcolumn-diacritics.txt`, in order.
#[rustfmt::skip]
const DIACRITICS: [char; 297] = [
    '\u{0305}', '\u{030d}', '\u{030e}', '\u{0310}', '\u{0312}', '\u{033d}', '\u{033e}',
    '\u{033f}', '\u{0346}', '\u{034a}', '\u{034b}', '\u{034c}', '\u{0350}', '\u{0351}',
    '\u{0352}', '\u{0357}', '\u{035b}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}',
    '\u{0367}', '\u{0368}', '\u{0369}', '\u{036a}', '\u{036b}', '\u{036c}', '\u{036d}',
    '\u{036e}', '\u{036f}', '\u{0483}', '\u{0484}', '\u{0485}', '\u{0486}', '\u{0487}',
    '\u{0592}', '\u{0593}', '\u{0594}', '\u{0595}', '\u{0597}', '\u{0598}', '\u{0599}',
    '\u{059c}', '\u{059d}', '\u{059e}', '\u{059f}', '\u{05a0}', '\u{05a1}', '\u{05a8}',
    '\u{05a9}', '\u{05ab}', '\u{05ac}', '\u{05af}', '\u{05c4}', '\u{0610}', '\u{0611}',
    '\u{0612}', '\u{0613}', '\u{0614}', '\u{0615}', '\u{0616}', '\u{0617}', '\u{0657}',
    '\u{0658}', '\u{0659}', '\u{065a}', '\u{065b}', '\u{065d}', '\u{065e}', '\u{06d6}',
    '\u{06d7}', '\u{06d8}', '\u{06d9}', '\u{06da}', '\u{06db}', '\u{06dc}', '\u{06df}',
    '\u{06e0}', '\u{06e1}', '\u{06e2}', '\u{06e4}', '\u{06e7}', '\u{06e8}', '\u{06eb}',
    '\u{06ec}', '\u{0730}', '\u{0732}', '\u{0733}', '\u{0735}', '\u{0736}', '\u{073a}',
    '\u{073d}', '\u{073f}', '\u{0740}', '\u{0741}', '\u{0743}', '\u{0745}', '\u{0747}',
    '\u{0749}', '\u{074a}', '\u{07eb}', '\u{07ec}', '\u{07ed}', '\u{07ee}', '\u{07ef}',
    '\u{07f0}', '\u{07f1}', '\u{07f3}', '\u{0816}', '\u{0817}', '\u{0818}', '\u{0819}',
    '\u{081b}', '\u{081c}', '\u{081d}', '\u{081e}', '\u{081f}', '\u{0820}', '\u{0821}',
    '\u{0822}', '\u{0823}', '\u{0825}', '\u{0826}', '\u{0827}', '\u{0829}', '\u{082a}',
    '\u{082b}', '\u{082c}', '\u{082d}', '\u{0951}', '\u{0953}', '\u{0954}', '\u{0f82}',
    '\u{0f83}', '\u{0f86}', '\u{0f87}', '\u{135d}', '\u{135e}', '\u{135f}', '\u{17dd}',
    '\u{193a}', '\u{1a17}', '\u{1a75}', '\u{1a76}', '\u{1a77}', '\u{1a78}', '\u{1a79}',
    '\u{1a7a}', '\u{1a7b}', '\u{1a7c}', '\u{1b6b}', '\u{1b6d}', '\u{1b6e}', '\u{1b6f}',
    '\u{1b70}', '\u{1b71}', '\u{1b72}', '\u{1b73}', '\u{1cd0}', '\u{1cd1}', '\u{1cd2}',
    '\u{1cda}', '\u{1cdb}', '\u{1ce0}', '\u{1dc0}', '\u{1dc1}', '\u{1dc3}', '\u{1dc4}',
    '\u{1dc5}', '\u{1dc6}', '\u{1dc7}', '\u{1dc8}', '\u{1dc9}', '\u{1dcb}', '\u{1dcc}',
    '\u{1dd1}', '\u{1dd2}', '\u{1dd3}', '\u{1dd4}', '\u{1dd5}', '\u{1dd6}', '\u{1dd7}',
    '\u{1dd8}', '\u{1dd9}', '\u{1dda}', '\u{1ddb}', '\u{1ddc}', '\u{1ddd}', '\u{1dde}',
    '\u{1ddf}', '\u{1de0}', '\u{1de1}', '\u{1de2}', '\u{1de3}', '\u{1de4}', '\u{1de5}',
    '\u{1de6}', '\u{1dfe}', '\u{20d0}', '\u{20d1}', '\u{20d4}', '\u{20d5}', '\u{20d6}',
    '\u{20d7}', '\u{20db}', '\u{20dc}', '\u{20e1}', '\u{20e7}', '\u{20e9}', '\u{20f0}',
    '\u{2cef}', '\u{2cf0}', '\u{2cf1}', '\u{2de0}', '\u{2de1}', '\u{2de2}', '\u{2de3}',
    '\u{2de4}', '\u{2de5}', '\u{2de6}', '\u{2de7}', '\u{2de8}', '\u{2de9}', '\u{2dea}',
    '\u{2deb}', '\u{2dec}', '\u{2ded}', '\u{2dee}', '\u{2def}', '\u{2df0}', '\u{2df1}',
    '\u{2df2}', '\u{2df3}', '\u{2df4}', '\u{2df5}', '\u{2df6}', '\u{2df7}', '\u{2df8}',
    '\u{2df9}', '\u{2dfa}', '\u{2dfb}', '\u{2dfc}', '\u{2dfd}', '\u{2dfe}', '\u{2dff}',
    '\u{a66f}', '\u{a67c}', '\u{a67d}', '\u{a6f0}', '\u{a6f1}', '\u{a8e0}', '\u{a8e1}',
    '\u{a8e2}', '\u{a8e3}', '\u{a8e4}', '\u{a8e5}', '\u{a8e6}', '\u{a8e7}', '\u{a8e8}',
    '\u{a8e9}', '\u{a8ea}', '\u{a8eb}', '\u{a8ec}', '\u{a8ed}', '\u{a8ee}', '\u{a8ef}',
    '\u{a8f0}', '\u{a8f1}', '\u{aab0}', '\u{aab2}', '\u{aab3}', '\u{aab7}', '\u{aab8}',
    '\u{aabe}', '\u{aabf}', '\u{aac1}', '\u{fe20}', '\u{fe21}', '\u{fe22}', '\u{fe23}',
    '\u{fe24}', '\u{fe25}', '\u{fe26}', '\u{10a0f}', '\u{10a38}', '\u{1d185}', '\u{1d186}',
    '\u{1d187}', '\u{1d188}', '\u{1d189}', '\u{1d1aa}', '\u{1d1ab}', '\u{1d1ac}', '\u{1d1ad}',
    '\u{1d242}', '\u{1d243}', '\u{1d244}',
];

/// Base64 characters per transfer chunk, the protocol's maximum.
const CHUNK: usize = 4096;

/// Largest PNG file read for display.
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
/// Largest total of PNG bytes one command reads, and sends, for display.
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const MAX_SIDE: u32 = 16_384;
const MAX_PIXELS: u64 = 40_000_000;

const APC_START: &str = "\x1b_G";
const APC_END: &str = "\x1b\\";

/// Whether the terminal, as the process environment describes it, is one of
/// kitty, Ghostty or Rex, not behind a multiplexer. Multiplexers need the
/// sequences wrapped for passthrough, which is not done.
pub fn supported<'a>(env: impl Fn(&str) -> Option<&'a str>) -> bool {
    let set = |name: &str| env(name).is_some_and(|value| !value.is_empty());
    let term = env("TERM").unwrap_or_default();
    if set("TMUX") || set("STY") || term == "dumb" {
        return false;
    }
    if term.starts_with("screen") || term.starts_with("tmux") {
        return false;
    }
    match env("TERM_PROGRAM").filter(|program| !program.is_empty()) {
        Some("ghostty" | "rex" | "kitty") => true,
        // A terminal started from inside kitty inherits KITTY_WINDOW_ID
        // (and an older TERM); its own TERM_PROGRAM says what it is.
        Some(_) => false,
        None => matches!(term, "xterm-kitty" | "xterm-ghostty") || set("KITTY_WINDOW_ID"),
    }
}

/// A PNG file that passed the structural checks, with its pixel size.
#[derive(Clone, Eq, PartialEq)]
pub struct Png {
    bytes: Vec<u8>,
    width: NonZeroU32,
    height: NonZeroU32,
}

impl fmt::Debug for Png {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Png")
            .field("bytes", &self.bytes.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PngError {
    /// Not a PNG file at all; such images are shown as links, silently.
    NotPng,
    /// A PNG signature, but a file that cannot be displayed.
    Malformed(&'static str),
    TooLarge,
}

impl fmt::Display for PngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPng => f.write_str("not a PNG file"),
            Self::Malformed(why) => write!(f, "malformed PNG: {why}"),
            Self::TooLarge => f.write_str("too large to show"),
        }
    }
}

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    let word: [u8; 4] = bytes.get(at..end)?.try_into().ok()?;
    Some(u32::from_be_bytes(word))
}

impl Png {
    /// Checks the signature, the header, the chunk framing (an image data
    /// chunk, ending with IEND at the end of the file) and the size limits.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, PngError> {
        if bytes.get(..8) != Some(&SIGNATURE[..]) {
            return Err(PngError::NotPng);
        }
        if bytes.len() > MAX_FILE_BYTES {
            return Err(PngError::TooLarge);
        }
        let truncated = PngError::Malformed("truncated chunk");
        let mut at = SIGNATURE.len();
        let mut header = None;
        let mut data = false;
        loop {
            let length = be_u32(&bytes, at).ok_or(PngError::Malformed("truncated chunk"))?;
            let length = usize::try_from(length).map_err(|_| PngError::TooLarge)?;
            let kind_at = at.checked_add(4).ok_or(PngError::TooLarge)?;
            let data_at = kind_at.checked_add(4).ok_or(PngError::TooLarge)?;
            let end = data_at
                .checked_add(length)
                .and_then(|end| end.checked_add(4))
                .ok_or(PngError::TooLarge)?;
            if end > bytes.len() {
                return Err(truncated);
            }
            let kind = bytes
                .get(kind_at..data_at)
                .ok_or(PngError::Malformed("truncated chunk"))?;
            let body = bytes
                .get(data_at..data_at.saturating_add(length))
                .ok_or(PngError::Malformed("truncated chunk"))?;
            match (kind, header.is_some()) {
                (b"IHDR", false) => header = Some(parse_header(body)?),
                (_, false) => return Err(PngError::Malformed("IHDR is not the first chunk")),
                (b"IHDR", true) => return Err(PngError::Malformed("more than one IHDR")),
                (b"IDAT", true) => data = true,
                (b"IEND", true) => {
                    if !body.is_empty() || end != bytes.len() {
                        return Err(PngError::Malformed("data after IEND"));
                    }
                    break;
                }
                (_, true) => {}
            }
            at = end;
        }
        if !data {
            return Err(PngError::Malformed("no image data"));
        }
        let (width, height) = header.expect("IEND is accepted only after IHDR");
        Ok(Self {
            bytes,
            width,
            height,
        })
    }

    /// The file's size in bytes.
    pub fn size(&self) -> usize {
        self.bytes.len()
    }
}

fn parse_header(body: &[u8]) -> Result<(NonZeroU32, NonZeroU32), PngError> {
    let invalid = PngError::Malformed("invalid IHDR");
    let [
        w0,
        w1,
        w2,
        w3,
        h0,
        h1,
        h2,
        h3,
        depth,
        color,
        compression,
        filter,
        interlace,
    ] = <[u8; 13]>::try_from(body).map_err(|_| PngError::Malformed("invalid IHDR"))?;
    let legal = match color {
        0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
        3 => matches!(depth, 1 | 2 | 4 | 8),
        2 | 4 | 6 => matches!(depth, 8 | 16),
        _ => false,
    };
    if !legal || compression != 0 || filter != 0 || interlace > 1 {
        return Err(invalid);
    }
    let width = NonZeroU32::new(u32::from_be_bytes([w0, w1, w2, w3]))
        .ok_or(PngError::Malformed("zero width"))?;
    let height = NonZeroU32::new(u32::from_be_bytes([h0, h1, h2, h3]))
        .ok_or(PngError::Malformed("zero height"))?;
    let pixels = u64::from(width.get()) * u64::from(height.get());
    if width.get() > MAX_SIDE || height.get() > MAX_SIDE || pixels > MAX_PIXELS {
        return Err(PngError::TooLarge);
    }
    Ok((width, height))
}

#[derive(Debug)]
pub enum LoadError {
    Read(std::io::Error),
    Png(PngError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "could not read it: {error}"),
            Self::Png(error) => error.fmt(f),
        }
    }
}

/// The PNG at `path`, read only up to `limit` bytes; `None` for a file in
/// another format.
pub fn load(path: &std::path::Path, limit: usize) -> Result<Option<Png>, LoadError> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(LoadError::Read)?;
    let mut bytes = Vec::new();
    let cap = u64::try_from(limit.min(MAX_FILE_BYTES)).unwrap_or(u64::MAX);
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(LoadError::Read)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > cap {
        return if bytes.starts_with(&SIGNATURE) {
            Err(LoadError::Png(PngError::TooLarge))
        } else {
            Ok(None)
        };
    }
    match Png::parse(bytes) {
        Ok(png) => Ok(Some(png)),
        Err(PngError::NotPng) => Ok(None),
        Err(error) => Err(LoadError::Png(error)),
    }
}

/// The size of a terminal cell in pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellSize {
    pub width: NonZeroU32,
    pub height: NonZeroU32,
}

impl CellSize {
    /// Assumed when the terminal does not report its pixel size. The
    /// terminal fits an image inside its placement keeping its aspect ratio,
    /// so a wrong guess only leaves a margin.
    pub const FALLBACK: Self = Self {
        width: NonZeroU32::MIN.saturating_add(7),
        height: NonZeroU32::MIN.saturating_add(15),
    };

    /// The cell size from a window's pixel and cell dimensions, when all are
    /// known.
    pub fn from_window(pixels: (u16, u16), cells: (u16, u16)) -> Option<Self> {
        let (width, height) = pixels;
        let (columns, rows) = cells;
        let per = |pixels: u16, cells: u16| {
            u32::from(pixels)
                .checked_div(u32::from(cells))
                .and_then(NonZeroU32::new)
        };
        Some(Self {
            width: per(width, columns)?,
            height: per(height, rows)?,
        })
    }
}

/// How many cells an image covers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Placement {
    pub columns: usize,
    pub rows: usize,
}

fn ceil_div(value: u64, by: u64) -> u64 {
    value.div_ceil(by.max(1))
}

/// The cells `png` covers: its natural size in cells, shrunk keeping its
/// aspect ratio to at most `columns` by `rows` (and the placeholder limit),
/// never enlarged. `None` when there is no room.
pub fn place(png: &Png, cell: CellSize, columns: usize, rows: usize) -> Option<Placement> {
    let to_u64 = |value: usize| u64::try_from(value).unwrap_or(u64::MAX);
    let max_columns = to_u64(columns.min(DIACRITICS.len()));
    let max_rows = to_u64(rows.min(DIACRITICS.len()));
    if max_columns == 0 || max_rows == 0 {
        return None;
    }
    let (width, height) = (u64::from(png.width.get()), u64::from(png.height.get()));
    let (cell_width, cell_height) = (u64::from(cell.width.get()), u64::from(cell.height.get()));
    let mut fit_columns = ceil_div(width, cell_width).clamp(1, max_columns);
    // The rows the image needs at that width, keeping its aspect ratio.
    let mut fit_rows = ceil_div(height * fit_columns * cell_width, width * cell_height).max(1);
    if fit_rows > max_rows {
        fit_rows = max_rows;
        fit_columns =
            ceil_div(width * fit_rows * cell_height, height * cell_width).clamp(1, max_columns);
    }
    Some(Placement {
        columns: usize::try_from(fit_columns).ok()?,
        rows: usize::try_from(fit_rows).ok()?,
    })
}

/// A 24-bit image ID for `png` shown at `placement`, never zero. The same
/// image at the same size always gets the same ID, so showing it again
/// replaces the copy the terminal already holds.
pub fn image_id(png: &Png, placement: Placement) -> u32 {
    let mut hash = Sha256::new();
    hash.update(&png.bytes);
    hash.update(placement.columns.to_be_bytes());
    hash.update(placement.rows.to_be_bytes());
    let digest = hash.finalize();
    let id = digest
        .iter()
        .take(3)
        .fold(0u32, |id, byte| (id << 8) | u32::from(*byte));
    id.max(1)
}

/// The escape sequences that send `png` to the terminal under `id` and make
/// a virtual placement of it covering `placement`. Quiet: the terminal sends
/// no reply.
pub fn transfer(png: &Png, id: u32, placement: Placement) -> String {
    let data = BASE64_STANDARD.encode(&png.bytes);
    let mut out = String::with_capacity(data.len() + data.len() / CHUNK * 32 + 128);
    let mut chunks = data.as_bytes().chunks(CHUNK).peekable();
    let mut first = true;
    while let Some(chunk) = chunks.next() {
        let more = u8::from(chunks.peek().is_some());
        let chunk = std::str::from_utf8(chunk).expect("base64 is ASCII");
        if first {
            out.push_str(&format!(
                "{APC_START}a=t,f=100,t=d,i={id},q=2,m={more};{chunk}{APC_END}"
            ));
            first = false;
        } else {
            out.push_str(&format!("{APC_START}q=2,m={more};{chunk}{APC_END}"));
        }
    }
    out.push_str(&format!(
        "{APC_START}a=p,U=1,i={id},c={},r={},q=2{APC_END}",
        placement.columns, placement.rows
    ));
    out
}

/// Row `row` of the placeholder grid of image `id`, `columns` cells wide.
pub fn placeholder_row(id: u32, row: usize, columns: usize) -> String {
    let [_, red, green, blue] = id.to_be_bytes();
    let mark = |index: usize| {
        *DIACRITICS
            .get(index)
            .expect("placements fit the diacritics table")
    };
    let mut out = format!("\x1b[38;2;{red};{green};{blue}m");
    for column in 0..columns {
        out.push(PLACEHOLDER);
        out.push(mark(row));
        out.push(mark(column));
    }
    out.push_str("\x1b[39m");
    out
}

/// An image drawn at a size, under its ID.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Drawn {
    path: String,
    placement: Placement,
}

#[derive(Debug, Default)]
struct State {
    images: HashMap<String, Png>,
    /// PNG bytes read so far.
    loaded: usize,
    drawn: BTreeMap<u32, Drawn>,
    /// PNG bytes the transfers of `drawn` send.
    sent: usize,
}

/// The PNG images one command can show, by the local path its Markdown
/// names them with, and the ones it drew. Rendering draws placeholder text
/// only; [`Self::transfers`] then sends each drawn image once, outside the
/// text. One registry serves every view a command prints, so image IDs
/// never collide and the byte limits hold for the whole command.
#[derive(Debug)]
pub struct InlineImages {
    cell: CellSize,
    max_rows: usize,
    /// The most PNG bytes read, and the most sent.
    limit: usize,
    state: Mutex<State>,
}

/// Two registries are equal only when they are the same one.
impl PartialEq for InlineImages {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for InlineImages {}

impl InlineImages {
    pub fn new(cell: CellSize, max_rows: usize) -> Self {
        Self::with_limit(cell, max_rows, MAX_TOTAL_BYTES)
    }

    fn with_limit(cell: CellSize, max_rows: usize, limit: usize) -> Self {
        Self {
            cell,
            max_rows,
            limit,
            state: Mutex::new(State::default()),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .expect("no thread panicked holding the image state")
    }

    /// Makes `png` available under `path` without reading a file.
    #[cfg(test)]
    pub fn insert(&self, path: &str, png: Png) {
        let mut state = self.state();
        state.loaded = state.loaded.saturating_add(png.size());
        state.images.insert(path.to_owned(), png);
    }

    /// Reads the PNG at `path` unless it is already known; `None` for a
    /// file in another format.
    pub fn load(&self, path: &str) -> Result<Option<()>, LoadError> {
        let mut state = self.state();
        if state.images.contains_key(path) {
            return Ok(Some(()));
        }
        let left = self.limit.saturating_sub(state.loaded);
        let Some(png) = load(std::path::Path::new(path), left)? else {
            return Ok(None);
        };
        state.loaded = state.loaded.saturating_add(png.size());
        state.images.insert(path.to_owned(), png);
        Ok(Some(()))
    }

    /// The placeholder rows drawing the image at `path` at most `columns`
    /// wide, recording it for [`Self::transfers`]; `None` for an image this
    /// command cannot show, including one whose transfer would pass the
    /// byte limit.
    pub fn draw(&self, path: &str, columns: usize) -> Option<Vec<String>> {
        let mut state = self.state();
        let png = state.images.get(path)?;
        let placement = place(png, self.cell, columns, self.max_rows)?;
        let size = png.size();
        let mut id = image_id(png, placement);
        let wanted = Drawn {
            path: path.to_owned(),
            placement,
        };
        // Another image hashed to the same ID: take the next free one.
        while state.drawn.get(&id).is_some_and(|other| *other != wanted) {
            id = ((id + 1) & 0x00ff_ffff).max(1);
        }
        if !state.drawn.contains_key(&id) {
            let sent = state
                .sent
                .checked_add(size)
                .filter(|sent| *sent <= self.limit)?;
            state.sent = sent;
            state.drawn.insert(id, wanted);
        }
        Some(
            (0..placement.rows)
                .map(|row| placeholder_row(id, row, placement.columns))
                .collect(),
        )
    }

    /// The escape sequences that send every drawn image to the terminal,
    /// each once.
    pub fn transfers(&self) -> String {
        let state = self.state();
        state
            .drawn
            .iter()
            .map(|(id, drawn)| {
                let png = state
                    .images
                    .get(&drawn.path)
                    .expect("only loaded images are drawn");
                transfer(png, *id, drawn.placement)
            })
            .collect()
    }
}

/// A structurally valid PNG of the given size, for tests.
#[cfg(test)]
pub fn sample_png(width: u32, height: u32) -> Png {
    let chunk = |kind: &[u8; 4], body: &[u8]| {
        let mut out = u32::try_from(body.len())
            .expect("small chunk")
            .to_be_bytes()
            .to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out.extend_from_slice(&[0; 4]);
        out
    };
    let mut header = width.to_be_bytes().to_vec();
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut bytes = SIGNATURE.to_vec();
    bytes.extend(chunk(b"IHDR", &header));
    bytes.extend(chunk(b"IDAT", &[0x78, 0x9c, 0x63, 0, 0]));
    bytes.extend(chunk(b"IEND", &[]));
    Png::parse(bytes).expect("a valid sample PNG")
}

#[cfg(test)]
mod tests;
