use core::fmt;

/// Number of bytes per pixel in the RGBA8 layout the engine works on.
pub const BYTES_PER_PIXEL: usize = 4;

/// A borrowed RGBA8 image.
///
/// Pixels are stored row by row, four bytes per pixel, without padding between
/// rows. The buffer is borrowed so that callers decide where the pixels live.
#[derive(Debug, Clone, Copy)]
pub struct Image<'a> {
    width: u32,
    height: u32,
    data: &'a [u8],
}

impl<'a> Image<'a> {
    /// Wraps an RGBA8 buffer.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError`] when `width * height * 4` overflows `usize` or
    /// exceeds the length of `data`.
    pub fn from_rgba8(width: u32, height: u32, data: &'a [u8]) -> Result<Self, ImageError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
            .ok_or(ImageError::DimensionOverflow { width, height })?;
        if data.len() < expected {
            return Err(ImageError::BufferTooSmall {
                expected,
                actual: data.len(),
            });
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Number of pixels.
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Pixel bytes, truncated to `width * height * 4`.
    pub fn as_bytes(&self) -> &'a [u8] {
        let len = self.pixel_count() as usize * BYTES_PER_PIXEL;
        &self.data[..len]
    }

    /// Pixels as four-byte groups, `width * height` of them.
    pub fn pixels(&self) -> &'a [[u8; BYTES_PER_PIXEL]] {
        let (pixels, _) = self.as_bytes().as_chunks::<BYTES_PER_PIXEL>();
        pixels
    }

    /// Whether both images have the same dimensions.
    pub fn same_size_as(&self, other: &Image<'_>) -> bool {
        self.width == other.width && self.height == other.height
    }
}

/// Reasons an RGBA8 buffer cannot be wrapped in an [`Image`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    /// The buffer holds fewer bytes than the dimensions require.
    BufferTooSmall { expected: usize, actual: usize },
    /// `width * height * 4` does not fit in `usize`.
    DimensionOverflow { width: u32, height: u32 },
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { expected, actual } => {
                write!(f, "buffer holds {actual} bytes, {expected} required")
            }
            Self::DimensionOverflow { width, height } => {
                write!(f, "{width}x{height} pixels do not fit in an address space")
            }
        }
    }
}

impl core::error::Error for ImageError {}
