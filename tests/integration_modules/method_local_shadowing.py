class Example:
    def run(self):
        run = 1
        return run

# diet-python: validate

def validate_module(module):

    instance = module.Example()
    assert instance.run() == 1
