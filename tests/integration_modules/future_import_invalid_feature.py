SOURCE = "from __future__ import not_a_feature\nVALUE = 1\n"

# diet-python: validate

def validate_module(module):
    import os
    import tempfile
    import pytest
    from pathlib import Path
    from tests import _integration

    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False) as handle:
        handle.write(module.SOURCE)
        path = handle.name
    try:
        with pytest.raises(SyntaxError) as excinfo:
            _integration.render_transformed_source(Path(path))
        assert "not_a_feature" in str(excinfo.value)
    finally:
        os.remove(path)
