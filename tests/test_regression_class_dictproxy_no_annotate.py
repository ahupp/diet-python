from tests._integration import transformed_module


def test_class_dictproxy_omits_dunder_annotate(tmp_path):
    source = """
class C:
    def meth(self):
        pass

def run():
    return "__annotate__" in C.__dict__
"""
    with transformed_module(tmp_path, "class_dictproxy_no_annotate", source) as module:
        assert module.run() is False
