
class Outer:
    def run(self):
        counter = 0

        class Inner:
            def bump(self):
                nonlocal counter
                counter += 1

        Inner().bump()
        return counter


# diet-python: validate

def validate_module(module):
    assert module.Outer().run() == 1
