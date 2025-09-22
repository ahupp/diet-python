class date:
    __slots__ = ()


class Example:
    slots = date.__slots__

    def date(self):
        return date()

