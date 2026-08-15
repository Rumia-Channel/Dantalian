from __future__ import annotations

import os
import re
from pathlib import Path
from typing import NoReturn
from uuid import UUID

TARGETS = {"staging", "production"}
NAME_PATTERN = re.compile(r"^[a-z0-9][a-z0-9-]{0,62}$")
SECRET_NAME_PATTERN = re.compile(r"^[A-Za-z0-9._-]+$")

def fail(message: str) -> NoReturn:
    raise SystemExit(message)


def required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        fail(f"Missing deployment value: {name}")
    return value


def main() -> None:
    target = required("DANTALIAN_DEPLOY_TARGET")
    if target not in TARGETS:
        fail("DANTALIAN_DEPLOY_TARGET must be staging or production")

    database_name = required("DANTALIAN_D1_DATABASE_NAME")
    database_id = required("DANTALIAN_D1_DATABASE_ID")
    queue_name = required("DANTALIAN_AUDIO_JOB_QUEUE")
    if not NAME_PATTERN.fullmatch(database_name):
        fail("DANTALIAN_D1_DATABASE_NAME must be a lowercase target database name")
    if not NAME_PATTERN.fullmatch(queue_name):
        fail("DANTALIAN_AUDIO_JOB_QUEUE must be a lowercase target queue name")
    database_suffix = f"-{target}"
    if not database_name.endswith(database_suffix):
        fail("DANTALIAN_D1_DATABASE_NAME has an invalid target suffix")
    if not queue_name.endswith(database_suffix):
        fail("DANTALIAN_AUDIO_JOB_QUEUE has an invalid target suffix")
    if not NAME_PATTERN.fullmatch(database_name[: -len(database_suffix)]):
        fail("DANTALIAN_D1_DATABASE_NAME has an invalid base name")
    if not NAME_PATTERN.fullmatch(queue_name[: -len(database_suffix)]):
        fail("DANTALIAN_AUDIO_JOB_QUEUE has an invalid base name")
    try:
        UUID(database_id)
    except ValueError:
        fail("DANTALIAN_D1_DATABASE_ID must be a UUID")

    secret_names = {
        "WASABI_ACCESS_KEY_ID_SECRET_NAME": required("WASABI_ACCESS_KEY_ID_SECRET_NAME"),
        "WASABI_SECRET_ACCESS_KEY_SECRET_NAME": required("WASABI_SECRET_ACCESS_KEY_SECRET_NAME"),
        "WASABI_ENDPOINT_SECRET_NAME": required("WASABI_ENDPOINT_SECRET_NAME"),
        "WASABI_REGION_SECRET_NAME": required("WASABI_REGION_SECRET_NAME"),
        "WASABI_BUCKET_SECRET_NAME": required("WASABI_BUCKET_SECRET_NAME"),
    }
    for name, value in secret_names.items():
        if not SECRET_NAME_PATTERN.fullmatch(value):
            fail(f"{name} contains an invalid Secrets Store name")

    replacements = {
        "DANTALIAN_D1_DATABASE_NAME": database_name,
        "DANTALIAN_D1_DATABASE_ID": database_id,
        "DANTALIAN_AUDIO_JOB_QUEUE": queue_name,
        "WASABI_PREFIX": f"dantalian/{target}",
        **secret_names,
    }
    if target == "production":
        service_domain = required("SERVICE_DOMAIN")
        if not re.fullmatch(r"[A-Za-z0-9.-]+", service_domain):
            fail("SERVICE_DOMAIN must be a hostname without a scheme or path")
        replacements["SERVICE_DOMAIN"] = service_domain
        template_path = Path("wrangler.production.toml")
    else:
        template_path = Path("wrangler.staging.toml")

    template = template_path.read_text(encoding="utf-8")
    for name, value in replacements.items():
        template = template.replace("__" + name + "__", value)
    if "__" in template:
        fail(f"{template_path} still contains unresolved placeholders")
    Path("wrangler.ci.toml").write_text(template, encoding="utf-8")


if __name__ == "__main__":
    main()
