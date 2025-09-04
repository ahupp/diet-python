# diet-python

This repository includes a small Rust utility for transforming Python source
code. It parses a file with Ruff's parser and rewrites binary operations and
augmented assignments (e.g., `+=`) into calls to the corresponding functions in
the standard library's `operator` module.

Run it with:

```
cargo run -- path/to/file.py
```
