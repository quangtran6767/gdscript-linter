import re

def main() -> None:
    """Generate release notes for the latest GitHub release."""

    with open("CHANGELOG.md", encoding="utf-8") as file:
        changelog = file.read()

    first_section = re.search(r"^### ", changelog, re.MULTILINE)
    if first_section is None:
        raise RuntimeError("Could not find the first changelog section.")

    next_release = re.search(r"^## ", changelog[first_section.start():], re.MULTILINE)

    if next_release is None:
        raise RuntimeError("Could not find the next release heading.")

    latest_changes = changelog[first_section.start() : first_section.start() + next_release.start()].strip()

    release_notes = f"""GDScript linting, reviewed fixes and formatting for CLI and AI-agent workflows.

## Changelog

{latest_changes}

See [CLI and agent documentation](https://github.com/quangtran6767/gdscript-linter/blob/main/docs/agents.md) and [migration notes](https://github.com/quangtran6767/gdscript-linter/blob/main/docs/migration.md).
"""

    print(release_notes)

if __name__ == "__main__":
    main()