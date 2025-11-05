# GitHub Repository Transfer Guide

## Quick Reference: Transfer Repository via CLI

This guide shows how to transfer a GitHub repository from a personal account to an organization using the GitHub CLI.

## Prerequisites

- GitHub CLI installed (`gh`)
- Authenticated with proper scopes (`repo`, `workflow`)
- Admin access to the repository
- Member of the target organization

## Step-by-Step Process

### 1. Check Available Organizations

First, verify what organizations you have access to:

```bash
gh api user/orgs --jq '.[].login'
```

**Example Output:**
```
Nguyenetic
```

**Important:** Note the exact capitalization - GitHub org names are case-sensitive!

### 2. Clear Environment Token (if needed)

If you're having authentication issues, clear the `GITHUB_TOKEN` environment variable:

```bash
unset GITHUB_TOKEN
```

This forces `gh` to use its stored credentials instead of the env var.

### 3. Verify Authentication

Check your authentication status:

```bash
gh auth status
```

**Expected Output:**
```
github.com
  ✓ Logged in to github.com account USERNAME (keyring)
  - Active account: true
  - Git operations protocol: https
  - Token: gho_************************************
  - Token scopes: 'gist', 'read:org', 'repo', 'workflow'
```

If not authenticated, run:
```bash
gh auth login
```

### 4. Transfer the Repository

Use the GitHub API to transfer the repository:

```bash
gh api -X POST repos/OLD_OWNER/REPO_NAME/transfer -f new_owner=ORG_NAME
```

**Command Breakdown:**
- `gh api` - Call GitHub API via CLI
- `-X POST` - HTTP POST method
- `repos/OLD_OWNER/REPO_NAME/transfer` - API endpoint
- `-f new_owner=ORG_NAME` - Field parameter with new owner

**Example:**
```bash
gh api -X POST repos/johnhnguyen97/NexusQL/transfer -f new_owner=Nguyenetic
```

### 5. Verify Transfer Completion

Wait a few seconds for the transfer to process, then verify:

```bash
sleep 5
gh repo view ORG_NAME/REPO_NAME --json name,owner
```

**Expected Output:**
```json
{
  "name": "NexusQL",
  "owner": {
    "id": "O_kgDOBzFsAw",
    "login": "Nguyenetic"
  }
}
```

### 6. Update Local Git Remote

Update your local repository to point to the new location:

```bash
cd /path/to/your/repo
git remote set-url origin https://github.com/ORG_NAME/REPO_NAME.git
git remote -v  # Verify
```

## Complete Script

Here's a complete script you can use:

```bash
#!/bin/bash

# Configuration
OLD_OWNER="johnhnguyen97"
REPO_NAME="NexusQL"
NEW_OWNER="Nguyenetic"

# Step 1: Clear environment token
unset GITHUB_TOKEN

# Step 2: Check authentication
echo "Checking authentication..."
gh auth status

# Step 3: List available organizations
echo "Available organizations:"
gh api user/orgs --jq '.[].login'

# Step 4: Transfer repository
echo "Transferring $OLD_OWNER/$REPO_NAME to $NEW_OWNER..."
gh api -X POST repos/$OLD_OWNER/$REPO_NAME/transfer -f new_owner=$NEW_OWNER

# Step 5: Wait for transfer to complete
echo "Waiting for transfer to complete..."
sleep 5

# Step 6: Verify transfer
echo "Verifying transfer..."
gh repo view $NEW_OWNER/$REPO_NAME --json name,owner

# Step 7: Update local git remote
echo "Updating local git remote..."
git remote set-url origin https://github.com/$NEW_OWNER/$REPO_NAME.git

echo "✅ Transfer complete!"
echo "New repository URL: https://github.com/$NEW_OWNER/$REPO_NAME"
```

## Common Issues & Solutions

### ❌ Error: "Invalid new_owner" (HTTP 422)

**Cause:** Incorrect organization name capitalization

**Solution:** Use the exact capitalization from `gh api user/orgs --jq '.[].login'`

```bash
# Wrong
gh api -X POST repos/johnhnguyen97/NexusQL/transfer -f new_owner=nguyenetic

# Correct
gh api -X POST repos/johnhnguyen97/NexusQL/transfer -f new_owner=Nguyenetic
```

### ❌ Error: "Bad credentials" (HTTP 401)

**Cause:** Invalid `GITHUB_TOKEN` environment variable

**Solution:** Clear the env var and re-authenticate

```bash
unset GITHUB_TOKEN
gh auth login
```

### ❌ Error: "Not Found" when verifying transfer

**Cause:** Transfer is still processing

**Solution:** Wait a few more seconds and try again

```bash
sleep 5
gh repo view ORG_NAME/REPO_NAME --json owner
```

### ❌ Error: "Must have admin rights to Repository"

**Cause:** You don't have admin access to the repository

**Solution:** Contact the repository owner to grant you admin access

### ❌ Error: "Cannot transfer to this organization"

**Cause:** You're not a member of the target organization

**Solution:** Have the org owner add you as a member first

## API Reference

### Transfer Repository Endpoint

```
POST /repos/{owner}/{repo}/transfer
```

**Parameters:**
- `new_owner` (required, string) - The username or organization name the repository will be transferred to

**Response:**
- `200 OK` - Transfer initiated successfully
- `401 Unauthorized` - Invalid authentication
- `404 Not Found` - Repository not found
- `422 Unprocessable Entity` - Invalid new_owner or transfer not allowed

**Documentation:** https://docs.github.com/rest/repos/repos#transfer-a-repository

## After Transfer Checklist

✅ Update `package.json` repository URLs
✅ Update `Cargo.toml` repository metadata
✅ Update `README.md` installation instructions
✅ Update local git remote: `git remote set-url origin`
✅ Update CI/CD pipelines (GitHub Actions, etc.)
✅ Update documentation links
✅ Notify team members of new repository location
✅ Update any webhooks or integrations

## Security Notes

- The `gh` CLI stores tokens securely in your system's keyring
- Never commit tokens or credentials to your repository
- Use `gh auth login` instead of manual token management
- Regularly rotate access tokens
- Use fine-grained personal access tokens when possible

## Related Commands

```bash
# List all repositories
gh repo list

# View repository details
gh repo view OWNER/REPO

# Clone from new location
gh repo clone ORG_NAME/REPO_NAME

# Check repository permissions
gh api repos/OWNER/REPO --jq '.permissions'
```

---

**Date Created:** 2024-11-03
**Last Updated:** 2024-11-03
**Repository:** NexusQL
**Organization:** Nguyenetic
