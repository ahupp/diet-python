from unittest import mock


def mock_class_property_ok():
    m = mock.Mock(spec=int)
    return isinstance(m, int)


RESULT = mock_class_property_ok()

# diet-python: validate

def validate_module(module):

    assert module.RESULT is True
