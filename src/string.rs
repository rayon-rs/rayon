//! This module contains extension methods for `String` that expose
//! parallel iterators, such as `par_split_whitespace()`. You will
//! rarely need to interact with it directly, since if you add `use
//! rayon::prelude::*` to your file, that will include the helper
//! traits defined in this module.

use iter::*;
use iter::internal::*;
use std::cmp::min;


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
    private_decl!{}

    /// Returns a parallel iterator over the characters of a string.
    fn par_chars(&self) -> ParChars;

    /// Returns a parallel iterator over substrings separated by a
    /// given character, similar to `str::split`.
    fn par_split<P: Pattern>(&self, P) -> ParSplit<P>;

    /// Returns a parallel iterator over substrings terminated by a
    /// given character, similar to `str::split_terminator`.  It's
    /// equivalent to `par_split`, except it doesn't produce an empty
    /// substring after a trailing terminator.
    fn par_split_terminator<P: Pattern>(&self, P) -> ParSplitTerminator<P>;

    /// Returns a parallel iterator over the lines of a string, ending with an
    /// optional carriage return and with a newline (`\r\n` or just `\n`).
    /// The final line ending is optional, and line endings are not included in
    /// the output strings.
    fn par_lines(&self) -> ParLines;

    /// Returns a parallel iterator over the sub-slices of a string that are
    /// separated by any amount of whitespace.
    ///
    /// As with `str::split_whitespace`, 'whitespace' is defined according to
    /// the terms of the Unicode Derived Core Property `White_Space`.
    fn par_split_whitespace(&self) -> ParSplitWhitespace;
}

impl ParallelString for str {
    private_impl!{}

    fn par_chars(&self) -> ParChars {
        ParChars { chars: self }
    }

    fn par_split<P: Pattern>(&self, separator: P) -> ParSplit<P> {
        ParSplit::new(self, separator)
    }

    fn par_split_terminator<P: Pattern>(&self, terminator: P) -> ParSplitTerminator<P> {
        ParSplitTerminator::new(self, terminator)
    }

    fn par_lines(&self) -> ParLines {
        ParLines(self)
    }

    fn par_split_whitespace(&self) -> ParSplitWhitespace {
        ParSplitWhitespace(self)
    }
}


// /////////////////////////////////////////////////////////////////////////

/// Pattern-matching trait for `ParallelString`, somewhat like a mix of
/// `std::str::pattern::{Pattern, Searcher}`.
pub trait Pattern: Sized + Sync {
    fn find_in(&self, &str) -> Option<usize>;
    fn rfind_in(&self, &str) -> Option<usize>;
    fn is_suffix_of(&self, &str) -> bool;
    fn fold_with<'ch, F>(&self, &'ch str, folder: F, skip_last: bool) -> F where F: Folder<&'ch str>;
}

impl Pattern for char {
    fn find_in(&self, chars: &str) -> Option<usize> {
        chars.find(*self)
    }

    fn rfind_in(&self, chars: &str) -> Option<usize> {
        chars.rfind(*self)
    }

    fn is_suffix_of(&self, chars: &str) -> bool {
        chars.ends_with(*self)
    }

    fn fold_with<'ch, F>(&self, chars: &'ch str, folder: F, skip_last: bool) -> F
        where F: Folder<&'ch str>
    {
        let mut split = chars.split(*self);
        if skip_last {
            split.next_back();
        }
        folder.consume_iter(split)
    }
}

impl<FN: Sync + Fn(char) -> bool> Pattern for FN {
    fn find_in(&self, chars: &str) -> Option<usize> {
        chars.find(self)
    }

    fn rfind_in(&self, chars: &str) -> Option<usize> {
        chars.rfind(self)
    }

    fn is_suffix_of(&self, chars: &str) -> bool {
        chars.ends_with(self)
    }

    fn fold_with<'ch, F>(&self, chars: &'ch str, folder: F, skip_last: bool) -> F
        where F: Folder<&'ch str>
    {
        let mut split = chars.split(self);
        if skip_last {
            split.next_back();
        }
        folder.consume_iter(split)
    }
}


// /////////////////////////////////////////////////////////////////////////

pub struct ParChars<'ch> {
    chars: &'ch str,
}

impl<'ch> ParallelIterator for ParChars<'ch> {
    type Item = char;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge_unindexed(self, consumer)
    }
}

impl<'ch> UnindexedProducer for ParChars<'ch> {
    type Item = char;

    fn split(mut self) -> (Self, Option<Self>) {
        let index = find_char_midpoint(self.chars);
        if index > 0 {
            let (left, right) = self.chars.split_at(index);
            self.chars = left;
            (self, Some(ParChars { chars: right }))
        } else {
            (self, None)
        }
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        folder.consume_iter(self.chars.chars())
    }
}


// /////////////////////////////////////////////////////////////////////////

pub struct ParSplit<'ch, P: Pattern> {
    chars: &'ch str,
    separator: P,
}

struct ParSplitProducer<'ch, 'sep, P: Pattern + 'sep> {
    chars: &'ch str,
    separator: &'sep P,

    /// Marks the endpoint beyond which we've already found no separators.
    tail: usize,
}

impl<'ch, P: Pattern> ParSplit<'ch, P> {
    fn new(chars: &'ch str, separator: P) -> Self {
        ParSplit {
            chars: chars,
            separator: separator,
        }
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> ParSplitProducer<'ch, 'sep, P> {
    fn new(split: &'sep ParSplit<'ch, P>) -> Self {
        ParSplitProducer {
            chars: split.chars,
            separator: &split.separator,
            tail: split.chars.len(),
        }
    }

    /// Common `fold_with` implementation, integrating `ParSplitTerminator`'s
    /// need to sometimes skip its final empty item.
    fn fold_with<F>(self, folder: F, skip_last: bool) -> F
        where F: Folder<<Self as UnindexedProducer>::Item>
    {
        let ParSplitProducer { chars, separator, tail } = self;

        if tail == chars.len() {
            // No tail section, so just let `str::split` handle it.
            separator.fold_with(chars, folder, skip_last)

        } else if let Some(index) = separator.rfind_in(&chars[..tail]) {
            // We found the last separator to complete the tail, so
            // end with that slice after `str::split` finds the rest.
            let (left, right) = chars.split_at(index);
            let folder = separator.fold_with(left, folder, false);
            if skip_last || folder.full() {
                folder
            } else {
                let mut right_iter = right.chars();
                right_iter.next(); // skip the separator
                folder.consume(right_iter.as_str())
            }

        } else {
            // We know there are no separators at all.  Return our whole string.
            if skip_last {
                folder
            } else {
                folder.consume(chars)
            }
        }
    }
}

impl<'ch, P: Pattern> ParallelIterator for ParSplit<'ch, P> {
    type Item = &'ch str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = ParSplitProducer::new(&self);
        bridge_unindexed(producer, consumer)
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> UnindexedProducer for ParSplitProducer<'ch, 'sep, P> {
    type Item = &'ch str;

    fn split(mut self) -> (Self, Option<Self>) {
        let ParSplitProducer { chars, separator, tail } = self;

        // First find a suitable UTF-8 boundary in the unsearched region.
        let char_index = find_char_midpoint(&chars[..tail]);

        // Look forward for the separator, and failing that look backward.
        let index = separator.find_in(&chars[char_index..tail])
            .map(|i| char_index + i)
            .or_else(|| separator.rfind_in(&chars[..char_index]));

        if let Some(index) = index {
            let (left, right) = chars.split_at(index);

            // Update `self` as the region before the separator.
            self.chars = left;
            self.tail = min(char_index, index);

            // Create the right split following the separator.
            let mut right_iter = right.chars();
            right_iter.next(); // skip the separator
            let right_chars = right_iter.as_str();
            let right_index = chars.len() - right_chars.len();

            let mut right = ParSplitProducer {
                chars: right_chars,
                separator: separator,
                tail: tail - right_index,
            };

            // If we scanned backwards to find the separator, everything in
            // the right side is exhausted, with no separators left to find.
            if index < char_index {
                right.tail = 0;
            }

            (self, Some(right))

        } else {
            self.tail = 0;
            (self, None)
        }
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        self.fold_with(folder, false)
    }
}


// /////////////////////////////////////////////////////////////////////////

pub struct ParSplitTerminator<'ch, P: Pattern> {
    splitter: ParSplit<'ch, P>,
}

struct ParSplitTerminatorProducer<'ch, 'sep, P: Pattern + 'sep> {
    splitter: ParSplitProducer<'ch, 'sep, P>,
    endpoint: bool,
}

impl<'ch, P: Pattern> ParSplitTerminator<'ch, P> {
    fn new(chars: &'ch str, terminator: P) -> Self {
        ParSplitTerminator { splitter: ParSplit::new(chars, terminator) }
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> ParSplitTerminatorProducer<'ch, 'sep, P> {
    fn new(split: &'sep ParSplitTerminator<'ch, P>) -> Self {
        ParSplitTerminatorProducer {
            splitter: ParSplitProducer::new(&split.splitter),
            endpoint: true,
        }
    }
}

impl<'ch, P: Pattern> ParallelIterator for ParSplitTerminator<'ch, P> {
    type Item = &'ch str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = ParSplitTerminatorProducer::new(&self);
        bridge_unindexed(producer, consumer)
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> UnindexedProducer for ParSplitTerminatorProducer<'ch, 'sep, P> {
    type Item = &'ch str;

    fn split(mut self) -> (Self, Option<Self>) {
        let (left, right) = self.splitter.split();
        self.splitter = left;
        let right = right.map(|right| {
            let endpoint = self.endpoint;
            self.endpoint = false;
            ParSplitTerminatorProducer {
                splitter: right,
                endpoint: endpoint,
            }
        });
        (self, right)
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        // See if we need to eat the empty trailing substring
        let skip_last = if self.endpoint {
            let chars = self.splitter.chars;
            let terminator = self.splitter.separator;
            chars.is_empty() || terminator.is_suffix_of(chars)
        } else {
            false
        };

        self.splitter.fold_with(folder, skip_last)
    }
}


// /////////////////////////////////////////////////////////////////////////

pub struct ParLines<'ch>(&'ch str);

impl<'ch> ParallelIterator for ParLines<'ch> {
    type Item = &'ch str;

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


// /////////////////////////////////////////////////////////////////////////

pub struct ParSplitWhitespace<'ch>(&'ch str);

impl<'ch> ParallelIterator for ParSplitWhitespace<'ch> {
    type Item = &'ch str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        self.0
            .par_split(char::is_whitespace)
            .filter(|string| !string.is_empty())
            .drive_unindexed(consumer)
    }
}
