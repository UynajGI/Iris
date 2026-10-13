"""Set all Iris application versions, or check them against Cargo.toml. Never commits or tags."""

import argparse

from versioning import check, current, sync

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    version = args.version or current()
    if args.check:
        print(check(version))
    else:
        print(f"{version}: {', '.join(sync(version)) or 'already synchronized'}")
