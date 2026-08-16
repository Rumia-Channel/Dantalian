from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path
from typing import Any, NoReturn

TARGETS = {"develop", "staging", "production"}
NAME_PATTERN = re.compile(r"^[a-z0-9][a-z0-9-]{0,62}$")
UUID_PATTERN = re.compile(
    r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b"
)


def fail(message: str) -> NoReturn:
    raise SystemExit(message)


def run_wrangler(*args: str) -> str:
    result = subprocess.run(
        ["npx", "wrangler", *args],
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        details = (result.stderr or result.stdout).strip()
        fail(f"wrangler {' '.join(args)} failed: {details}")
    return result.stdout


def parse_json_output(output: str) -> Any:
    stripped = output.strip()
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        pass

    for line in reversed(stripped.splitlines()):
        line = line.strip()
        if line.startswith(("[", "{")):
            try:
                return json.loads(line)
            except json.JSONDecodeError:
                continue
    fail("wrangler returned invalid JSON")


def database_records() -> list[dict[str, Any]]:
    payload = parse_json_output(run_wrangler("d1", "list", "--json"))
    if not isinstance(payload, list):
        fail("wrangler d1 list returned an unexpected payload")
    return [record for record in payload if isinstance(record, dict)]


def database_id(record: dict[str, Any]) -> str | None:
    for key in ("uuid", "database_id", "id"):
        value = record.get(key)
        if isinstance(value, str) and UUID_PATTERN.fullmatch(value):
            return value
    return None


def ensure_database(name: str) -> str:
    for record in database_records():
        if record.get("name") == name:
            identifier = database_id(record)
            if identifier:
                return identifier
            fail(f"D1 database {name!r} has no UUID in Wrangler output")

    create = subprocess.run(
        ["npx", "wrangler", "d1", "create", name],
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if create.returncode == 0:
        match = UUID_PATTERN.search(create.stdout + create.stderr)
        if match:
            return match.group(0)

    # A concurrent workflow may have created it between list and create.
    for record in database_records():
        if record.get("name") == name:
            identifier = database_id(record)
            if identifier:
                return identifier
    details = (create.stderr or create.stdout).strip()
    fail(f"could not create or resolve D1 database {name!r}: {details}")


def queue_records() -> list[tuple[str, str]]:
    output = run_wrangler("queues", "list")
    records: list[tuple[str, str]] = []
    for line in output.splitlines():
        columns = [column.strip() for column in line.split("│")]
        if len(columns) < 4:
            continue
        identifier, name = columns[1], columns[2]
        if identifier and name and identifier.lower() != "id" and name.lower() != "name":
            records.append((identifier, name))
    return records


def ensure_queue(name: str) -> None:
    if any(queue_name == name for _, queue_name in queue_records()):
        return

    create = subprocess.run(
        ["npx", "wrangler", "queues", "create", name],
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if create.returncode == 0:
        return

    # A concurrent workflow may have created it between list and create.
    if any(queue_name == name for _, queue_name in queue_records()):
        return
    details = (create.stderr or create.stdout).strip()
    fail(f"could not create or resolve Queue {name!r}: {details}")


def write_outputs(values: dict[str, str]) -> None:
    output_path = os.environ.get("GITHUB_OUTPUT")
    if output_path:
        with Path(output_path).open("a", encoding="utf-8") as output:
            for key, value in values.items():
                output.write(f"{key}={value}\n")
    else:
        for key, value in values.items():
            print(f"{key}={value}")


def main() -> None:
    target = os.environ.get("DANTALIAN_DEPLOY_TARGET", "")
    base_database = os.environ.get("DANTALIAN_D1_DATABASE_NAME", "")
    base_queue = os.environ.get("DANTALIAN_AUDIO_JOB_QUEUE", "")
    if target not in TARGETS:
        fail("DANTALIAN_DEPLOY_TARGET must be develop, staging, or production")
    if not NAME_PATTERN.fullmatch(base_database):
        fail("DANTALIAN_D1_DATABASE_NAME must be a lowercase base name without an environment suffix")
    if not NAME_PATTERN.fullmatch(base_queue):
        fail("DANTALIAN_AUDIO_JOB_QUEUE must be a lowercase base name without an environment suffix")
    if any(base_database.endswith(f"-{target_name}") for target_name in TARGETS):
        fail("DANTALIAN_D1_DATABASE_NAME must not include a target suffix")
    if any(base_queue.endswith(f"-{target_name}") for target_name in TARGETS):
        fail("DANTALIAN_AUDIO_JOB_QUEUE must not include a target suffix")

    database_name = f"{base_database}-{target}"
    queue_name = f"{base_queue}-{target}"
    dead_letter_queue = f"{queue_name}-dlq"
    if any(len(name) > 63 for name in (database_name, queue_name, dead_letter_queue)):
        fail("derived D1 or Queue name exceeds 63 characters")
    database_uuid = ensure_database(database_name)
    ensure_queue(queue_name)
    ensure_queue(dead_letter_queue)
    write_outputs(
        {
            "database_name": database_name,
            "database_id": database_uuid,
            "queue_name": queue_name,
            "dead_letter_queue": dead_letter_queue,
            "wasabi_prefix": f"dantalian/{target}",
        }
    )


if __name__ == "__main__":
    main()
