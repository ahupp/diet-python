from __future__ import annotations

import functools
import pickle


class HasCache:
    @functools.lru_cache()
    def cached_meth(self, x, y):
        return x + y


def pickle_cached_method():
    return pickle.loads(pickle.dumps(HasCache.cached_meth))
