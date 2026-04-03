def pluralize(count):
    return f'time{"s" if count > 1 else ""}'


# diet-python: validate
assert pluralize(1) == "time"
assert pluralize(2) == "times"
