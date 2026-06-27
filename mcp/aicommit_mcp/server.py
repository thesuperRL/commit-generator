#!/usr/bin/env python3
"""MCP server for git-aicommit integration."""

import asyncio
import json
import logging
import subprocess
from pathlib import Path
from typing import Any

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import Tool, TextContent

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = Server("aicommit")


def run_git_aicommit(repo_path: str, args: list[str]) -> dict[str, Any]:
    """Run git-aicommit command and capture output."""
    try:
        # Change to repo directory and run git-aicommit
        result = subprocess.run(
            ["git", "aicommit"] + args,
            cwd=repo_path,
            capture_output=True,
            text=True,
            check=False,
        )
        
        return {
            "success": result.returncode == 0,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "exit_code": result.returncode,
        }
    except FileNotFoundError:
        return {
            "success": False,
            "error": "git-aicommit not found. Make sure it's installed and in your PATH.",
            "exit_code": -1,
        }
    except Exception as e:
        return {
            "success": False,
            "error": str(e),
            "exit_code": -1,
        }


@app.list_tools()
async def list_tools() -> list[Tool]:
    """List available aicommit tools."""
    return [
        Tool(
            name="generate_commit_message",
            description="Generate an AI commit message for staged changes in a git repository. Returns the suggested commit message without committing.",
            inputSchema={
                "type": "object",
                "properties": {
                    "repo_path": {
                        "type": "string",
                        "description": "Path to git repository (any directory inside the repo works)",
                    },
                    "provider": {
                        "type": "string",
                        "description": "LLM provider: 'openrouter' (default) or 'gemini'",
                        "default": "openrouter",
                    },
                    "include_context": {
                        "type": "boolean",
                        "description": "Include previous commit as context",
                        "default": False,
                    },
                    "retry_forever": {
                        "type": "boolean",
                        "description": "Retry on errors until success",
                        "default": False,
                    },
                },
                "required": ["repo_path"],
            },
        ),
        Tool(
            name="commit_with_ai_message",
            description="Generate an AI commit message and commit immediately without opening an editor. Use this to commit staged changes with an AI-generated message.",
            inputSchema={
                "type": "object",
                "properties": {
                    "repo_path": {
                        "type": "string",
                        "description": "Path to git repository (any directory inside the repo works)",
                    },
                    "provider": {
                        "type": "string",
                        "description": "LLM provider: 'openrouter' (default) or 'gemini'",
                        "default": "openrouter",
                    },
                    "include_context": {
                        "type": "boolean",
                        "description": "Include previous commit as context",
                        "default": False,
                    },
                    "retry_forever": {
                        "type": "boolean",
                        "description": "Retry on errors until success",
                        "default": False,
                    },
                },
                "required": ["repo_path"],
            },
        ),
    ]


@app.call_tool()
async def call_tool(name: str, arguments: Any) -> list[TextContent]:
    """Handle tool calls."""
    try:
        repo_path = arguments.get("repo_path", ".")
        provider = arguments.get("provider", "openrouter")
        include_context = arguments.get("include_context", False)
        retry_forever = arguments.get("retry_forever", False)
        
        # Build command args
        cmd_args = ["--provider", provider]
        
        if include_context:
            cmd_args.append("-c")
        
        if retry_forever:
            cmd_args.append("-r")
        
        if name == "generate_commit_message":
            # Just generate, don't commit
            cmd_args.append("--dry-run")
            result = run_git_aicommit(repo_path, cmd_args)
            
            if result["success"]:
                message = result["stdout"]
                stderr_preview = result["stderr"].split("\n")[0] if result["stderr"] else ""
                return [
                    TextContent(
                        type="text",
                        text=f"Suggested commit message:\n\n{message}\n\n{stderr_preview}",
                    )
                ]
            else:
                error_msg = result.get("error", result["stderr"])
                return [
                    TextContent(
                        type="text",
                        text=f"Error generating commit message:\n{error_msg}",
                    )
                ]
        
        elif name == "commit_with_ai_message":
            # Commit immediately without editor
            cmd_args.append("--fast")
            result = run_git_aicommit(repo_path, cmd_args)
            
            if result["success"]:
                # Extract message from stderr (where the tool prints it)
                stderr_lines = result["stderr"].split("\n")
                message_lines = []
                capturing = False
                for line in stderr_lines:
                    if "Suggested commit message:" in line:
                        capturing = True
                        continue
                    if capturing and line.strip():
                        message_lines.append(line)
                
                message = "\n".join(message_lines) if message_lines else "Committed successfully"
                return [
                    TextContent(
                        type="text",
                        text=f"Committed successfully with message:\n\n{message}",
                    )
                ]
            else:
                error_msg = result.get("error", result["stderr"])
                return [
                    TextContent(
                        type="text",
                        text=f"Error committing:\n{error_msg}",
                    )
                ]
        
        else:
            return [
                TextContent(
                    type="text",
                    text=f"Unknown tool: {name}",
                )
            ]
    
    except Exception as e:
        logger.error(f"Tool error: {e}", exc_info=True)
        return [
            TextContent(
                type="text",
                text=f"Error: {str(e)}",
            )
        ]


async def main():
    """Run the MCP server."""
    async with stdio_server() as (read_stream, write_stream):
        await app.run(read_stream, write_stream, app.create_initialization_options())


if __name__ == "__main__":
    asyncio.run(main())
