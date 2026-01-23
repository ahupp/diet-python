def collect_for_else_continue_minimal():
    seen = []
    for outer in range(2):
        for _inner in []:
            seen.append((_inner, outer))
        else:
            seen.append(outer)
            continue
        seen.append("unreachable")
    return seen


RESULT = collect_for_else_continue_minimal()

# diet-python: validate

def validate(module):
    assert module.RESULT == [0, 1]
