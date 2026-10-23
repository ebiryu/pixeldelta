use core::fmt;

/// An image format, as recognized from the leading bytes of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Format {
    Png,
    Jpeg,
    Gif,
    WebP,
    Bmp,
    Tiff,
    Avif,
    /// The bytes match no format this crate recognizes.
    Unknown,
}

impl Format {
    /// Recognizes the format from the leading bytes of a file.
    ///
    /// Only enough bytes to tell the formats apart are read, so a truncated or
    /// otherwise broken file is still named after what it claims to be.
    pub fn of(bytes: &[u8]) -> Self {
        let at = |range: core::ops::Range<usize>| bytes.get(range);
        if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
            Self::Png
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            Self::Jpeg
        } else if bytes.starts_with(b"GIF8") {
            Self::Gif
        } else if bytes.starts_with(b"RIFF") && at(8..12) == Some(b"WEBP") {
            Self::WebP
        } else if bytes.starts_with(b"BM") {
            Self::Bmp
        } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
            Self::Tiff
        } else if at(4..8) == Some(b"ftyp") && matches!(at(8..12), Some(b"avif") | Some(b"avis")) {
            Self::Avif
        } else {
            Self::Unknown
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::WebP => "WebP",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
            Self::Avif => "AVIF",
            Self::Unknown => "unrecognized",
        };
        f.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::Format;

    #[test]
    fn a_signature_is_recognized_from_the_first_bytes_alone() {
        assert_eq!(
            Format::of(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10]),
            Format::Png
        );
        assert_eq!(Format::of(&[0xff, 0xd8, 0xff]), Format::Jpeg);
        assert_eq!(Format::of(b"GIF89a"), Format::Gif);
        assert_eq!(Format::of(b"RIFF\0\0\0\0WEBPVP8 "), Format::WebP);
        assert_eq!(Format::of(b"BM\0\0"), Format::Bmp);
        assert_eq!(Format::of(b"II*\0"), Format::Tiff);
        assert_eq!(Format::of(b"\0\0\0\x20ftypavif"), Format::Avif);
    }

    #[test]
    fn bytes_too_short_to_hold_a_signature_are_unknown() {
        assert_eq!(Format::of(b""), Format::Unknown);
        assert_eq!(Format::of(&[0x89, b'P']), Format::Unknown);
        assert_eq!(Format::of(b"RIFF"), Format::Unknown);
    }
}
