def parse_line(line: str) -> str:
    try:
        key, value = line.split("=")
    except ValueError:
        return "handled"
    else:
        return "missing separator"


# diet-python: validate

def validate_module(module):
    assert module.parse_line("no equals here") == "handled"
