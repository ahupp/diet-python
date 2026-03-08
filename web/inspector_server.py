#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import sys
import traceback
import uuid
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


def _register_plans_from_source(source: str) -> str:
    module_name = f"_dp_web_{uuid.uuid4().hex}"
    DIET_PYTHON.transform_source_with_name(source, module_name, True)
    return module_name


def _render_clif(source: str, qualname: str | None, entry_label: str | None):
    plan_module = _register_plans_from_source(source)
    if qualname is None:
        qualname = "_dp_module_init"
    if entry_label is None:
        entry_label = "_dp_bb__dp_module_init_start"
    plan_qualname = f"{qualname}::{entry_label}"
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
        "resolved_entry": plan_qualname,
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
            qualname = payload.get("qualname")
            entry_label = payload.get("entryLabel")
            if not isinstance(source, str):
                raise TypeError("source must be a string")
            if qualname is not None and not isinstance(qualname, str):
                raise TypeError("qualname must be a string when provided")
            if entry_label is not None and not isinstance(entry_label, str):
                raise TypeError("entryLabel must be a string when provided")
            result = _render_clif(source, qualname, entry_label)
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
