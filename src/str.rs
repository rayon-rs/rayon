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
///
/// Implementing this trait is not permitted outside of `rayon`.
pub trait ParallelString {
    private_decl!{}

    /// Returns a parallel iterator over the characters of a string.
    fn par_chars(&self) -> Chars;

    /// Returns a parallel iterator over substrings separated by a
    /// given character, similar to `str::split`.
    fn par_split<P: Pattern>(&self, P) -> Split<P>;

    /// Returns a parallel iterator over substrings terminated by a
    /// given character, similar to `str::split_terminator`.  It's
    /// equivalent to `par_split`, except it doesn't produce an empty
    /// substring after a trailing terminator.
    fn par_split_terminator<P: Pattern>(&self, P) -> SplitTerminator<P>;

    /// Returns a parallel iterator over the lines of a string, ending with an
    /// optional carriage return and with a newline (`\r\n` or just `\n`).
    /// The final line ending is optional, and line endings are not included in
    /// the output strings.
    fn par_lines(&self) -> Lines;

    /// Returns a parallel iterator over the sub-slices of a string that are
    /// separated by any amount of whitespace.
    ///
    /// As with `str::split_whitespace`, 'whitespace' is defined according to
    /// the terms of the Unicode Derived Core Property `White_Space`.
    fn par_split_whitespace(&self) -> SplitWhitespace;
}

impl ParallelString for str {
    private_impl!{}

    fn par_chars(&self) -> Chars {
        Chars { chars: self }
    }

    fn par_split<P: Pattern>(&self, separator: P) -> Split<P> {
        Split::new(self, separator)
    }

    fn par_split_terminator<P: Pattern>(&self, terminator: P) -> SplitTerminator<P> {
        SplitTerminator::new(self, terminator)
    }

    fn par_lines(&self) -> Lines {
        Lines(self)
    }

    fn par_split_whitespace(&self) -> SplitWhitespace {
        SplitWhitespace(self)
    }
}


// /////////////////////////////////////////////////////////////////////////

/// Pattern-matching trait for `ParallelString`, somewhat like a mix of
/// `std::str::pattern::{Pattern, Searcher}`.
///
/// Implementing this trait is not permitted outside of `rayon`.
pub trait Pattern: Sized + Sync {
    private_decl!{}
    fn find_in(&self, &str) -> Option<usize>;
    fn rfind_in(&self, &str) -> Option<usize>;
    fn is_suffix_of(&self, &str) -> bool;
    fn fold_with<'ch, F>(&self, &'ch str, folder: F, skip_last: bool) -> F where F: Folder<&'ch str>;
}

impl Pattern for char {
    private_impl!{}

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
    private_impl!{}

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

/// Parallel iterator over the characters of a string
pub struct Chars<'ch> {
    chars: &'ch str,
}

struct CharsProducer<'ch> {
    chars: &'ch str,
}

impl<'ch> ParallelIterator for Chars<'ch> {
    type Item = char;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge_unindexed(CharsProducer { chars: self.chars }, consumer)
    }
}

impl<'ch> UnindexedProducer for CharsProducer<'ch> {
    type Item = char;

    fn split(mut self) -> (Self, Option<Self>) {
        let index = find_char_midpoint(self.chars);
        if index > 0 {
            let (left, right) = self.chars.split_at(index);
            self.chars = left;
            (self, Some(CharsProducer { chars: right }))
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

/// Parallel iterator over substrings separated by a pattern
pub struct Split<'ch, P: Pattern> {
    chars: &'ch str,
    separator: P,
}

struct SplitProducer<'ch, 'sep, P: Pattern + 'sep> {
    chars: &'ch str,
    separator: &'sep P,

    /// Marks the endpoint beyond which we've already found no separators.
    tail: usize,
}

impl<'ch, P: Pattern> Split<'ch, P> {
    fn new(chars: &'ch str, separator: P) -> Self {
        Split {
            chars: chars,
            separator: separator,
        }
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> SplitProducer<'ch, 'sep, P> {
    fn new(split: &'sep Split<'ch, P>) -> Self {
        SplitProducer {
            chars: split.chars,
            separator: &split.separator,
            tail: split.chars.len(),
        }
    }

    /// Common `fold_with` implementation, integrating `SplitTerminator`'s
    /// need to sometimes skip its final empty item.
    fn fold_with<F>(self, folder: F, skip_last: bool) -> F
        where F: Folder<<Self as UnindexedProducer>::Item>
    {
        let SplitProducer { chars, separator, tail } = self;

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

impl<'ch, P: Pattern> ParallelIterator for Split<'ch, P> {
    type Item = &'ch str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitProducer::new(&self);
        bridge_unindexed(producer, consumer)
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> UnindexedProducer for SplitProducer<'ch, 'sep, P> {
    type Item = &'ch str;

    fn split(mut self) -> (Self, Option<Self>) {
        let SplitProducer { chars, separator, tail } = self;

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

            let mut right = SplitProducer {
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

/// Parallel iterator over substrings separated by a terminator pattern
pub struct SplitTerminator<'ch, P: Pattern> {
    splitter: Split<'ch, P>,
}

struct SplitTerminatorProducer<'ch, 'sep, P: Pattern + 'sep> {
    splitter: SplitProducer<'ch, 'sep, P>,
    endpoint: bool,
}

impl<'ch, P: Pattern> SplitTerminator<'ch, P> {
    fn new(chars: &'ch str, terminator: P) -> Self {
        SplitTerminator { splitter: Split::new(chars, terminator) }
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> SplitTerminatorProducer<'ch, 'sep, P> {
    fn new(split: &'sep SplitTerminator<'ch, P>) -> Self {
        SplitTerminatorProducer {
            splitter: SplitProducer::new(&split.splitter),
            endpoint: true,
        }
    }
}

impl<'ch, P: Pattern> ParallelIterator for SplitTerminator<'ch, P> {
    type Item = &'ch str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitTerminatorProducer::new(&self);
        bridge_unindexed(producer, consumer)
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> UnindexedProducer for SplitTerminatorProducer<'ch, 'sep, P> {
    type Item = &'ch str;

    fn split(mut self) -> (Self, Option<Self>) {
        let (left, right) = self.splitter.split();
        self.splitter = left;
        let right = right.map(|right| {
            let endpoint = self.endpoint;
            self.endpoint = false;
            SplitTerminatorProducer {
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

/// Parallel iterator over lines in a string
pub struct Lines<'ch>(&'ch str);

impl<'ch> ParallelIterator for Lines<'ch> {
    type Item = &'ch str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        self.0
            .par_split_terminator('\n')
            .map(|line| if line.ends_with('\r') {
                     &line[..line.len() - 1]
                 } else {
                     line
                 })
            .drive_unindexed(consumer)
    }
}


// /////////////////////////////////////////////////////////////////////////

/// Parallel iterator over substrings separated by whitespace
pub struct SplitWhitespace<'ch>(&'ch str);

impl<'ch> ParallelIterator for SplitWhitespace<'ch> {
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
