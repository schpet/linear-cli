mod json;
mod mock;
mod sandbox;

pub use json::{assert_json, nodes};
pub use mock::{MockLinear, Request};
pub use sandbox::{API_KEY, Cli, Run};

/// A structurally valid PNG `width` by `height` pixels (its pixel data is
/// not decodable; only the terminal decodes it).
pub fn png(width: u32, height: u32) -> Vec<u8> {
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
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(chunk(b"IHDR", &header));
    bytes.extend(chunk(b"IDAT", &[0x78, 0x9c, 0x63, 0, 0]));
    bytes.extend(chunk(b"IEND", &[]));
    bytes
}
