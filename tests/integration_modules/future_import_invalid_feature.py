SOURCE = "from __future__ import not_a_feature\nVALUE = 1\n"

# diet-python: validate

import os
import tempfile
import pytest
import diet_import_hook

def validate(module):
    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False) as handle:
        handle.write(module.SOURCE)
        path = handle.name
    try:
        transformed = diet_import_hook._transform_source(path)
        with pytest.raises(SyntaxError) as excinfo:
            compile(transformed, path, "exec")
        assert "not_a_feature" in str(excinfo.value)
    finally:
        os.remove(path)
