#!/usr/bin/env python3
import os
import sys
from pathlib import Path


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    config_home = Path(
        os.environ.get("MARS_PRIVATE_CONFIG_HOME", repo_root / "misc" / "private_yazelix")
    )
    config_file = config_home / "config.toml"

    if not config_file.is_file():
        print(f"Mars private Yazelix config is missing: {config_file}", file=sys.stderr)
        return 1

    mars_binary = os.environ.get("MARS_BINARY", "mars")
    os.environ["RIO_CONFIG_HOME"] = str(config_home)
    os.execvp(mars_binary, [mars_binary, *sys.argv[1:]])
    return 127


if __name__ == "__main__":
    raise SystemExit(main())
