# git-aicommit

Generate a commit message from staged changes using a free LLM, then open your git editor to review before committing.

## Install

```bash
cargo build --release
cp target/release/git-aicommit ~/.local/bin/
```

Because the binary is named `git-aicommit`, Git exposes it as:

```bash
git aicommit
```

## Setup

Copy `.env.example` to `.env` and add your OpenRouter key:

```bash
cp .env.example .env
```

Defaults (via `.env` or env vars):

| Setting | Default |
|---------|---------|
| Provider | `openrouter` |
| Model | `google/gemma-4-31b-it:free` |
| API key | `OPENROUTER_API_KEY` from [OpenRouter](https://openrouter.ai) |

The tool loads `.env` from the current directory automatically.

## Usage

```bash
git add .
git aicommit              # opens editor with suggested message
git aicommit --dry-run    # print message only, no commit
git aicommit --provider gemini   # switch to Gemini instead
git aicommit --prompt ./commit-style.txt
```

### Options

| Flag | Env | Description |
|------|-----|-------------|
| `--provider` | `AICOMMIT_PROVIDER` | `openrouter` (default) or `gemini` |
| `--model` | `AICOMMIT_MODEL` | Override model name |
| `--api-key` | `AICOMMIT_API_KEY` | Override API key |
| `--base-url` | `AICOMMIT_BASE_URL` | Override API base URL |
| `--prompt FILE` | — | Extra instructions appended to system prompt |
| `--dry-run` | — | Print message, skip editor/commit |

## How it works

1. Reads `git diff --staged`
2. Strips lockfiles, binaries, and oversized diffs
3. Sends diff + recent commit subjects to the LLM
4. Opens `git commit -e -F` with the suggested message pre-filled
