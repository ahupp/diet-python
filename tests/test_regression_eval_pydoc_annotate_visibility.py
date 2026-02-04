from tests._integration import integration_module


def test_eval_pydoc_hides_class_annotate_method(tmp_path):
    source = """
class B:
    NO_MEANING: str = "eggs"
"""
    with integration_module(tmp_path, "eval_pydoc_annotate", source, mode="eval") as module:
        import pydoc

        assert callable(getattr(module.B, "__annotate__"))
        text = pydoc.TextDoc().docmodule(module)
        assert "__annotate__(" not in text
