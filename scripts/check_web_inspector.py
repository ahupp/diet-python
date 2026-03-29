#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
import threading
import urllib.request
from http.server import ThreadingHTTPServer
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from web import inspector_server


class FakeTransform:
    def __init__(self) -> None:
        self.last_registered_module: str | None = None
        self.last_registered_source: str | None = None
        self.last_render_request: tuple[str, int] | None = None

    def transform_source_with_name(
        self, source: str, module_name: str, ensure: bool
    ) -> str:
        assert ensure is True
        self.last_registered_module = module_name
        self.last_registered_source = source
        return source

    def inspect_pipeline(self, source: str, ensure: bool) -> str:
        assert ensure is True
        return json.dumps(
            {
                "steps": [
                    {
                        "key": "input_source",
                        "label": "input source",
                        "text": source,
                    },
                    {
                        "key": "semantic_blockpy",
                        "label": "semantic_blockpy",
                        "text": "function classify(n):\n    return n\n",
                    },
                ],
                "functions": [
                    {
                        "functionId": 7,
                        "qualname": "classify",
                        "displayName": "classify",
                        "bindName": "classify",
                        "kind": "function",
                        "entryLabel": "_dp_bb_0_0",
                    }
                ],
            }
        )

    def jit_render_bb_with_cfg_plan(
        self, module_name: str, function_id: int
    ) -> dict[str, str]:
        self.last_render_request = (module_name, function_id)
        return {
            "clif": "function u0:0(i64) -> i64 {\nblock0(v0: i64):\n    return v0\n}\n",
            "cfg_dot": "digraph cfg { entry -> exit; }",
            "vcode_disasm": "v0 = copy v1",
        }


def read_url(url: str, *, data: bytes | None = None) -> tuple[int, str]:
    request = urllib.request.Request(url, data=data)
    if data is not None:
        request.add_header("Content-Type", "application/json")
        request.method = "POST"
    with urllib.request.urlopen(request, timeout=5) as response:
        return response.status, response.read().decode("utf-8")


def main() -> int:
    fake = FakeTransform()
    original_get_transform = inspector_server._get_transform
    inspector_server._get_transform = lambda: fake

    server = ThreadingHTTPServer(("127.0.0.1", 0), inspector_server.InspectorHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    try:
        base_url = f"http://127.0.0.1:{server.server_port}"
        status, index_html = read_url(f"{base_url}/")
        assert status == 200
        assert "/api/inspect_pipeline" in index_html
        assert "/api/jit_clif" in index_html
        assert "./pkg/diet_python.js" not in index_html
        assert "inspectPipelineWasm" not in index_html
        assert "await init()" not in index_html

        source = "def classify(n):\n    return n\n"
        status, inspect_body = read_url(
            f"{base_url}/api/inspect_pipeline",
            data=json.dumps({"source": source}).encode("utf-8"),
        )
        assert status == 200
        inspect_payload = json.loads(inspect_body)
        assert inspect_payload["steps"][0]["key"] == "input_source"
        assert inspect_payload["functions"][0]["qualname"] == "classify"

        status, clif_body = read_url(
            f"{base_url}/api/jit_clif",
            data=json.dumps(
                {
                    "source": source,
                    "functionId": 7,
                    "qualname": "classify",
                    "entryLabel": "_dp_bb_0_0",
                }
            ).encode("utf-8"),
        )
        assert status == 200
        clif_payload = json.loads(clif_body)
        assert "function u0:0" in clif_payload["clif"]
        assert clif_payload["resolved_entry"].startswith("classify::__dp_fn_7")
        assert fake.last_registered_source == source
        assert fake.last_registered_module is not None
        assert fake.last_render_request == (fake.last_registered_module, 7)
        print("web inspector server-backed smoke check passed")
        return 0
    finally:
        inspector_server._get_transform = original_get_transform
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
