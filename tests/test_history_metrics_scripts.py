from __future__ import annotations

import importlib.util
import json
import sys
from contextlib import contextmanager
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


@contextmanager
def argv_context(argv: list[str]):
    original_argv = sys.argv[:]
    sys.argv = argv
    try:
        yield
    finally:
        sys.argv = original_argv


def test_collect_history_parse_args_defaults_to_current_workspace_ancestors():
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history_parse_args")
    with argv_context(["collect_warloc_history.py", "out.jsonl"]):
        args = module.parse_args()
    assert args.output == "out.jsonl"
    assert args.revset == "..@"


def test_git_non_vendor_history_uses_path_limited_git_rev_list(monkeypatch):
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history_git_history")
    observed: dict[str, object] = {}

    def fake_run(cmd, *, cwd, capture_output=False):
        observed["cmd"] = cmd
        observed["cwd"] = cwd
        observed["capture_output"] = capture_output

        class Result:
            stdout = "abc123\nfed456\n"

        return Result()

    monkeypatch.setattr(module, "run", fake_run)

    assert module.git_non_vendor_history("base123") == ["abc123", "fed456"]
    assert observed["cmd"] == ["git", "rev-list", "--reverse", "base123", "--", ".", ":(exclude)vendor"]
    assert observed["cwd"] == REPO_ROOT
    assert observed["capture_output"] is True


def test_list_commits_default_revset_uses_current_line_non_vendor_history(monkeypatch):
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history_default_list")
    base_commit = module.CommitMetadata(
        commit_id="base123",
        change_id="basechange",
        timestamp="2026-03-25T00:00:00+00:00",
        description="base",
    )
    git_commit = module.CommitMetadata(
        commit_id="git123",
        change_id="gitchange",
        timestamp="2026-03-25T01:00:00+00:00",
        description="git",
    )
    local_commit = module.CommitMetadata(
        commit_id="local123",
        change_id="localchange",
        timestamp="2026-03-25T02:00:00+00:00",
        description="local",
    )
    observed: dict[str, object] = {}

    def fake_current_line_git_base_commit():
        observed["base_called"] = True
        return base_commit

    def fake_git_non_vendor_history(head_revision):
        observed["git_head_revision"] = head_revision
        return ["git123"]

    def fake_commit_metadata_for_revision(revision):
        observed.setdefault("git_revisions", []).append(revision)
        assert revision == "git123"
        return git_commit

    def fake_list_jj_commits(revset, *, allow_empty=False):
        observed["local_revset"] = revset
        observed["local_allow_empty"] = allow_empty
        assert revset == "base123::@ ~ base123"
        assert allow_empty is True
        return [local_commit]

    def fake_filter_commits_to_non_vendor_changes(commits, *, allow_empty=False):
        observed["filtered_commits"] = commits
        observed["filtered_allow_empty"] = allow_empty
        return commits

    monkeypatch.setattr(module, "current_line_git_base_commit", fake_current_line_git_base_commit)
    monkeypatch.setattr(module, "git_non_vendor_history", fake_git_non_vendor_history)
    monkeypatch.setattr(module, "commit_metadata_for_revision", fake_commit_metadata_for_revision)
    monkeypatch.setattr(module, "list_jj_commits", fake_list_jj_commits)
    monkeypatch.setattr(module, "filter_commits_to_non_vendor_changes", fake_filter_commits_to_non_vendor_changes)

    assert module.list_commits(module.DEFAULT_REVSET) == [git_commit, local_commit]
    assert observed["base_called"] is True
    assert observed["git_head_revision"] == "base123"
    assert observed["git_revisions"] == ["git123"]
    assert observed["filtered_commits"] == [local_commit]
    assert observed["filtered_allow_empty"] is True


def test_list_commits_non_default_revset_filters_non_vendor_changes(monkeypatch):
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history_custom_revset")
    commits = [
        module.CommitMetadata("commit1", "change1", "2026-03-25T00:00:00+00:00", "one"),
        module.CommitMetadata("commit2", "change2", "2026-03-25T01:00:00+00:00", "two"),
    ]
    observed: dict[str, object] = {}

    def fake_list_jj_commits(revset, *, allow_empty=False):
        observed["revset"] = revset
        observed["allow_empty"] = allow_empty
        return commits

    def fake_filter_commits_to_non_vendor_changes(input_commits, *, allow_empty=False):
        observed["input_commits"] = input_commits
        observed["filter_allow_empty"] = allow_empty
        return [commits[1]]

    monkeypatch.setattr(module, "list_jj_commits", fake_list_jj_commits)
    monkeypatch.setattr(module, "filter_commits_to_non_vendor_changes", fake_filter_commits_to_non_vendor_changes)

    assert module.list_commits("custom") == [commits[1]]
    assert observed["revset"] == "custom"
    assert observed["allow_empty"] is False
    assert observed["input_commits"] == commits
    assert observed["filter_allow_empty"] is False


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


def test_warloc_total_from_by_file_jsonl_ignores_vendor_files():
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history_warloc")
    output = "\n".join(
        [
            json.dumps(
                {
                    "scope": "file",
                    "file": "./dp-transform/src/lib.rs",
                    "file_count": 1,
                    "code_lines": 10,
                    "test_lines": 2,
                    "blank_lines": 3,
                    "comment_lines": 4,
                }
            ),
            json.dumps(
                {
                    "scope": "file",
                    "file": "./vendor/ruff/src/lib.rs",
                    "file_count": 1,
                    "code_lines": 500,
                    "test_lines": 600,
                    "blank_lines": 700,
                    "comment_lines": 800,
                }
            ),
            json.dumps(
                {
                    "scope": "total",
                    "file_count": 2,
                    "code_lines": 510,
                    "test_lines": 602,
                    "blank_lines": 703,
                    "comment_lines": 804,
                }
            ),
        ]
    )
    assert module.warloc_total_from_by_file_jsonl(output) == {
        "scope": "total",
        "file_count": 1,
        "code_lines": 10,
        "test_lines": 2,
        "blank_lines": 3,
        "comment_lines": 4,
    }


def test_warloc_total_from_by_file_jsonl_handles_summary_only_output():
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history_warloc_summary_only")
    output = json.dumps(
        {
            "scope": "total",
            "file_count": 0,
            "code_lines": 0,
            "test_lines": 0,
            "blank_lines": 0,
            "comment_lines": 0,
        }
    )
    assert module.warloc_total_from_by_file_jsonl(output) == {
        "scope": "total",
        "file_count": 0,
        "code_lines": 0,
        "test_lines": 0,
        "blank_lines": 0,
        "comment_lines": 0,
    }


def test_restore_workspace_from_commit_uses_jj_restore(monkeypatch, tmp_path: Path):
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history_restore")
    observed: dict[str, object] = {}

    def fake_jj_cmd(*args, ignore_working_copy=False):
        observed["args"] = args
        observed["ignore_working_copy"] = ignore_working_copy
        return ["jj", *args]

    def fake_run(cmd, *, cwd, capture_output=False):
        observed["cmd"] = cmd
        observed["cwd"] = cwd
        observed["capture_output"] = capture_output
        return object()

    monkeypatch.setattr(module, "jj_cmd", fake_jj_cmd)
    monkeypatch.setattr(module, "run", fake_run)

    module.restore_workspace_from_commit(tmp_path, "abc123")

    assert observed["args"] == ("restore", "--from", "abc123")
    assert observed["ignore_working_copy"] is False
    assert observed["cmd"] == ["jj", "restore", "--from", "abc123"]
    assert observed["cwd"] == tmp_path
    assert observed["capture_output"] is True


def test_lines_changed_for_commit_uses_non_vendor_fileset(monkeypatch):
    module = load_module(REPO_ROOT / "scripts" / "collect_warloc_history.py", "collect_warloc_history_lines_changed")
    observed: dict[str, object] = {}

    def fake_jj_cmd(*args, ignore_working_copy=False):
        observed["args"] = args
        observed["ignore_working_copy"] = ignore_working_copy
        return ["jj", *args]

    def fake_run(cmd, *, cwd, capture_output=False):
        observed["cmd"] = cmd
        observed["cwd"] = cwd
        observed["capture_output"] = capture_output

        class Result:
            stdout = "0 files changed, 0 insertions(+), 0 deletions(-)\n"

        return Result()

    monkeypatch.setattr(module, "jj_cmd", fake_jj_cmd)
    monkeypatch.setattr(module, "run", fake_run)

    assert module.lines_changed_for_commit("abc123") == 0
    assert observed["args"] == ("diff", "-r", "abc123", "--stat", "~vendor")
    assert observed["ignore_working_copy"] is True
    assert observed["cmd"] == ["jj", "diff", "-r", "abc123", "--stat", "~vendor"]
    assert observed["cwd"] == REPO_ROOT
    assert observed["capture_output"] is True


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
    diet_root = str(REPO_ROOT.parent / "diet-python")
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
    diet_session_path = sessions_dir / "rollout-diet-python.jsonl"
    diet_session_path.write_text(
        "\n".join(
            [
                json.dumps(
                    {
                        "timestamp": "2026-03-25T17:00:00Z",
                        "type": "session_meta",
                        "payload": {"cwd": diet_root},
                    }
                ),
                json.dumps(
                    {
                        "timestamp": "2026-03-25T17:02:00Z",
                        "type": "event_msg",
                        "payload": {
                            "type": "token_count",
                            "info": {"total_token_usage": {"input_tokens": 20, "output_tokens": 5}},
                        },
                    }
                ),
                json.dumps(
                    {
                        "timestamp": "2026-03-25T17:03:00Z",
                        "type": "event_msg",
                        "payload": {
                            "type": "token_count",
                            "info": {"total_token_usage": {"input_tokens": 35, "output_tokens": 9}},
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
        repo_cwd_prefixes=[repo_root, diet_root],
    )
    assert totals == [
        {
            "date": "2026-03-25",
            "input_tokens": 195,
            "output_tokens": 49,
        }
    ]


def test_write_static_report_emits_interactive_html(tmp_path: Path):
    module = load_module(REPO_ROOT / "scripts" / "build_history_metrics_rollup.py", "build_history_metrics_rollup_static")
    html_output = tmp_path / "history_metrics_smoke.html"
    module.write_static_report(
        html_output_path=html_output,
        template_path=REPO_ROOT / "web" / "history_metrics_template.html",
        generated_at="2026-03-25T10:00:00-07:00",
        timezone_name="America/Los_Angeles",
        history_path=REPO_ROOT / "logs" / "warloc_history.jsonl",
        codex_root=Path.home() / ".codex",
        repo_cwd_prefixes=[str(REPO_ROOT), str(REPO_ROOT.parent / "diet-python")],
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
    assert '<script id="history-metrics-data" type="application/json">' in html
    assert 'id="loc-chart"' in html
    assert 'id="churn-chart"' in html
    assert 'id="tokens-chart"' in html
    assert 'id="zoom-reset"' in html
    assert "Brush any chart to zoom the full stack." in html
    assert 'renderAllCharts();' in html
    assert "123" in html
    assert "2026-03-25" in html
    assert "300" in html
    assert str(REPO_ROOT) in html
    assert str(REPO_ROOT.parent / "diet-python") in html
    assert "history_metrics_smoke_loc.svg" not in html
    assert "history_metrics_smoke_churn.svg" not in html
    assert "history_metrics_smoke_tokens.svg" not in html
    assert not (tmp_path / "history_metrics_smoke_loc.svg").exists()
    assert not (tmp_path / "history_metrics_smoke_churn.svg").exists()
    assert not (tmp_path / "history_metrics_smoke_tokens.svg").exists()
