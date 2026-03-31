def collect_segments(data: bytes) -> list[bytes]:
    pieces = []
    for start in range(len(data)):
        for end in range(start + 1, len(data) + 1):
            slice = data[start:end]
            pieces.append(slice)
    return pieces


# diet-python: validate

def validate_module(module):
    assert module.collect_segments(b"ab") == [b"a", b"ab", b"b"]
