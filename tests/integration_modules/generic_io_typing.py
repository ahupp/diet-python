from typing import Generic, List, TypeVar

AnyStr = TypeVar("AnyStr", str, bytes)


class Example(Generic[AnyStr]):
    def readlines(self) -> List[AnyStr]:
        ...
