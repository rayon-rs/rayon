use super::*;
use std::cmp::min;
use std::iter::Chain;
use std::option::IntoIter as OptionIter;
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


/// Parallel extensions for strings.
pub trait ParallelString {
    /// Returns a parallel iterator over the characters of a string.
    fn par_chars(&self) -> ParChars;

    /// Returns a parallel iterator over substrings separated by a
    /// given character, similar to `str::split`.
    fn par_split(&self, char) -> ParSplit;

    /// Returns a parallel iterator over substrings terminated by a
    /// given character, similar to `str::split_terminator`.  It's
    /// equivalent to `par_split`, except it doesn't produce an empty
    /// substring after a trailing terminator.
    fn par_split_terminator(&self, char) -> ParSplitTerminator;

    /// Returns a parallel iterator over the lines of a string, ending with an
    /// optional carriage return and with a newline (`\r\n` or just `\n`).
    /// The final line ending is optional, and line endings are not included in
    /// the output strings.
    fn par_lines(&self) -> ParLines;
}

impl ParallelString for str {
    fn par_chars(&self) -> ParChars {
        ParChars { chars: self }
    }

    fn par_split(&self, separator: char) -> ParSplit {
        ParSplit::new(self, separator)
    }

    fn par_split_terminator(&self, terminator: char) -> ParSplitTerminator {
        ParSplitTerminator::new(self, terminator)
    }

    fn par_lines(&self) -> ParLines {
        ParLines(self)
    }
}


// /////////////////////////////////////////////////////////////////////////

pub struct ParChars<'a> {
    chars: &'a str,
}

impl<'a> ParallelIteratorImpl for ParChars<'a> {
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

    /// Marks the endpoint beyond which we've already found no separators.
    tail: usize,
}

impl<'a> ParSplit<'a> {
    fn new(chars: &'a str, separator: char) -> Self {
        ParSplit {
            chars: chars,
            separator: separator,
            tail: chars.len(),
        }
    }
}

impl<'a> ParallelIteratorImpl for ParSplit<'a> {
    type Item = &'a str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge_unindexed(self, consumer)
    }
}

impl<'a> UnindexedProducer for ParSplit<'a> {
    fn split(&mut self) -> Option<Self> {
        let ParSplit { chars, separator, tail } = *self;

        // First find a suitable UTF-8 boundary in the unsearched region.
        let char_index = find_char_midpoint(&chars[..tail]);

        // Look forward for the separator, and failing that look backward.
        let index = chars[char_index..tail]
            .find(separator)
            .map(|i| char_index + i)
            .or_else(|| chars[..char_index].rfind(separator));

        if let Some(index) = index {
            // Update `self` as the region before the separator.
            self.chars = &chars[..index];
            self.tail = min(char_index, index);

            // Create the right split following the separator.
            let right_index = index + separator.len_utf8();
            let mut right = ParSplit {
                chars: &chars[right_index..],
                separator: separator,
                tail: tail - right_index,
            };

            // If we scanned backwards to find the separator, everything in
            // the right side is exhausted, with no separators left to find.
            if index < char_index {
                right.tail = 0;
            }

            Some(right)

        } else {
            self.tail = 0;
            None
        }
    }
}

impl<'a> IntoIterator for ParSplit<'a> {
    type Item = &'a str;
    type IntoIter = Chain<Split<'a, char>, OptionIter<&'a str>>;

    fn into_iter(self) -> Self::IntoIter {
        let ParSplit { chars, separator, tail } = self;

        if tail == chars.len() {
            // No tail section, so just let `str::split` handle it.
            chars.split(separator).chain(None)

        } else if let Some(index) = chars[..tail].rfind(separator) {
            // We found the last separator to complete the tail, so
            // end with that slice after `str::split` finds the rest.
            let head = &chars[..index];
            let last = &chars[index + separator.len_utf8()..];
            head.split(separator).chain(Some(last))

        } else {
            // We know there are no separators at all.  Return our whole string,
            // but for type correctness we need to chain an emptied `Split` too.
            let mut empty = "".split('\0');
            Iterator::last(&mut empty);
            empty.chain(Some(chars))
        }
    }
}


// /////////////////////////////////////////////////////////////////////////

pub struct ParSplitTerminator<'a> {
    splitter: ParSplit<'a>,
    endpoint: bool,
}

impl<'a> ParSplitTerminator<'a> {
    fn new(chars: &'a str, terminator: char) -> Self {
        ParSplitTerminator {
            splitter: ParSplit::new(chars, terminator),
            endpoint: true,
        }
    }
}

impl<'a> ParallelIteratorImpl for ParSplitTerminator<'a> {
    type Item = &'a str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge_unindexed(self, consumer)
    }
}

impl<'a> UnindexedProducer for ParSplitTerminator<'a> {
    fn split(&mut self) -> Option<Self> {
        self.splitter.split().map(|right| {
            let endpoint = self.endpoint;
            self.endpoint = false;
            ParSplitTerminator {
                splitter: right,
                endpoint: endpoint,
            }
        })
    }
}

impl<'a> IntoIterator for ParSplitTerminator<'a> {
    type Item = &'a str;
    type IntoIter = Chain<Split<'a, char>, OptionIter<&'a str>>;

    fn into_iter(self) -> Self::IntoIter {
        let skip_last = if self.endpoint {
            let chars = self.splitter.chars;
            let terminator = self.splitter.separator;
            chars.is_empty() || chars.ends_with(terminator)
        } else {
            false
        };

        let mut iter = self.splitter.into_iter();

        if skip_last {
            // eat the empty trailing substring
            iter.next_back();
        }

        iter
    }
}


// /////////////////////////////////////////////////////////////////////////

pub struct ParLines<'a>(&'a str);

impl<'a> ParallelIteratorImpl for ParLines<'a> {
    type Item = &'a str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        self.0
            .par_split_terminator('\n')
            .map(|line| {
                if line.ends_with('\r') {
                    &line[..line.len() - 1]
                } else {
                    line
                }
            })
            .drive_unindexed(consumer)
    }
}
