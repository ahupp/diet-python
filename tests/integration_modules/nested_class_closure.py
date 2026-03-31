class Container:
    def build(self):
        values = []

        class Recorder:
            def record(self, item):
                values.append(item)
                return list(values)

        return Recorder()


def use_container() -> list[str]:
    recorder = Container().build()
    return recorder.record("payload")


# diet-python: validate

def validate_module(module):
    assert module.use_container() == ["payload"]
