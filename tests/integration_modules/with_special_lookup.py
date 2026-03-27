

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


# diet-python: validate

def validate_module(module):
    assert module.run() == []
