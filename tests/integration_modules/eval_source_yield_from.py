def make_values():
    for value in (1, 2, 3):
        yield value


def forward(gen):
    yield from gen
