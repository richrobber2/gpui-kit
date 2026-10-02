//! Native engine XRGB1 wire format, validated before creating a GPUI image.
pub(crate) fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), &'static str> {
    if bytes.len() < 9 || &bytes[..5] != b"XRGB1" {
        return Err("Invalid preview header");
    }
    let width = u16::from_le_bytes([bytes[5], bytes[6]]) as u32;
    let height = u16::from_le_bytes([bytes[7], bytes[8]]) as u32;
    if width == 0
        || height == 0
        || width > 1024
        || height > 1024
        || bytes.len() != 9 + (width * height * 3) as usize
    {
        return Err("Incomplete or oversized preview");
    }
    let mut bgra = Vec::with_capacity((width * height * 4) as usize);
    for rgb in bytes[9..].chunks_exact(3) {
        bgra.extend_from_slice(&[rgb[2], rgb[1], rgb[0], 255]);
    }
    Ok((width, height, bgra))
}
#[cfg(test)]
mod tests {
    use super::decode;
    #[test]
    fn preserves_dimensions_and_converts_rgb_to_gpui_bgra() {
        let bytes = b"XRGB1\x02\x00\x01\x00\xff\x00\x00\x00\x00\xff";
        let (w, h, bgra) = decode(bytes).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(bgra, [0, 0, 255, 255, 255, 0, 0, 255]);
    }
    #[test]
    fn rejects_partial_writes_and_bad_headers() {
        let bytes = b"XRGB1\x01\x00\x01\x00\x12\x34\x56";
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end]).is_err());
        }
        assert!(decode(b"OTHER\x01\x00\x01\x00\x12\x34\x56").is_err());
    }
    #[test]
    fn rejects_zero_oversized_and_trailing_data() {
        assert!(decode(b"XRGB1\x00\x00\x01\x00").is_err());
        assert!(decode(b"XRGB1\x01\x04\x01\x00").is_err());
        assert!(decode(b"XRGB1\x01\x00\x01\x00\x12\x34\x56\x00").is_err());
    }
}
