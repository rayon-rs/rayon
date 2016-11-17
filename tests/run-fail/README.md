This directory contains test files that are **expected to run and
panic**. Usually, though, these tests are better written with
`#[test]` tests. To use it, create a `.rs` file and include a comment
like

```
// error-pattern:boom
```

indicating the panic message we should expect to see.

Note: if the test uses unstable features, prefer the
`run-fail-unstable` directory.

