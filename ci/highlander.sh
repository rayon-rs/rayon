#!/bin/bash

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "INFO: There Can Be Only One!" >&2

if cargo build --manifest-path "$DIR/highlander/Cargo.toml"; then
    echo "ERROR: we built with multiple rayon-core!" >&2
    exit 1
fi

echo "PASS: using multiple rayon-core failed." >&2
