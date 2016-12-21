use super::internal::*;
use super::*;
use std::cmp::max;
use std::iter::{self, Chain, Once};
use std::str::{Chars, Split};

/// Test if a byte is the start of a UTF-8 character.
/// (extracted from `str::is_char_boundary`)
fn is_char_boundary(b: u8) -> bool {
    // This is bit magic equivalent to: b < 128 || b >= 192
    (b as i8) >= -0x40
}

/// Find the index of a character boundary near the midpoint.
fn find_char_midpoint(chars: &str) -> usize {
    let mid = chars.len() / 2;

    // We want to split near the midpoint, but we need to find an actual
    // character boundary.  So we look at the raw bytes, first scanning
    // forward from the midpoint for a boundary, then trying backward.
    let (left, right) = chars.as_bytes().split_at(mid);
    right.iter()
        .cloned()
        .position(is_char_boundary)
        .map(|i| mid + i)
        .or_else(|| left.iter().cloned().rposition(is_char_boundary))
        .unwrap_or(0)
}


impl<'a> ParallelString for &'a str {
    type Chars = ParChars<'a>;
    type Split = ParSplit<'a>;

    fn par_chars(self) -> Self::Chars {
        ParChars { chars: self }
    }

    fn par_split(self, separator: char) -> Self::Split {
        ParSplit::new(self, separator)
    }
}


// /////////////////////////////////////////////////////////////////////////

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
    fn split(&mut self) -> Option<Self> {
        let index = find_char_midpoint(self.chars);
        if index > 0 {
            let (left, right) = self.chars.split_at(index);
            self.chars = left;
            Some(ParChars { chars: right })
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for ParChars<'a> {
    type Item = char;
    type IntoIter = Chars<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.chars.chars()
    }
}


// /////////////////////////////////////////////////////////////////////////

pub struct ParSplit<'a> {
    chars: &'a str,
    separator: char,

    /// Keeps track of the first separator found in the string.  This lets us
    /// quickly answer `can_split`, and it also corresponds to what parts we've
    /// already scanned as we keep splitting smaller.
    first: Option<usize>,
}

impl<'a> ParSplit<'a> {
    fn new(chars: &'a str, separator: char) -> Self {
        ParSplit {
            chars: chars,
            separator: separator,
            first: chars.find(separator),
        }
    }
}

impl<'a> ParallelIterator for ParSplit<'a> {
    type Item = &'a str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge_unindexed(self, consumer)
    }
}

impl<'a> UnindexedProducer for ParSplit<'a> {
    fn split(&mut self) -> Option<Self> {
        let ParSplit { chars, separator, first } = *self;
        let first = match first {
            None => return None,
            Some(first) => first,
        };

        // First find a suitable UTF-8 boundary in the unsearched region.
        let char_index = find_char_midpoint(&chars[first..]) + first;

        // Find a separator in reverse, towards the `first` that we know exists.
        let index = chars[first..char_index]
            .rfind(separator)
            .map(|i| i + first)
            .unwrap_or(first);

        // Update `self` as the left side of the split.  It might not have a
        // `first` anymore if that's the exact separator we're splitting on now.
        self.chars = &chars[..index];
        if first == index {
            self.first = None;
        }

        // Return the right side of the split starting just after this separator.
        // We find its `first` starting from the `char_index` already scanned above.
        let right_index = index + separator.len_utf8();
        let right_search = max(char_index, right_index);
        let right_first = chars[right_search..]
            .find(separator)
            .map(|i| i + (right_search - right_index));
        Some(ParSplit {
            chars: &chars[right_index..],
            separator: separator,
            first: right_first,
        })
    }
}

impl<'a> IntoIterator for ParSplit<'a> {
    type Item = &'a str;
    type IntoIter = Chain<Once<&'a str>, Split<'a, char>>;

    fn into_iter(self) -> Self::IntoIter {
        if let Some(first) = self.first {
            // We know where the first separator is, so start with that
            // and then let `str::split` find the rest.
            let head = &self.chars[..first];
            let tail = &self.chars[first + self.separator.len_utf8()..];
            iter::once(head).chain(tail.split(self.separator))
        } else {
            // We know there are no separators at all.  Return our whole string,
            // but for type correctness we need to chain an emptied `Split` too.
            let head = self.chars;
            let mut tail = "".split('\0');
            Iterator::last(&mut tail);
            iter::once(head).chain(tail)
        }
    }
}
