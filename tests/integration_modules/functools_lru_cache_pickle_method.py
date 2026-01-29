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

from __future__ import annotations

module = __import__("sys").modules[__name__]
cached = module.pickle_cached_method()
assert cached is not None
assert cached.__qualname__ == "HasCache.cached_meth"
