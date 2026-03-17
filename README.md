# Todors

A simple Rust CLI todo app with:

- Task CRUD
- Subtasks
- Priority (`low`, `medium`, `high`)
- Reordering
- Timestamps
- JSON output mode
- JSON input for `add`

## Install

### Local development

```bash
cargo run -- <command> <args>
```

### Install as CLI

```bash
cargo install --path . --force
```

Then use:

```bash
todors <command> <args>
```

### Curl installer (from GitHub releases)

```bash
curl -fsSL https://raw.githubusercontent.com/sawirricardo/todors/main/install.sh | bash
```

## Usage

```bash
todors help
```

Main commands:

```bash
todors add [--priority <low|medium|high>] <task text>
todors add '{"text":"Task from JSON","priority":"high"}'
todors add --from-json <json|->
todors list
todors done <id>
todors remove <id> [--yes]
todors set-priority <id> <low|medium|high>
todors reorder <id> <position>
todors add-subtask <task_id> <subtask text>
todors done-subtask <task_id> <subtask_id>
todors remove-subtask <task_id> <subtask_id> [--yes]
```

## JSON Output

Add `--json` (or `-j`) to commands:

```bash
todors list --json
todors add "Write docs" --json
todors done 1 --json
```

## JSON Input for `add`

### Inline JSON

```bash
todors add '{"text":"Buy milk","priority":"high"}'
```

### JSON via stdin

```bash
echo '{"text":"Buy milk","priority":"high"}' | todors add --from-json -
```

Schema for `add` JSON:

```json
{
  "text": "string (required)",
  "priority": "low|medium|high (optional)"
}
```

Notes:

- Unknown fields are rejected.
- If input looks like JSON (`{...}`), schema validation is strict.

## Release

This repo includes a GitHub Actions release workflow at:

`.github/workflows/release.yml`

It builds binaries and attaches them to tag releases (`v*`).
