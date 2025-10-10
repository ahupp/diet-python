class Recording:
    def __init__(self) -> None:
        self.exited = False

    def __enter__(self) -> "Recording":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.exited = True


def use_context(manager: "Recording") -> "Recording":
    with manager as result:
        return result


def run() -> tuple[bool, "Recording"]:
    manager = Recording()
    result = use_context(manager)
    return manager.exited, result
