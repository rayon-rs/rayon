//! This module contains extension methods for `String` that expose
//! parallel iterators, such as `par_split_whitespace()`. You will
//! rarely need to interact with it directly, since if you add `use
//! rayon::prelude::*` to your file, that will include the helper
//! traits defined in this module.
//!
//! Note: [`ParallelString::par_split()`] and [`par_split_terminator()`]
//! reference a `Pattern` trait which is not visible outside this crate.
//! This trait is intentionally kept private, for use only by Rayon itself.
//! It is implemented for `char` and any `F: Fn(char) -> bool + Sync + Send`.
//!
//! [`ParallelString::par_split()`]: trait.ParallelString.html#method.par_split
//! [`par_split_terminator()`]: trait.ParallelString.html#method.par_split_terminator

use iter::*;
use iter::internal::*;
use split_producer::*;


/// Test if a byte is the start of a UTF-8 character.
/// (extracted from `str::is_char_boundary`)
#[inline]
fn is_char_boundary(b: u8) -> bool {
    // This is bit magic equivalent to: b < 128 || b >= 192
    (b as i8) >= -0x40
}

/// Find the index of a character boundary near the midpoint.
#[inline]
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
    /// Returns a plain string slice, which is used to implement the rest of
    /// the parallel methods.
    fn as_parallel_string(&self) -> &str;

    /// Returns a parallel iterator over the characters of a string.
    fn par_chars(&self) -> Chars {
        Chars { chars: self.as_parallel_string() }
    }

    /// Returns a parallel iterator over substrings separated by a
    /// given character or predicate, similar to `str::split`.
    ///
    /// Note: the `Pattern` trait is private, for use only by Rayon itself.
    /// It is implemented for `char` and any `F: Fn(char) -> bool + Sync + Send`.
    fn par_split<P: Pattern>(&self, separator: P) -> Split<P> {
        Split::new(self.as_parallel_string(), separator)
    }

    /// Returns a parallel iterator over substrings terminated by a
    /// given character or predicate, similar to `str::split_terminator`.
    /// It's equivalent to `par_split`, except it doesn't produce an empty
    /// substring after a trailing terminator.
    ///
    /// Note: the `Pattern` trait is private, for use only by Rayon itself.
    /// It is implemented for `char` and any `F: Fn(char) -> bool + Sync + Send`.
    fn par_split_terminator<P: Pattern>(&self, terminator: P) -> SplitTerminator<P> {
        SplitTerminator::new(self.as_parallel_string(), terminator)
    }

    /// Returns a parallel iterator over the lines of a string, ending with an
    /// optional carriage return and with a newline (`\r\n` or just `\n`).
    /// The final line ending is optional, and line endings are not included in
    /// the output strings.
    fn par_lines(&self) -> Lines {
        Lines(self.as_parallel_string())
    }

    /// Returns a parallel iterator over the sub-slices of a string that are
    /// separated by any amount of whitespace.
    ///
    /// As with `str::split_whitespace`, 'whitespace' is defined according to
    /// the terms of the Unicode Derived Core Property `White_Space`.
    fn par_split_whitespace(&self) -> SplitWhitespace {
        SplitWhitespace(self.as_parallel_string())
    }
}

impl ParallelString for str {
    #[inline]
    fn as_parallel_string(&self) -> &str {
        self
    }
}


// /////////////////////////////////////////////////////////////////////////

/// We hide the `Pattern` trait in a private module, as its API is not meant
/// for general consumption.  If we could have privacy on trait items, then it
/// would be nicer to have its basic existence and implementors public while
/// keeping all of the methods private.
mod private {
    use iter::internal::Folder;

    /// Pattern-matching trait for `ParallelString`, somewhat like a mix of
    /// `std::str::pattern::{Pattern, Searcher}`.
    ///
    /// Implementing this trait is not permitted outside of `rayon`.
    pub trait Pattern: Sized + Sync + Send {
        private_decl!{}
        fn find_in(&self, &str) -> Option<usize>;
        fn rfind_in(&self, &str) -> Option<usize>;
        fn is_suffix_of(&self, &str) -> bool;
        fn fold_with<'ch, F>(&self, &'ch str, folder: F, skip_last: bool) -> F
            where F: Folder<&'ch str>;
    }
}
use self::private::Pattern;

impl Pattern for char {
    private_impl!{}

    #[inline]
    fn find_in(&self, chars: &str) -> Option<usize> {
        chars.find(*self)
    }

    #[inline]
    fn rfind_in(&self, chars: &str) -> Option<usize> {
        chars.rfind(*self)
    }

    #[inline]
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

impl<FN: Sync + Send + Fn(char) -> bool> Pattern for FN {
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

impl<'ch, P: Pattern> Split<'ch, P> {
    fn new(chars: &'ch str, separator: P) -> Self {
        Split {
            chars: chars,
            separator: separator,
        }
    }
}

impl<'ch, P: Pattern> ParallelIterator for Split<'ch, P> {
    type Item = &'ch str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitProducer::new(self.chars, &self.separator);
        bridge_unindexed(producer, consumer)
    }
}

/// Implement support for `SplitProducer`.
impl<'ch, P: Pattern> Fissile<P> for &'ch str {
    fn length(&self) -> usize {
        self.len()
    }

    fn midpoint(&self, end: usize) -> usize {
        // First find a suitable UTF-8 boundary.
        find_char_midpoint(&self[..end])
    }

    fn find(&self, separator: &P, start: usize, end: usize) -> Option<usize> {
        separator.find_in(&self[start..end])
    }

    fn rfind(&self, separator: &P, end: usize) -> Option<usize> {
        separator.rfind_in(&self[..end])
    }

    fn split_once(self, index: usize) -> (Self, Self) {
        let (left, right) = self.split_at(index);
        let mut right_iter = right.chars();
        right_iter.next(); // skip the separator
        (left, right_iter.as_str())
    }

    fn fold_splits<F>(self, separator: &P, folder: F, skip_last: bool) -> F
        where F: Folder<Self>
    {
        separator.fold_with(self, folder, skip_last)
    }
}


// /////////////////////////////////////////////////////////////////////////

/// Parallel iterator over substrings separated by a terminator pattern
pub struct SplitTerminator<'ch, P: Pattern> {
    chars: &'ch str,
    terminator: P,
}

struct SplitTerminatorProducer<'ch, 'sep, P: Pattern + 'sep> {
    splitter: SplitProducer<'sep, P, &'ch str>,
    skip_last: bool,
}

impl<'ch, P: Pattern> SplitTerminator<'ch, P> {
    fn new(chars: &'ch str, terminator: P) -> Self {
        SplitTerminator {
            chars: chars,
            terminator: terminator,
        }
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> SplitTerminatorProducer<'ch, 'sep, P> {
    fn new(chars: &'ch str, terminator: &'sep P) -> Self {
        SplitTerminatorProducer {
            splitter: SplitProducer::new(chars, terminator),
            skip_last: chars.is_empty() || terminator.is_suffix_of(chars),
        }
    }
}

impl<'ch, P: Pattern> ParallelIterator for SplitTerminator<'ch, P> {
    type Item = &'ch str;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitTerminatorProducer::new(self.chars, &self.terminator);
        bridge_unindexed(producer, consumer)
    }
}

impl<'ch, 'sep, P: Pattern + 'sep> UnindexedProducer for SplitTerminatorProducer<'ch, 'sep, P> {
    type Item = &'ch str;

    fn split(mut self) -> (Self, Option<Self>) {
        let (left, right) = self.splitter.split();
        self.splitter = left;
        let right = right.map(|right| {
            let skip_last = self.skip_last;
            self.skip_last = false;
            SplitTerminatorProducer {
                splitter: right,
                skip_last: skip_last,
            }
        });
        (self, right)
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        self.splitter.fold_with(folder, self.skip_last)
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
