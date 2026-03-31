def bounded_loop(limit=1):
    start = 0
    while start <= limit:
        start += 1
        if start > 2:
            raise RuntimeError("loop guard not recomputed")
    return start


# diet-python: validate

def validate_module(module):
    assert module.bounded_loop() == 2
