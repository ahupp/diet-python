#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path
import sys

_VALIDATE_DELIMITER = "# diet-python: validate"


def split_source(path: Path) -> str:
    source = path.read_text(encoding="utf-8")
    if _VALIDATE_DELIMITER in source:
        source = source.split(_VALIDATE_DELIMITER, 1)[0]
    return source.rstrip() + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Render JIT CLIF for a registered BB plan using the "
            "diet_python.jit_render_bb_with_cfg_plan helper."
        )
    )
    parser.add_argument("source", help="Python source file to transform/register")
    parser.add_argument("function_id", type=int, help="Registered BB function id to render")
    parser.add_argument(
        "--module-name",
        help="Module name to register plans under (default: source file stem)",
    )
    parser.add_argument(
        "--cfg-dot-out",
        help="Optional path to write the rendered Graphviz CFG dot output",
    )
    parser.add_argument(
        "--debug-plan",
        action="store_true",
        help="Print the debug plan before rendering CLIF",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    source_path = Path(args.source).resolve()
    module_name = args.module_name or source_path.stem
    source = split_source(source_path)

    try:
        import diet_python
    except Exception as exc:
        print(f"failed to import diet_python: {exc}", file=sys.stderr)
        return 1

    try:
        diet_python.transform_source_with_name(source, module_name, True)
    except Exception as exc:
        print(f"failed to transform/register {source_path}: {exc}", file=sys.stderr)
        return 1

    if args.debug_plan:
        try:
            print(diet_python.jit_debug_plan(module_name, args.function_id), file=sys.stderr)
        except Exception as exc:
            print(
                f"failed to dump debug plan for {module_name}.fn#{args.function_id}: {exc}",
                file=sys.stderr,
            )
            return 1

    try:
        rendered = diet_python.jit_render_bb_with_cfg_plan(module_name, args.function_id)
    except Exception as exc:
        print(
            f"failed to render CLIF for {module_name}.fn#{args.function_id}: {exc}",
            file=sys.stderr,
        )
        return 1

    clif = rendered.get("clif")
    cfg_dot = rendered.get("cfg_dot")
    if not isinstance(clif, str) or not isinstance(cfg_dot, str):
        print(
            f"unexpected render payload for {module_name}.fn#{args.function_id}: {type(rendered)!r}",
            file=sys.stderr,
        )
        return 1

    if args.cfg_dot_out:
        Path(args.cfg_dot_out).write_text(cfg_dot, encoding="utf-8")

    sys.stdout.write(clif)
    if clif and not clif.endswith("\n"):
        sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
