def test_exception_cleanup_deleted(run_integration_module):
    with run_integration_module("exception_cleanup_deleted") as module:
        assert module.cleanup_deleted() is False
        assert module.unbound_after_delete() is True
