This directory contains test files that are **not expected to
compile**.  It is useful for writing tests that things which ought not
to type-check do not, in fact, type-check.

To write a test, just write a `.rs` file using Rayon. Then, for each
compilation error, write a `//~ ERROR E123` annotation on the line
where the error occurs. `E123` should be the error code that is issued
by rustc. This should be reasonably robust against future compiler
changes, though in some cases the errors may start showing up on
different lines etc as compiler heuristics change.

