from __future__ import annotations


def exercise():
    test_namespace = None

    class Meta(type):
        def __new__(cls, name, bases, namespace):
            nonlocal test_namespace
            test_namespace = namespace
            return None

    class A(metaclass=Meta):
        @staticmethod
        def f():
            return __class__

    B = type("B", (), test_namespace)
    return A, B, B.f()
