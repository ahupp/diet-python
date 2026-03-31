class Base:
    def probe(self):
        return "sentinel"


class Container:
    def build(self):
        class Derived(Base):
            def probe(self):
                return super().probe()

        instance = Derived()
        return instance.probe()


# diet-python: validate

def validate_module(module):
    assert module.Container().build() == "sentinel"
