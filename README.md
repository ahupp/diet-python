# diet-python

This repository includes a small Rust utility for transforming Python source
code. It parses a file with Ruff's parser and rewrites binary operations and
augmented assignments (e.g., `+=`) into calls to the corresponding functions in
the standard library's `operator` module.

Run it with:

```
cargo run -- path/to/file.py
```

## Python import hook

To apply the transform automatically when modules are imported, install the
provided import hook:

```python
import diet_import_hook
diet_import_hook.install()
```

After calling `install()`, any subsequent imports will be rewritten using the
`diet-python` transform before execution.

Run the included example to see the hook in action:

```
python example_usage.py
```

The script installs the hook, imports `example_module`, and asserts that its
bytecode calls `operator.add` instead of using `BINARY_OP`.
