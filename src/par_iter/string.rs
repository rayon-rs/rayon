use super::internal::*;
use super::*;
use std::str::Chars;

/// Test if a byte is the start of a UTF-8 character.
/// (extracted from `str::is_char_boundary`)
fn is_char_boundary(b: u8) -> bool {
    // This is bit magic equivalent to: b < 128 || b >= 192
    (b as i8) >= -0x40
}


impl<'a> ParallelString for &'a str {
    type Chars = ParChars<'a>;

    fn par_chars(self) -> Self::Chars {
        ParChars { chars: self }
    }
}


pub struct ParChars<'a> {
    chars: &'a str,
}

impl<'a> ParallelIterator for ParChars<'a> {
    type Item = char;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge_unindexed(self, consumer)
    }
}

impl<'a> UnindexedProducer for ParChars<'a> {
    fn can_split(&self) -> bool {
        // This is pessimistic, as we only *know* there are multiple characters
        // when it's longer than Unicode's maximum UTF-8 length of 4.  There
        // could be smaller characters, but it's ok not to split maximally.
        self.chars.len() > 4
    }

    fn split(self) -> (Self, Self) {
        let mid = self.chars.len() / 2;

        // We want to split near the midpoint, but we need to find an actual
        // character boundary.  So we look at the raw bytes, first scanning
        // forward from the midpoint for a boundary, then trying backward.
        let (left, right) = self.chars.as_bytes().split_at(mid);
        let index = right.iter()
            .cloned()
            .position(is_char_boundary)
            .map(|i| mid + i)
            .or_else(|| left.iter().cloned().rposition(is_char_boundary))
            .unwrap_or(0);

        let (left, right) = self.chars.split_at(index);
        (ParChars { chars: left }, ParChars { chars: right })
    }
}

impl<'a> IntoIterator for ParChars<'a> {
    type Item = char;
    type IntoIter = Chars<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.chars.chars()
    }
}
