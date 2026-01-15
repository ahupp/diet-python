#!/usr/bin/env python3
import argparse
import runpy
import sys


def _build_event_map():
    event_map = {}
    for name in dir(sys.monitoring):
        if name.startswith("EVENT_"):
            event_map[name[6:]] = getattr(sys.monitoring, name)
    return event_map


def _parse_event_list(values):
    events = []
    for value in values:
        for part in value.split(","):
            part = part.strip()
            if part:
                events.append(part.upper())
    return events


def _resolve_events(event_map, include, exclude):
    if include:
        names = set(include)
    else:
        names = set(event_map.keys())
    if exclude:
        names -= set(exclude)
    unknown = sorted(name for name in names if name not in event_map)
    if unknown:
        valid = ", ".join(sorted(event_map.keys()))
        raise SystemExit(f"Unknown events: {', '.join(unknown)}. Valid events: {valid}")
    return sorted(names)


def _format_event(event_name, args):
    code = args[0] if args else None
    filename = getattr(code, "co_filename", "?") if code else "?"
    func = getattr(code, "co_name", "?") if code else "?"
    line = None
    offset = None
    extra = None
    if event_name == "LINE" and len(args) >= 2:
        line = args[1]
    elif code is not None:
        line = code.co_firstlineno
    if len(args) >= 2 and event_name != "LINE":
        offset = args[1]
    if len(args) > 2:
        extra = args[2:]
    parts = [event_name, f"{filename}:{line}", func]
    if offset is not None:
        parts.append(f"offset={offset}")
    if extra is not None:
        parts.append(f"extra={extra!r}")
    return " ".join(parts)


def main():
    parser = argparse.ArgumentParser(
        description="Trace Python execution using sys.monitoring.",
    )
    parser.add_argument(
        "--include",
        action="append",
        default=[],
        help="Comma-separated event names to include (default: all).",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        help="Comma-separated event names to exclude.",
    )
    parser.add_argument(
        "--list-events",
        action="store_true",
        help="List available event names and exit.",
    )
    parser.add_argument(
        "--output",
        default="-",
        help="Output file (default: stdout).",
    )
    parser.add_argument(
        "script",
        help="Path to the script to run under monitoring.",
    )
    parser.add_argument(
        "script_args",
        nargs=argparse.REMAINDER,
        help="Arguments to pass to the script.",
    )
    args = parser.parse_args()

    event_map = _build_event_map()
    if args.list_events:
        for name in sorted(event_map.keys()):
            print(name)
        return 0

    include = _parse_event_list(args.include)
    exclude = _parse_event_list(args.exclude)
    event_names = _resolve_events(event_map, include, exclude)

    tool_id = 1
    try:
        use_tool_id = getattr(sys.monitoring, "use_tool_id", None)
        if use_tool_id is not None:
            use_tool_id(tool_id, "monitor-trace")
    except Exception as exc:
        raise SystemExit(f"Failed to reserve sys.monitoring tool id {tool_id}: {exc}")

    if args.output == "-":
        log = sys.stdout
    else:
        log = open(args.output, "w", encoding="utf-8")

    def make_callback(name):
        def callback(*cb_args):
            print(_format_event(name, cb_args), file=log, flush=True)
        return callback

    events_mask = 0
    for name in event_names:
        event_id = event_map[name]
        sys.monitoring.register_callback(tool_id, event_id, make_callback(name))
        events_mask |= event_id

    sys.monitoring.set_events(tool_id, events_mask)

    try:
        sys.argv = [args.script] + args.script_args
        runpy.run_path(args.script, run_name="__main__")
    finally:
        sys.monitoring.set_events(tool_id, 0)
        free_tool_id = getattr(sys.monitoring, "free_tool_id", None)
        if free_tool_id is not None:
            free_tool_id(tool_id)
        if log is not sys.stdout:
            log.close()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
