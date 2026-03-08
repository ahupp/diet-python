#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import sys
import tempfile
import traceback
import uuid
import types
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
WEB_DIR = ROOT / "web"

# Ensure local modules (`diet_import_hook`, `__dp__`) are importable.
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

# CLIF rendering must use JIT-enabled transformed execution.
os.environ.setdefault("DIET_PYTHON_MODE", "transform")
os.environ.setdefault("DIET_PYTHON_JIT", "1")

import diet_import_hook  # noqa: E402
diet_import_hook.install()
import __dp__  # noqa: E402


DIET_PYTHON = diet_import_hook._get_pyo3_transform()


def _load_module_from_source(source: str):
    module_name = f"_dp_web_{uuid.uuid4().hex}"
    tmp_path: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".py", delete=False, encoding="utf-8"
        ) as tmp:
            tmp.write(source)
            tmp_path = tmp.name
        transformed_source = DIET_PYTHON.transform_source_with_name(
            source, module_name, True
        )
        module = types.ModuleType(module_name)
        module.__file__ = tmp_path
        module.__name__ = module_name
        module.__package__ = None
        exec(compile(transformed_source, tmp_path, "exec"), module.__dict__)
        init = getattr(module, "_dp_module_init", None)
        if callable(init):
            init()
        return module
    finally:
        if tmp_path is not None:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass
        sys.modules.pop(module_name, None)


def _resolve_entry_callable(module, entry_label: str | None):
    if entry_label:
        direct = getattr(module, entry_label, None)
        if callable(direct):
            return direct, f"module.{entry_label}"

        for name, value in module.__dict__.items():
            if not callable(value):
                continue
            plan_qualname = getattr(value, "__dp_plan_qualname", None)
            if isinstance(plan_qualname, str) and plan_qualname.endswith(
                f"::{entry_label}"
            ):
                return value, f"{name} (plan={plan_qualname})"
            if getattr(value, "__name__", None) == entry_label:
                return value, f"{name} (name={entry_label})"

    init = getattr(module, "_dp_module_init", None)
    if callable(init):
        return init, "_dp_module_init"
    return None, None


def _plan_key_from_callable(entry_callable):
    plan_module = getattr(entry_callable, "__dp_plan_module", None)
    plan_qualname = getattr(entry_callable, "__dp_plan_qualname", None)
    if isinstance(plan_module, str) and isinstance(plan_qualname, str):
        return plan_module, plan_qualname
    module_name = getattr(entry_callable, "__module__", None)
    qualname = getattr(entry_callable, "__qualname__", None)
    if isinstance(module_name, str) and isinstance(qualname, str):
        return module_name, qualname
    raise RuntimeError("entry callable is missing JIT plan metadata")


def _render_clif(source: str, entry_label: str | None):
    module = _load_module_from_source(source)
    entry_callable, resolved = _resolve_entry_callable(module, entry_label)
    if entry_callable is None:
        raise RuntimeError(
            f"could not resolve callable for entry_label={entry_label!r}; "
            "no callable _dp_module_init found"
        )
    plan_module, plan_qualname = _plan_key_from_callable(entry_callable)
    rendered = DIET_PYTHON.jit_render_bb_with_cfg_plan(plan_module, plan_qualname)
    if not isinstance(rendered, dict):
        raise RuntimeError("jit_render_bb_with_cfg_plan() returned non-dict payload")
    clif = rendered.get("clif", "")
    cfg_dot = rendered.get("cfg_dot")
    vcode_disasm = rendered.get("vcode_disasm", "")
    return {
        "clif": clif,
        "cfgDot": cfg_dot,
        "vcodeDisasm": vcode_disasm,
        "resolved_entry": resolved,
    }


class InspectorHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args, directory=None, **kwargs):
        super().__init__(*args, directory=str(WEB_DIR), **kwargs)

    def do_POST(self):
        if self.path != "/api/jit_clif":
            self.send_error(HTTPStatus.NOT_FOUND, "unknown endpoint")
            return

        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self.send_error(HTTPStatus.BAD_REQUEST, "invalid Content-Length")
            return

        try:
            body = self.rfile.read(length)
            payload = json.loads(body.decode("utf-8"))
            source = payload.get("source", "")
            entry_label = payload.get("entryLabel")
            if not isinstance(source, str):
                raise TypeError("source must be a string")
            if entry_label is not None and not isinstance(entry_label, str):
                raise TypeError("entryLabel must be a string when provided")
            result = _render_clif(source, entry_label)
            self._send_json(HTTPStatus.OK, result)
        except Exception as exc:  # noqa: BLE001
            self._send_json(
                HTTPStatus.INTERNAL_SERVER_ERROR,
                {
                    "error": str(exc),
                    "traceback": traceback.format_exc(),
                },
            )

    def _send_json(self, status: HTTPStatus, payload):
        encoded = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def log_message(self, fmt, *args):
        # Keep stdout clean; run script captures stderr/stdout to file.
        return


def main():
    host = os.environ.get("HOST", "127.0.0.1")
    port = int(os.environ.get("PORT", "8000"))
    server = ThreadingHTTPServer((host, port), InspectorHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
