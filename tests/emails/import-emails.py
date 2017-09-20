#!/usr/bin/env python3

import base64
import email
import hashlib
import re
import shutil
import sys

from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
from pathlib import Path

src = Path(sys.argv[1])
dest = Path(sys.argv[2])

def read_datetime(file: Path) -> datetime:
    with file.open('rb') as f:
        mail = email.message_from_binary_file(f)
    return parsedate_to_datetime(mail.get("Date"))

def mkdirp(dir: Path):
    dir.mkdir(0o755, True, True)

def generate_filename(item: Path) -> str:
    rel_path = item.relative_to(src).as_posix().encode("UTF-8")
    hash = hashlib.sha256(rel_path).digest()
    result = base64.urlsafe_b64encode(hash).decode("ASCII")
    return result.rstrip("=")

def import_emails(dir: Path):
    for item in dir.iterdir():
        if item.name.startswith("."):
            continue
        if item.is_dir():
            import_emails(item)
            continue
        try:
            dt = read_datetime(item)
        except:
            target_dir = "unknown"
        else:
            if dt.tzinfo is None:
                dt = dt.replace(tzinfo=timezone.utc)
            else:
                dt = dt.astimezone(timezone.utc)
            target_dir = dt.strftime("%Y-%m")
        # Copy the file
        filename = generate_filename(item)
        target_dir = dest / target_dir
        mkdirp(target_dir)
        shutil.copyfile(str(item), str(target_dir / filename))

mkdirp(dest)
import_emails(src)
