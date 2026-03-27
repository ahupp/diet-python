from __future__ import annotations

import functools
import pickle


class HasCache:
    @functools.lru_cache()
    def cached_meth(self, x, y):
        return x + y


def pickle_cached_method():
    return pickle.loads(pickle.dumps(HasCache.cached_meth))

# diet-python: validate

def validate_module(module):

    cached = module.pickle_cached_method()
    assert cached is not None
    assert cached.__qualname__ == "HasCache.cached_meth"
