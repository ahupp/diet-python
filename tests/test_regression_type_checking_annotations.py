from tests._integration import transformed_module


def test_type_checking_annotations_in_class(tmp_path):
    source = """
from typing import TYPE_CHECKING

SENTINEL = object()


class Marker:
    if TYPE_CHECKING:
        typed_attr: int
        other_attr: str

    value = SENTINEL
"""
    with transformed_module(tmp_path, "type_checking_annotations_regression", source) as module:
        assert module.Marker.value is module.SENTINEL
