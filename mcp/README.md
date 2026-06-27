# MCP Server for git-aicommit

This directory contains an MCP server that wraps git-aicommit for use in Cursor.

## Setup

```bash
cd mcp
python -m venv .venv
source .venv/bin/activate
pip install -e .
```

## Configuration

Add to `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "aicommit": {
      "command": "/Users/ryanli/Documents/GitHub/commit-generator/mcp/.venv/bin/python",
      "args": ["-m", "aicommit_mcp.server"]
    }
  }
}
```

## Tools

- `generate_commit_message`: Generate AI commit message for staged changes
- `commit_with_ai_message`: Generate and commit immediately without editor
