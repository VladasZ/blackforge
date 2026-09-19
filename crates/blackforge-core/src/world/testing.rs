//! Builds the bytes of a save for unit tests.

use crate::world::zdo::stable_hash;

#[derive(Default)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    pub fn raw(mut self, bytes: &[u8]) -> Self {
        self.0.extend(bytes);
        self
    }

    pub fn i32(self, value: i32) -> Self {
        self.raw(&value.to_le_bytes())
    }

    pub fn vector(self, value: [f32; 3]) -> Self {
        value
            .iter()
            .fold(self, |bytes, part| bytes.raw(&part.to_le_bytes()))
    }

    /// The hash the game stores for the name of a prefab or a property.
    pub fn hash(self, name: &str) -> Self {
        self.i32(stable_hash(name))
    }

    /// Text shorter than 128 bytes, so the length is one byte.
    pub fn text(self, value: &str) -> Self {
        let length = u8::try_from(value.len()).expect("the text is too long for a test");
        self.raw(&[length]).raw(value.as_bytes())
    }

    /// The bytes with their length in front, as a 32 bit number.
    pub fn sized(self, bytes: &[u8]) -> Self {
        let length = i32::try_from(bytes.len()).expect("too many bytes for a test");
        self.i32(length).raw(bytes)
    }

    pub fn done(self) -> Vec<u8> {
        self.0
    }
}
