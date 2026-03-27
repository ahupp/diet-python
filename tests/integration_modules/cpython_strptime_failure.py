def _format_timezone_offset(format):
    parts = {"z": "-01:3030"}
    return f"Inconsistent use of : in {parts['z']}"


def parse_invalid_offset():
    return _format_timezone_offset("%z")

# diet-python: validate

def validate_module(module):

    assert module.parse_invalid_offset() == "Inconsistent use of : in -01:3030"
