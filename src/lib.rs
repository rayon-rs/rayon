#![doc(html_root_url = "https://docs.rs/rayon/0.8.2")]
#![allow(non_camel_case_types)] // I prefer to use ALL_CAPS for type parameters
#![cfg_attr(test, feature(conservative_impl_trait))]
#![cfg_attr(test, feature(i128_type))]

// If you're not compiling the unstable code, it often happens that
// there is stuff that is considered "dead code" and so forth. So
// disable warnings in that scenario.
#![cfg_attr(not(rayon_unstable), allow(warnings))]

extern crate rayon_core;
extern crate either;

#[cfg(test)]
extern crate rand;

#[macro_use]
mod delegate;

#[macro_use]
mod private;

mod split_producer;

pub mod collections;
pub mod iter;
pub mod option;
pub mod prelude;
pub mod range;
pub mod result;
pub mod slice;
pub mod str;
pub mod vec;

mod par_either;
mod test;

pub use iter::split;

pub use rayon_core::current_num_threads;
pub use rayon_core::Configuration;
pub use rayon_core::initialize;
pub use rayon_core::ThreadPool;
pub use rayon_core::join;
pub use rayon_core::{scope, Scope};
pub use rayon_core::spawn;

/// Fork and join many expressions at once.
///
/// The syntax is one or more occurrences of
///
/// ```ignore
/// let <irrefutable pattern> = fork <closure expresssion>;`.
/// ```
///
/// For example,
///
/// ```
/// #[macro_use]
/// extern crate rayon;
///
/// # fn main() {
/// join! {
///     let w = fork || 0;
///     let x = fork || 1;
///     let y = fork || 2;
///     let z = fork || 3;
/// }
///
/// assert_eq!(w, 0);
/// assert_eq!(x, 1);
/// assert_eq!(y, 2);
/// assert_eq!(z, 3);
/// # }
/// ```
///
/// This is equivalent to nesting calls to `rayon::join` like this:
///
/// ```
/// # extern crate rayon;
/// let (w, (x, (y, z))) = rayon::join(
///     || 0,
///     || rayon::join(
///         || 1,
///         || rayon::join(
///             || 2,
///             || 3,
///         )
///     )
/// );
/// ```
///
/// Alternatively, you can just get a flattened tuple of results, without
/// binding the results to any variable inside the macro.
///
/// The syntax is one or more occurrences of `<closure expression> ,` where the
/// last `,` is optional.
///
/// ```rust
/// #[macro_use]
/// extern crate rayon;
///
/// # fn main() {
/// let (w, x, y, z) = join!(|| 0, || 1, || 2, || 3);
///
/// assert_eq!(w, 0);
/// assert_eq!(x, 1);
/// assert_eq!(y, 2);
/// assert_eq!(z, 3);
/// # }
/// ```
#[macro_export]
macro_rules! join {
    // Entry point for `let <pat> = fork <closure>;` usage.
    ( $( let $lhs:pat = fork $rhs:expr ; )+ ) => {
        let join!( @left $( $lhs , )+ ) = join!( @right $( $rhs , )+ );
    };

    // Entry point for `<closure>,` usage.
    ( $x:expr $( , $xs:expr )* ) => {
        join! { @flat $x $( , $xs )* }
    };

    // Flattening tuples with temporary variables.
    ( @flat $( let $lhs:ident = $rhs:expr ; )+ ) => {
        {
            let join!( @left $( $lhs , )+ ) = join!( @right $( $rhs , )+ );
            ($( $lhs ),+)
        }
    };
    ( @flat $( let $lhs:ident = $rhs:expr ; )* $x:expr $( , $xs:expr )*) => {
        join! { @flat
            $( let $lhs = $rhs ; )*
            let lhs = $x;
            $($xs),*
        }
    };

    // Left hand side recursion to nest individual patterns into tuple patterns
    // like `(x, (y, (z, ...)))`.
    ( @left $x:pat , ) => {
        $x
    };
    ( @left $x:pat , $( $xs:pat , )+ ) => {
        ( $x , join!( @left $( $xs , )+ ) )
    };

    // Right hand side recursion to nest exprs into rayon fork-joins
    // like:
    //
    //     rayon::join(
    //         x,
    //         || rayon::join(
    //             y,
    //             || rayon::join(
    //                 z,
    //                 || ...)))
    ( @right $x:expr , ) => {
        ($x)()
    };
    ( @right $x:expr , $( $xs:expr , )+ ) => {
        ::rayon::join( $x , || join!( @right $( $xs , )+ ) )
    }
}

// Necessary for the tests using macros that expand to `::rayon::whatever`.
#[cfg(test)]
mod rayon {
    pub use super::*;
}

#[cfg(test)]
mod tests {
    #[macro_use]
    use super::*;

    #[test]
    fn join_macro_with_more_complex_patterns() {
        struct Point(usize, usize);

        join! {
            let Point(w, x) = fork || Point(1, 2);
            let Point(y, z) = fork || Point(3, 4);
            let (((((a, _), _), _), _), _) = fork || (((((5, 4), 3), 2), 1), 0);
        };

        assert_eq!(w, 1);
        assert_eq!(x, 2);
        assert_eq!(y, 3);
        assert_eq!(z, 4);
        assert_eq!(a, 5);
    }
}
