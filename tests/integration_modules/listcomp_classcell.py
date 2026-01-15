def classcell_values():
    class C:
        def method(self):
            super()
            return __class__
        items = [(lambda: i) for i in range(5)]
        y = [x() for x in items]

    return C.y, C().method(), C
