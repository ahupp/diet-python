class Example:
    def trigger(self):
        counter = 0

        class Token:
            def bump(self):
                nonlocal counter
                counter += 1

        token = Token()
        token.bump()
        return counter


# diet-python: validate

def validate_module(module):
    assert module.Example().trigger() == 1
