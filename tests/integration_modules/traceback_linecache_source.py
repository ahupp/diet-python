import traceback


class Boom:
    def explode(self):
        raise RuntimeError("boom")


def get_traceback():
    try:
        Boom().explode()
    except RuntimeError:
        return traceback.format_exc()

# diet-python: validate

def validate(module):
    traceback_text = module.get_traceback()
    assert 'getattr(Boom(), "explode")()' in traceback_text
