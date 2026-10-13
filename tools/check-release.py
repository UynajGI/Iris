"""Fail a release when its tag and application versions disagree."""

import argparse
from pathlib import Path

from versioning import check as check_versions

ROOT = Path(__file__).resolve().parents[1]


def check(tag, root=ROOT):
    if not tag.startswith("v"):
        raise ValueError("Expected a version tag such as v0.1.0-beta")
    return check_versions(tag[1:], root)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    print(check(parser.parse_args().tag))
