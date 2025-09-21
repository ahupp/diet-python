def child():
    events = []
    try:
        value = yield "start"
        events.append(("send", value))
        while True:
            try:
                value = yield value
                events.append(("send", value))
            except KeyError as exc:
                events.append(("throw", str(exc)))
                value = "handled"
            if value == "stop":
                break
    finally:
        events.append(("finally", None))
    return events


def delegator():
    result = yield from child()
    return ("done", result)
