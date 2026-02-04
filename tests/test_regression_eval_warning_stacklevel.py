import pytest

from tests._integration import integration_module


@pytest.mark.parametrize("mode", ["transform", "eval"])
def test_warning_filter_module_stacklevel(tmp_path, mode):
    if mode == "eval":
        pytest.xfail("requires _warn_with_soac_stack warning stack shim in __dp__")

    source = """
import warnings


def target():
    warnings.warn(
        "FileType is deprecated. Simply open files after parsing arguments.",
        PendingDeprecationWarning,
        stacklevel=2,
    )


def caller():
    target()


def run():
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        warnings.filterwarnings(
            "ignore",
            "FileType is deprecated",
            PendingDeprecationWarning,
            __name__,
        )
        caller()
        if caught:
            warning = caught[0]
            return len(caught), str(warning.filename), int(getattr(warning, "lineno", -1))
        return 0, None, None
"""

    with integration_module(tmp_path, "warning_stacklevel_filter", source, mode=mode) as module:
        assert module.run() == (0, None, None)
