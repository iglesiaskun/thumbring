//! Minimal PNG writer: 8-bit RGBA. The ring preview stores the pixels
//! without compression, which is enough for a visual check with no extra
//! dependency; the build script, which makes the app icon, brings its own
//! compressor.

/// Encodes straight (not premultiplied) RGBA rows, top to bottom, without
/// compression.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    encode_rgba_with(width, height, rgba, stored)
}

/// The same, with `zlib` turning the raw rows into a zlib stream.
pub fn encode_rgba_with(
    width: u32,
    height: u32,
    rgba: &[u8],
    zlib: impl FnOnce(&[u8]) -> Vec<u8>,
) -> Vec<u8> {
    let row = width as usize * 4;
    assert_eq!(rgba.len(), row * height as usize, "pixel buffer size");

    // Each row starts with its filter type (0 = none).
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgba.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }

    let zlib = zlib(&raw);

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    // 8 bits per channel, RGBA, default compression, filter and no interlace.
    header.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib);
    chunk(&mut png, b"IEND", &[]);
    png
}

/// zlib stream with stored deflate blocks of up to 65535 bytes.
fn stored(raw: &[u8]) -> Vec<u8> {
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    if blocks.peek().is_none() {
        zlib.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(block) = blocks.next() {
        let last = blocks.peek().is_none();
        let len = block.len() as u16;
        zlib.push(u8::from(last));
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(raw).to_be_bytes());
    zlib
}

/// Converts premultiplied BGRA (as Direct3D gives it) to straight RGBA.
pub fn bgra_premultiplied_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bgra.len());
    for px in bgra.as_chunks::<4>().0 {
        let a = px[3];
        let straight = |c: u8| {
            if a == 0 {
                0
            } else {
                ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8
            }
        };
        out.extend_from_slice(&[straight(px[2]), straight(px[1]), straight(px[0]), a]);
    }
    out
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = png.len();
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let crc = crc32(&png[start..]);
    png.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, entry) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *entry = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc = table[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in bytes {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_the_reference_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn a_png_has_signature_header_and_end() {
        let png = encode_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..20], &2u32.to_be_bytes());
        assert_eq!(&png[20..24], &1u32.to_be_bytes());
        // The empty IEND chunk always ends with the same CRC.
        assert_eq!(
            &png[png.len() - 8..],
            &[b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
        );
    }

    #[test]
    fn big_images_are_split_into_stored_blocks() {
        let (w, h) = (200u32, 100u32);
        let png = encode_rgba(w, h, &vec![7; (w * h * 4) as usize]);
        // 100 rows of 801 bytes need two 65535-byte blocks.
        assert!(png.len() > (w * h * 4) as usize);
    }

    #[test]
    fn premultiplied_colors_are_made_straight() {
        // Half-transparent white, premultiplied: 128, 128, 128, 128.
        assert_eq!(
            bgra_premultiplied_to_rgba(&[128, 128, 128, 128]),
            [255, 255, 255, 128]
        );
        assert_eq!(bgra_premultiplied_to_rgba(&[10, 20, 30, 0]), [0, 0, 0, 0]);
        // Blue channel comes first in BGRA.
        assert_eq!(
            bgra_premultiplied_to_rgba(&[255, 0, 0, 255]),
            [0, 0, 255, 255]
        );
    }
}
