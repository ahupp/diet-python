class Example:
    def close(self):
        close = lambda: "ok"
        if close:
            return close()
        return "no"
