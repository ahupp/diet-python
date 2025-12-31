from __future__ import annotations


def test_lru_cache_method_pickles(run_integration_module):
    with run_integration_module("functools_lru_cache_pickle_method") as module:
        cached = module.pickle_cached_method()
        assert cached is not None
        assert cached.__qualname__ == "HasCache.cached_meth"
