#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any
from zoneinfo import ZoneInfo


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CODEX_ROOT = Path.home() / ".codex"
DEFAULT_TIMEZONE = "America/Los_Angeles"
DEFAULT_WEB_OUTPUT = REPO_ROOT / "web" / "history_metrics_data.json"


@dataclass(frozen=True)
class DailyTokenTotals:
    date: str
    input_tokens: int
    output_tokens: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Read per-commit history metrics JSONL, build a daily rollup, and emit "
            "a JSON bundle for the history metrics web page."
        )
    )
    parser.add_argument(
        "history_jsonl",
        help="Path to the per-commit history JSONL produced by collect_warloc_history.py",
    )
    parser.add_argument(
        "daily_output",
        help="Path to the daily rollup JSONL file",
    )
    parser.add_argument(
        "--web-output",
        default=str(DEFAULT_WEB_OUTPUT),
        help=f"Path to the web JSON bundle (default: {DEFAULT_WEB_OUTPUT})",
    )
    parser.add_argument(
        "--codex-root",
        default=str(DEFAULT_CODEX_ROOT),
        help=f"Root directory for Codex logs (default: {DEFAULT_CODEX_ROOT})",
    )
    parser.add_argument(
        "--timezone",
        default=DEFAULT_TIMEZONE,
        help=f"IANA timezone name for daily grouping (default: {DEFAULT_TIMEZONE})",
    )
    parser.add_argument(
        "--codex-cwd-prefix",
        default=str(REPO_ROOT),
        help="Only count Codex sessions whose cwd starts with this path",
    )
    return parser.parse_args()


def parse_timestamp(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def local_day(value: str, timezone: ZoneInfo) -> str:
    return parse_timestamp(value).astimezone(timezone).date().isoformat()


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as fh:
        for raw_line in fh:
            line = raw_line.strip()
            if not line:
                continue
            payload = json.loads(line)
            if not isinstance(payload, dict):
                raise RuntimeError(f"expected JSON object in {path}, got {type(payload)!r}")
            records.append(payload)
    return records


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as fh:
        for record in records:
            fh.write(json.dumps(record, sort_keys=True))
            fh.write("\n")


def build_daily_rollup(commit_records: list[dict[str, Any]], timezone: ZoneInfo) -> list[dict[str, Any]]:
    sorted_records = sorted(commit_records, key=lambda record: parse_timestamp(record["timestamp"]))
    per_day: dict[str, dict[str, Any]] = {}
    for record in sorted_records:
        date = local_day(record["timestamp"], timezone)
        day_rollup = per_day.setdefault(
            date,
            {
                "date": date,
                "code_lines": 0,
                "tests_python_total_lines": 0,
                "daily_churn": 0,
            },
        )
        day_rollup["code_lines"] = int(record["code_lines"])
        day_rollup["tests_python_total_lines"] = int(record["tests_python_total_lines"])
        day_rollup["daily_churn"] += int(record["lines_changed"])
    return [per_day[date] for date in sorted(per_day)]


def iter_token_events(session_path: Path, repo_cwd_prefix: str) -> list[tuple[str, int, int]]:
    cwd_matches = repo_cwd_prefix == ""
    previous_input = 0
    previous_output = 0
    token_events: list[tuple[str, int, int]] = []
    with session_path.open("r", encoding="utf-8") as fh:
        for raw_line in fh:
            line = raw_line.strip()
            if not line:
                continue
            payload = json.loads(line)
            if not isinstance(payload, dict):
                continue
            record_type = payload.get("type")
            if record_type == "session_meta":
                session_meta = payload.get("payload", {})
                if not isinstance(session_meta, dict):
                    continue
                session_cwd = session_meta.get("cwd")
                cwd_matches = isinstance(session_cwd, str) and session_cwd.startswith(repo_cwd_prefix)
                continue
            if not cwd_matches or record_type != "event_msg":
                continue
            event_payload = payload.get("payload", {})
            if not isinstance(event_payload, dict) or event_payload.get("type") != "token_count":
                continue
            info = event_payload.get("info", {})
            if not isinstance(info, dict):
                continue
            usage = info.get("total_token_usage", {})
            if not isinstance(usage, dict):
                continue
            input_tokens = usage.get("input_tokens")
            output_tokens = usage.get("output_tokens")
            timestamp = payload.get("timestamp")
            if not isinstance(input_tokens, int) or not isinstance(output_tokens, int) or not isinstance(timestamp, str):
                continue
            delta_input = input_tokens - previous_input if input_tokens >= previous_input else input_tokens
            delta_output = output_tokens - previous_output if output_tokens >= previous_output else output_tokens
            previous_input = input_tokens
            previous_output = output_tokens
            token_events.append((timestamp, delta_input, delta_output))
    return token_events


def collect_daily_tokens(codex_root: Path, timezone: ZoneInfo, repo_cwd_prefix: str) -> list[dict[str, Any]]:
    sessions_root = codex_root / "sessions"
    if not sessions_root.is_dir():
        return []

    totals: dict[str, DailyTokenTotals] = {}
    for session_path in sorted(sessions_root.rglob("*.jsonl")):
        for timestamp, delta_input, delta_output in iter_token_events(session_path, repo_cwd_prefix):
            date = local_day(timestamp, timezone)
            current = totals.get(date)
            if current is None:
                totals[date] = DailyTokenTotals(
                    date=date,
                    input_tokens=delta_input,
                    output_tokens=delta_output,
                )
                continue
            totals[date] = DailyTokenTotals(
                date=date,
                input_tokens=current.input_tokens + delta_input,
                output_tokens=current.output_tokens + delta_output,
            )
    return [
        {
            "date": totals[date].date,
            "input_tokens": totals[date].input_tokens,
            "output_tokens": totals[date].output_tokens,
        }
        for date in sorted(totals)
    ]


def build_web_bundle(
    daily_rollup: list[dict[str, Any]],
    daily_tokens: list[dict[str, Any]],
    timezone_name: str,
    history_path: Path,
    codex_root: Path,
    repo_cwd_prefix: str,
) -> dict[str, Any]:
    return {
        "generated_at": datetime.now().astimezone().isoformat(),
        "timezone": timezone_name,
        "history_jsonl": str(history_path),
        "codex_root": str(codex_root),
        "codex_cwd_prefix": repo_cwd_prefix,
        "daily_rollup": daily_rollup,
        "daily_tokens": daily_tokens,
    }


def main() -> int:
    args = parse_args()
    history_path = Path(args.history_jsonl).resolve()
    daily_output_path = Path(args.daily_output).resolve()
    web_output_path = Path(args.web_output).resolve()
    codex_root = Path(args.codex_root).expanduser().resolve()
    timezone = ZoneInfo(args.timezone)
    commit_records = load_jsonl(history_path)
    daily_rollup = build_daily_rollup(commit_records, timezone)
    daily_tokens = collect_daily_tokens(codex_root, timezone, args.codex_cwd_prefix)
    write_jsonl(daily_output_path, daily_rollup)
    web_output_path.parent.mkdir(parents=True, exist_ok=True)
    web_output_path.write_text(
        json.dumps(
            build_web_bundle(
                daily_rollup=daily_rollup,
                daily_tokens=daily_tokens,
                timezone_name=args.timezone,
                history_path=history_path,
                codex_root=codex_root,
                repo_cwd_prefix=args.codex_cwd_prefix,
            ),
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"wrote {len(daily_rollup)} daily rollup rows to {daily_output_path}")
    print(f"wrote web metrics bundle to {web_output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
