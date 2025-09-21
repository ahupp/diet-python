def probe(value):
    match value:
        case iterable if not hasattr(iterable, "__next__"):
            return f"no next for {type(iterable).__name__}"
        case _:
            return "has next"
