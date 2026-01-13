def test_traceback_linecache_source_integration(run_integration_module):
    with run_integration_module("traceback_linecache_source") as module:
        traceback_text = module.get_traceback()
        assert 'raise RuntimeError("boom")' in traceback_text
