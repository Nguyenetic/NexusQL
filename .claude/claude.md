# Claude Code Instructions for NexusQL

This file contains important instructions for Claude Code when working on this project.

## GitHub CLI Authentication

**CRITICAL:** Always use GitHub CLI with keyring authentication, NOT environment tokens.

### Before ANY `gh` command, always run:

**Windows PowerShell:**
```powershell
$env:GITHUB_TOKEN = $null
```

**Windows CMD:**
```cmd
set GITHUB_TOKEN=
```

**Unix/macOS/Bash:**
```bash
unset GITHUB_TOKEN
```

### Why This Matters

- The GITHUB_TOKEN environment variable conflicts with GitHub CLI's keyring authentication
- Keyring authentication is more reliable and properly authenticated
- Previous issues occurred when the environment token was set
- Always unset it before using `gh issue list`, `gh pr create`, etc.

### Workflow

1. **Always** unset GITHUB_TOKEN first
2. Then use any `gh` CLI commands
3. The CLI will use keyring authentication automatically

## Project-Specific Notes

- All 274 tests passing
- CREATE INDEX support added in v3.2.0
- Published to npm as @nguyenetic/nexusql
- See CLAUDE.md in project root for full documentation
