class Example:
    def run(self):
        run = 1
        return run

# diet-python: validate

from __future__ import annotations

def validate(module):
    instance = module.Example()
    assert instance.run() == 1
