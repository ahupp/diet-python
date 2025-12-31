from __future__ import annotations

def test_cpython_strptime_failure_formats_offset(run_integration_module):
    with run_integration_module("cpython_strptime_failure") as module:
        assert module.parse_invalid_offset() == "Inconsistent use of : in -01:3030"
