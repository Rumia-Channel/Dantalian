from __future__ import annotations

import os
import re
from pathlib import Path
from typing import NoReturn
from urllib.parse import urlparse

TARGETS = {"develop", "staging"}
NAME_PATTERN = re.compile(r"^[a-z0-9][a-z0-9-]{0,62}$")
SECRET_NAME_PATTERN = re.compile(r"^[A-Za-z0-9._-]+$")
WORKERS_DEV_URL_PATTERN = re.compile(r"^https://[A-Za-z0-9.-]+\.workers\.dev$")


def fail(message: str) -> NoReturn:
    raise SystemExit(message)


def required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        fail(f"Missing deployment value: {name}")
    return value


def render_controller_url(worker_base_url: str, target: str) -> str:
    if not WORKERS_DEV_URL_PATTERN.fullmatch(worker_base_url):
        fail("WORKER_BASE_URL must be a workers.dev URL without a path")
    hostname = urlparse(worker_base_url).hostname
    if hostname is None:
        fail("WORKER_BASE_URL must contain a hostname")
    worker_name = f"dantalian-worker-{target}"
    controller_name = f"dantalian-audio-controller-{target}"
    prefix = f"{worker_name}."
    if not hostname.startswith(prefix):
        fail("WORKER_BASE_URL has an unexpected target Worker name")
    return f"https://{controller_name}.{hostname[len(prefix):]}/internal-api"


def main() -> None:
    target = required("DANTALIAN_DEPLOY_TARGET")
    if target not in TARGETS:
        fail("DANTALIAN_DEPLOY_TARGET must be develop or staging")

    database_name = required("DANTALIAN_D1_DATABASE_NAME")
    queue_name = required("DANTALIAN_AUDIO_JOB_QUEUE")
    dead_letter_queue = required("DANTALIAN_AUDIO_JOB_DLQ")
    worker_base_url = required("WORKER_BASE_URL")
    if not NAME_PATTERN.fullmatch(database_name):
        fail("DANTALIAN_D1_DATABASE_NAME must be a lowercase target database name")
    if not NAME_PATTERN.fullmatch(queue_name):
        fail("DANTALIAN_AUDIO_JOB_QUEUE must be a lowercase target queue name")
    if not NAME_PATTERN.fullmatch(dead_letter_queue):
        fail("DANTALIAN_AUDIO_JOB_DLQ must be a lowercase queue name")
    if not queue_name.endswith(f"-{target}"):
        fail("DANTALIAN_AUDIO_JOB_QUEUE has an invalid target suffix")
    if dead_letter_queue != f"{queue_name}-dlq":
        fail("DANTALIAN_AUDIO_JOB_DLQ must match the target Queue")
    if not database_name.endswith(f"-{target}"):
        fail("DANTALIAN_D1_DATABASE_NAME has an invalid target suffix")

    secret_names = {
        "WASABI_ACCESS_KEY_ID_SECRET_NAME": required("WASABI_ACCESS_KEY_ID_SECRET_NAME"),
        "WASABI_SECRET_ACCESS_KEY_SECRET_NAME": required(
            "WASABI_SECRET_ACCESS_KEY_SECRET_NAME"
        ),
        "WASABI_ENDPOINT_SECRET_NAME": required("WASABI_ENDPOINT_SECRET_NAME"),
        "WASABI_REGION_SECRET_NAME": required("WASABI_REGION_SECRET_NAME"),
        "WASABI_BUCKET_SECRET_NAME": required("WASABI_BUCKET_SECRET_NAME"),
    }
    for name, value in secret_names.items():
        if not SECRET_NAME_PATTERN.fullmatch(value):
            fail(f"{name} contains an invalid Secrets Store name")

    replacements = {
        "AUDIO_CONTROLLER_NAME": f"dantalian-audio-controller-{target}",
        "DANTALIAN_API_SERVICE": f"dantalian-worker-{target}",
        "DANTALIAN_AUDIO_JOB_QUEUE": queue_name,
        "DANTALIAN_AUDIO_JOB_DLQ": dead_letter_queue,
        "PROCESSOR_API_BASE_URL": render_controller_url(worker_base_url, target),
        **secret_names,
    }
    template_path = Path(f"wrangler.audio.{target}.toml")
    template = template_path.read_text(encoding="utf-8")
    for name, value in replacements.items():
        template = template.replace(f"__{name}__", value)
    if "__" in template:
        fail(f"{template_path} still contains unresolved placeholders")
    Path("wrangler.audio.ci.toml").write_text(template, encoding="utf-8")


if __name__ == "__main__":
    main()
