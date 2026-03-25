from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def load_module(path: Path, module_name: str):
    spec = importlib.util.spec_from_file_location(module_name, path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


def test_parse_lines_changed_from_stat_handles_missing_sides():
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history")
    stat_output = "\n".join(
        [
            "script.py | 3 ++-",
            "1 file changed, 2 insertions(+), 1 deletion(-)",
        ]
    )
    assert module.parse_lines_changed_from_stat(stat_output) == 3
    assert module.parse_lines_changed_from_stat("1 file changed, 7 insertions(+)") == 7
    assert module.parse_lines_changed_from_stat("1 file changed, 4 deletions(-)") == 4


def test_build_daily_rollup_uses_end_of_day_snapshot():
    module = load_module(REPO_ROOT / "scripts" / "build_history_metrics_rollup.py", "build_history_metrics_rollup")
    timezone = module.ZoneInfo("America/Los_Angeles")
    commit_records = [
        {
            "timestamp": "2026-03-24T18:10:00+00:00",
            "code_lines": 120,
            "tests_python_total_lines": 18,
            "lines_changed": 4,
        },
        {
            "timestamp": "2026-03-24T23:55:00+00:00",
            "code_lines": 133,
            "tests_python_total_lines": 21,
            "lines_changed": 9,
        },
        {
            "timestamp": "2026-03-25T20:00:00+00:00",
            "code_lines": 150,
            "tests_python_total_lines": 24,
            "lines_changed": 6,
        },
    ]
    assert module.build_daily_rollup(commit_records, timezone) == [
        {
            "date": "2026-03-24",
            "code_lines": 133,
            "tests_python_total_lines": 21,
            "daily_churn": 13,
        },
        {
            "date": "2026-03-25",
            "code_lines": 150,
            "tests_python_total_lines": 24,
            "daily_churn": 6,
        },
    ]


def test_collect_daily_tokens_uses_repo_local_session_deltas(tmp_path: Path):
    module = load_module(REPO_ROOT / "scripts" / "build_history_metrics_rollup.py", "build_history_metrics_rollup_tokens")
    sessions_dir = tmp_path / "sessions" / "2026" / "03" / "25"
    sessions_dir.mkdir(parents=True)
    session_path = sessions_dir / "rollout-example.jsonl"
    repo_root = str(REPO_ROOT)
    other_root = "/tmp/other-repo"
    session_path.write_text(
        "\n".join(
            [
                json.dumps(
                    {
                        "timestamp": "2026-03-25T16:00:00Z",
                        "type": "session_meta",
                        "payload": {"cwd": repo_root},
                    }
                ),
                json.dumps(
                    {
                        "timestamp": "2026-03-25T16:02:00Z",
                        "type": "event_msg",
                        "payload": {
                            "type": "token_count",
                            "info": {"total_token_usage": {"input_tokens": 100, "output_tokens": 25}},
                        },
                    }
                ),
                json.dumps(
                    {
                        "timestamp": "2026-03-25T16:03:00Z",
                        "type": "event_msg",
                        "payload": {
                            "type": "token_count",
                            "info": {"total_token_usage": {"input_tokens": 160, "output_tokens": 40}},
                        },
                    }
                ),
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    other_session_path = sessions_dir / "rollout-other.jsonl"
    other_session_path.write_text(
        "\n".join(
            [
                json.dumps(
                    {
                        "timestamp": "2026-03-25T16:00:00Z",
                        "type": "session_meta",
                        "payload": {"cwd": other_root},
                    }
                ),
                json.dumps(
                    {
                        "timestamp": "2026-03-25T16:05:00Z",
                        "type": "event_msg",
                        "payload": {
                            "type": "token_count",
                            "info": {"total_token_usage": {"input_tokens": 999, "output_tokens": 999}},
                        },
                    }
                ),
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    totals = module.collect_daily_tokens(
        codex_root=tmp_path,
        timezone=module.ZoneInfo("America/Los_Angeles"),
        repo_cwd_prefix=repo_root,
    )
    assert totals == [
        {
            "date": "2026-03-25",
            "input_tokens": 160,
            "output_tokens": 40,
        }
    ]


def test_write_static_report_emits_html_and_svgs(tmp_path: Path):
    module = load_module(REPO_ROOT / "scripts" / "build_history_metrics_rollup.py", "build_history_metrics_rollup_static")
    html_output = tmp_path / "history_metrics_smoke.html"
    module.write_static_report(
        html_output_path=html_output,
        template_path=REPO_ROOT / "web" / "history_metrics_template.html",
        generated_at="2026-03-25T10:00:00-07:00",
        timezone_name="America/Los_Angeles",
        history_path=REPO_ROOT / "logs" / "warloc_history.jsonl",
        codex_root=Path.home() / ".codex",
        repo_cwd_prefix=str(REPO_ROOT),
        daily_rollup=[
            {
                "date": "2026-03-25",
                "code_lines": 123,
                "tests_python_total_lines": 45,
                "daily_churn": 12,
            }
        ],
        daily_tokens=[
            {
                "date": "2026-03-25",
                "input_tokens": 300,
                "output_tokens": 40,
            }
        ],
    )

    html = html_output.read_text(encoding="utf-8")
    assert "<script" not in html
    assert 'src="history_metrics_smoke_loc.svg"' in html
    assert 'src="history_metrics_smoke_churn.svg"' in html
    assert 'src="history_metrics_smoke_tokens.svg"' in html
    assert "123" in html
    assert (tmp_path / "history_metrics_smoke_loc.svg").read_text(encoding="utf-8").startswith("<svg")
    assert (tmp_path / "history_metrics_smoke_churn.svg").read_text(encoding="utf-8").startswith("<svg")
    assert (tmp_path / "history_metrics_smoke_tokens.svg").read_text(encoding="utf-8").startswith("<svg")
