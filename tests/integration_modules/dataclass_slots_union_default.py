import dataclasses


@dataclasses.dataclass(slots=True)
class Example:
    label: str
    state: str | None = None
    count: int = 0


def build_example(**kwargs):
    return Example("label", **kwargs)
