# TODO

- support super()
- support __class__
- handle class annotations specifically
- preserve type hints
- only allow import star when at unconditional top level
- investigate failing CPython tests: test_frozen, test_importlib
- ensure all internals are prefixed with _dp, and rewrite to disallow user code from accessing them
- template accept Box<T> or T