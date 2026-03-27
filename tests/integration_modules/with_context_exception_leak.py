import gc
import weakref


class CaptureExc:
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.exc = exc
        return True


def leak_check():
    class Victim:
        pass

    victim = Victim()
    with CaptureExc():
        raise RuntimeError("boom")
    ref = weakref.ref(victim)
    victim = None
    gc.collect()
    return ref()

# diet-python: validate

def validate_module(module):

    assert module.leak_check() is None
