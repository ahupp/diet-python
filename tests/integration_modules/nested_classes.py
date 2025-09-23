calls = []


def record(tag: str):
    def decorator(cls):
        calls.append((tag, cls.__qualname__, getattr(cls, "label", None)))
        applied = list(getattr(cls, "applied_decorators", []))
        applied.append(tag)
        cls.applied_decorators = applied
        return cls

    return decorator


class Outer:
    label = "outer"

    @record(label + "_mid")
    class Mid:
        label = "mid"

        @record(label + "_inner")
        @record("stack")
        class Inner:
            label = "inner"

            @record(label + "_leaf")
            class Leaf:
                pass
