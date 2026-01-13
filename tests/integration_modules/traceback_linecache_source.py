import traceback


class Boom:
    def explode(self):
        raise RuntimeError("boom")


def get_traceback():
    try:
        Boom().explode()
    except RuntimeError:
        return traceback.format_exc()
