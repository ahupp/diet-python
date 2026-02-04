import importlib


def test_sitecustomize_ignores_install_importerror(monkeypatch):
    monkeypatch.setenv("DIET_PYTHON_INSTALL_HOOK", "1")
    import sitecustomize

    def _boom():
        raise ImportError("diet-python pyo3 module is required but could not be imported")

    monkeypatch.setattr(sitecustomize.diet_import_hook, "install", _boom)

    # Regression: this used to bubble up and print "Error in sitecustomize".
    importlib.reload(sitecustomize)
