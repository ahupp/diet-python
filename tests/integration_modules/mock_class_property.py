from unittest import mock


def mock_class_property_ok():
    m = mock.Mock(spec=int)
    return isinstance(m, int)


RESULT = mock_class_property_ok()
