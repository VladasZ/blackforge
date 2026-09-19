use std::str::from_utf8;

use thiserror::Error;

/// Why the bytes of a world save could not be read.
#[derive(Debug, Error)]
pub enum Malformed {
    #[error("the data ends early, at byte {0}")]
    Truncated(usize),
    #[error("a bad length at byte {0}")]
    Length(usize),
    #[error("the text at byte {0} is not valid utf-8")]
    Text(usize),
    #[error("unknown object flags {flags:#04x} at byte {at}")]
    Flags { at: usize, flags: u8 },
    #[error("data is left after the last object, at byte {0}")]
    Trailing(usize),
    #[error("the index lists {listed} objects, the chunk holds {found}")]
    Count { listed: usize, found: usize },
    #[error("world version {0} is not supported")]
    Version(i32),
    #[error("the folder holds no finished save")]
    NoSave,
}

pub type Parsed<T> = Result<T, Malformed>;

/// Reads the little endian values the game writes with a C# `BinaryWriter`.
pub struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn is_at_end(&self) -> bool {
        self.position == self.data.len()
    }

    pub fn bytes(&mut self, count: usize) -> Parsed<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .filter(|end| *end <= self.data.len())
            .ok_or(Malformed::Truncated(self.position))?;
        let bytes = &self.data[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    pub fn skip(&mut self, count: usize) -> Parsed<()> {
        self.bytes(count)?;
        Ok(())
    }

    fn array<const N: usize>(&mut self) -> Parsed<[u8; N]> {
        let at = self.position;
        self.bytes(N)?
            .try_into()
            .map_err(|_| Malformed::Truncated(at))
    }

    pub fn u8(&mut self) -> Parsed<u8> {
        self.array().map(u8::from_le_bytes)
    }

    pub fn u16(&mut self) -> Parsed<u16> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn i16(&mut self) -> Parsed<i16> {
        self.array().map(i16::from_le_bytes)
    }

    pub fn i32(&mut self) -> Parsed<i32> {
        self.array().map(i32::from_le_bytes)
    }

    pub fn f32(&mut self) -> Parsed<f32> {
        self.array().map(f32::from_le_bytes)
    }

    pub fn vector(&mut self) -> Parsed<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    /// A length or a count stored as a signed 32 bit number.
    pub fn length(&mut self) -> Parsed<usize> {
        let at = self.position;
        usize::try_from(self.i32()?).map_err(|_| Malformed::Length(at))
    }

    /// The length in front of a C# string, 7 bits in every byte.
    fn text_length(&mut self) -> Parsed<usize> {
        let at = self.position;
        let mut length = 0;
        for shift in [0, 7, 14, 21, 28] {
            let byte = self.u8()?;
            length |= usize::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(length);
            }
        }
        Err(Malformed::Length(at))
    }

    pub fn text(&mut self) -> Parsed<String> {
        let length = self.text_length()?;
        let at = self.position;
        from_utf8(self.bytes(length)?)
            .map(str::to_owned)
            .map_err(|_| Malformed::Text(at))
    }

    pub fn skip_text(&mut self) -> Parsed<()> {
        let length = self.text_length()?;
        self.skip(length)
    }

    /// A count the way saves before version 31 store it, as one C# `char`,
    /// which the writer encodes as utf-8.
    pub fn char_count(&mut self) -> Parsed<usize> {
        let at = self.position;
        let first = usize::from(self.u8()?);
        match first {
            0..0x80 => Ok(first),
            0xc0..0xe0 => {
                let second = self.continuation()?;
                Ok(((first & 0x1f) << 6) | second)
            }
            0xe0..0xf0 => {
                let second = self.continuation()?;
                let third = self.continuation()?;
                Ok(((first & 0x0f) << 12) | (second << 6) | third)
            }
            _ => Err(Malformed::Length(at)),
        }
    }

    fn continuation(&mut self) -> Parsed<usize> {
        Ok(usize::from(self.u8()? & 0x3f))
    }

    /// A count the way saves from version 31 on store it. It is one byte.
    /// With `wide` a set high bit means a second byte follows.
    pub fn count(&mut self, wide: bool) -> Parsed<usize> {
        let first = usize::from(self.u8()?);
        if wide && first & 0x80 != 0 {
            return Ok(((first & 0x7f) << 8) | usize::from(self.u8()?));
        }
        Ok(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_counts_in_every_encoding() -> Parsed<()> {
        let mut reader = Reader::new(&[0x05, 0xc4, 0x80, 0x05, 0x81, 0x02, 0x81]);
        assert_eq!(reader.char_count()?, 5);
        assert_eq!(reader.char_count()?, 256);
        assert_eq!(reader.count(true)?, 5);
        assert_eq!(reader.count(true)?, 258);
        assert_eq!(reader.count(false)?, 0x81);
        assert!(reader.is_at_end());
        Ok(())
    }

    #[test]
    fn a_read_past_the_end_is_an_error() {
        let mut reader = Reader::new(&[0x04, b'a']);
        assert!(matches!(reader.text(), Err(Malformed::Truncated(1))));
    }
}
