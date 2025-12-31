# Expected Failures

- `test.test_code.CodeTest.test_code_hash_uses_bytecode`: diet-python rewrites `lambda x, y: x + y` and `lambda x, y: x * y` to calls like `__dp__.add(x, y)` and `__dp__.mul(x, y)`, so both lambdas compile to the same call shape. As a result, their `co_code` bytecode is identical and `c.replace(co_code=d.co_code)` does not change the code object, even though the original CPython bytecode would differ for `BINARY_ADD` vs `BINARY_MULTIPLY`.
