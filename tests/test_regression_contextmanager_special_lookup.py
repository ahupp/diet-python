from tests._integration import transformed_module


def test_with_bypasses_getattribute_for_specials(tmp_path):
    source = """

def run():
    events = []
    class C:
        def __getattribute__(self, name):
            if name in ("__enter__", "__exit__"):
                events.append(name)
            return object.__getattribute__(self, name)
        def __enter__(self):
            return self
        def __exit__(self, exc_type, exc, tb):
            return False
    with C():
        pass
    return events
"""
    with transformed_module(tmp_path, "with_special_lookup", source) as module:
        assert module.run() == []
