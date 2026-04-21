#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


OK_STATUS = "ok"
HEAD_UNAVAILABLE_STATUS = "head-unavailable"
BASE_UNAVAILABLE_STATUS = "base-unavailable"
BOTH_UNAVAILABLE_STATUS = "both-unavailable"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare tracked compute-unit profiles for base and head revisions."
    )
    parser.add_argument("--policy-file", required=True, type=Path)
    parser.add_argument("--base-dir", required=True, type=Path)
    parser.add_argument("--head-dir", required=True, type=Path)
    parser.add_argument("--markdown-output", required=True, type=Path)
    parser.add_argument("--json-output", required=True, type=Path)
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def load_profile(directory: Path, program: str) -> dict[str, Any]:
    path = directory / f"{program}.json"
    if not path.is_file():
        raise FileNotFoundError(f"missing profile for {program}: {path}")

    return load_json(path)


def load_optional_manifest(directory: Path) -> dict[str, Any]:
    path = directory / "manifest.json"
    if not path.is_file():
        return {}

    return load_json(path)


def resolve_profile_state(
    directory: Path,
    manifest: dict[str, Any],
    program: str,
) -> dict[str, Any]:
    profile_path = directory / f"{program}.json"
    manifest_result = manifest.get("results", {}).get(program, {})

    if profile_path.is_file():
        return {
            "status": OK_STATUS,
            "path": profile_path,
            "detail": manifest_result.get("detail", "profile available"),
        }

    return {
        "status": manifest_result.get("status", "unavailable"),
        "path": profile_path,
        "detail": manifest_result.get(
            "detail",
            f"profile unavailable at {profile_path}",
        ),
    }


def delta_percent(base_value: int, head_value: int) -> float:
    if base_value == 0:
        return 0.0 if head_value == 0 else 100.0

    return ((head_value - base_value) / base_value) * 100.0


def classify(delta_cu: int, delta_pct: float, policy: dict[str, Any]) -> str:
    if delta_cu < 0:
        return "improved"

    if delta_cu == 0:
        return "unchanged"

    fail = policy["fail"]
    if delta_cu >= fail["deltaCu"] and delta_pct >= fail["deltaPercent"]:
        return "fail"

    warn = policy["warn"]
    if delta_cu >= warn["deltaCu"] and delta_pct >= warn["deltaPercent"]:
        return "warn"

    return "within-policy"


def format_int(value: int) -> str:
    return f"{value:,}"


def format_signed_int(value: int) -> str:
    return f"{value:+,}"


def format_percent(value: float) -> str:
    return f"{value:+.1f}%"


def status_label(status: str) -> str:
    labels = {
        "fail": "❌ fail",
        "warn": "⚠️ warn",
        "improved": "✅ improved",
        "unchanged": "➖ unchanged",
        "within-policy": "✅ within policy",
    }
    return labels[status]


def render_markdown(
    policy: dict[str, Any],
    comparisons: list[dict[str, Any]],
    skipped: list[str],
    hard_errors: list[str],
) -> str:
    fail_count = sum(1 for item in comparisons if item["status"] == "fail")
    warn_count = sum(1 for item in comparisons if item["status"] == "warn")
    improved_count = sum(1 for item in comparisons if item["status"] == "improved")

    tracked_programs = ", ".join(f"`{program}`" for program in policy["trackedPrograms"])
    warn = policy["warn"]
    fail = policy["fail"]

    lines = [
        "## Compute-unit regression report",
        "",
        f"Tracked programs: {tracked_programs}",
        "",
        "Policy:",
        f"- warn when `total_cu` increases by at least +{warn['deltaCu']} CU and +{warn['deltaPercent']:.1f}%",
        f"- fail when `total_cu` increases by at least +{fail['deltaCu']} CU and +{fail['deltaPercent']:.1f}%",
        "- decreases and smaller increases are informational",
        "- values come from `pina profile` static SBF estimates, not runtime validator traces",
        "",
        "Summary:",
        f"- compared programs: {len(comparisons)}",
        f"- failures: {fail_count}",
        f"- warnings: {warn_count}",
        f"- improvements: {improved_count}",
        f"- skipped programs: {len(skipped)}",
        f"- head availability errors: {len(hard_errors)}",
        "",
    ]

    if comparisons:
        lines.extend(
            [
                "| Program | Base CU | Head CU | Delta | Delta % | Status |",
                "| ------- | ------: | ------: | ----: | ------: | ------ |",
            ]
        )

        for item in comparisons:
            lines.append(
                "| {program} | {base} | {head} | {delta} | {delta_pct} | {status} |".format(
                    program=f"`{item['program']}`",
                    base=format_int(item["baseTotalCu"]),
                    head=format_int(item["headTotalCu"]),
                    delta=format_signed_int(item["deltaCu"]),
                    delta_pct=format_percent(item["deltaPercent"]),
                    status=status_label(item["status"]),
                )
            )
    else:
        lines.append("No tracked programs produced comparable base/head profiles.")

    if skipped:
        lines.extend(["", "Skipped programs:"])
        lines.extend(f"- {item}" for item in skipped)

    if hard_errors:
        lines.extend(["", "Head availability errors:"])
        lines.extend(f"- {item}" for item in hard_errors)

    lines.extend(
        [
            "",
            "The JSON artifact also includes text-section, syscall, and binary-size deltas for each compared tracked program.",
            "",
        ]
    )

    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    comparisons: list[dict[str, Any]] = []
    skipped: list[str] = []
    hard_errors: list[str] = []

    try:
        policy = load_json(args.policy_file)
        base_manifest = load_optional_manifest(args.base_dir)
        head_manifest = load_optional_manifest(args.head_dir)

        for program in policy["trackedPrograms"]:
            base_state = resolve_profile_state(args.base_dir, base_manifest, program)
            head_state = resolve_profile_state(args.head_dir, head_manifest, program)

            if base_state["status"] != OK_STATUS and head_state["status"] != OK_STATUS:
                skipped.append(
                    f"`{program}` skipped because base and head profiles were unavailable "
                    f"({base_state['detail']}; {head_state['detail']})"
                )
                continue

            if base_state["status"] != OK_STATUS:
                skipped.append(
                    f"`{program}` skipped because the base profile was unavailable "
                    f"({base_state['detail']})"
                )
                continue

            if head_state["status"] != OK_STATUS:
                hard_errors.append(
                    f"`{program}` produced a base profile but not a head profile "
                    f"({head_state['detail']})"
                )
                continue

            base = load_profile(args.base_dir, program)
            head = load_profile(args.head_dir, program)

            base_total_cu = int(base["total_cu"])
            head_total_cu = int(head["total_cu"])
            delta_cu = head_total_cu - base_total_cu
            delta_pct = delta_percent(base_total_cu, head_total_cu)
            status = classify(delta_cu, delta_pct, policy)

            comparisons.append(
                {
                    "program": program,
                    "status": status,
                    "baseTotalCu": base_total_cu,
                    "headTotalCu": head_total_cu,
                    "deltaCu": delta_cu,
                    "deltaPercent": delta_pct,
                    "baseBinarySize": int(base["binary_size"]),
                    "headBinarySize": int(head["binary_size"]),
                    "deltaBinarySize": int(head["binary_size"]) - int(base["binary_size"]),
                    "baseTextSize": int(base["text_size"]),
                    "headTextSize": int(head["text_size"]),
                    "deltaTextSize": int(head["text_size"]) - int(base["text_size"]),
                    "baseTotalSyscalls": int(base["total_syscalls"]),
                    "headTotalSyscalls": int(head["total_syscalls"]),
                    "deltaTotalSyscalls": int(head["total_syscalls"])
                    - int(base["total_syscalls"]),
                }
            )
    except (FileNotFoundError, KeyError, ValueError, json.JSONDecodeError) as error:
        print(f"Error: {error}", file=sys.stderr)
        return 1

    markdown = render_markdown(policy, comparisons, skipped, hard_errors)

    args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
    args.json_output.parent.mkdir(parents=True, exist_ok=True)

    args.markdown_output.write_text(markdown, encoding="utf-8")
    args.json_output.write_text(
        json.dumps(
            {
                "policy": policy,
                "summary": {
                    "comparedPrograms": len(comparisons),
                    "failures": sum(1 for item in comparisons if item["status"] == "fail"),
                    "warnings": sum(1 for item in comparisons if item["status"] == "warn"),
                    "improvements": sum(
                        1 for item in comparisons if item["status"] == "improved"
                    ),
                    "skippedPrograms": len(skipped),
                    "headAvailabilityErrors": len(hard_errors),
                },
                "programs": comparisons,
                "skipped": skipped,
                "headAvailabilityErrors": hard_errors,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    print(markdown)

    if hard_errors:
        return 1

    return 2 if any(item["status"] == "fail" for item in comparisons) else 0


if __name__ == "__main__":
    raise SystemExit(main())
