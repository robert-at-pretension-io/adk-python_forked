# Repo Cloner Agent

The Repo Cloner Agent is an ADK agent designed to clone Git repositories with support for public and private repositories. It provides a simple interface for cloning repositories with optional branch selection, shallow cloning, and token-based authentication.

## Features

- Clone public Git repositories
- Clone private repositories using personal access tokens
- Support for GitHub and GitLab authentication
- Optional branch/tag selection
- Shallow clone capability
- Custom destination directory
- Automatic credential management

## Usage

### Starting the Agent

```bash
# From the agents/repo_cloner directory:
adk run

# Or from anywhere:
adk run agents/repo_cloner
```

### Example Interactions

**Clone a public repository:**
```
You: Clone https://github.com/python/cpython
Agent: Repository cloned to /tmp/adk_clone_abc123/repo
```

**Clone with specific branch:**
```
You: Clone https://github.com/nodejs/node on branch v20.x
Agent: Repository cloned to /tmp/adk_clone_def456/repo
```

**Clone with shallow depth:**
```
You: Clone https://github.com/torvalds/linux with depth 1
Agent: Repository cloned to /tmp/adk_clone_ghi789/repo
```

**Clone to specific directory:**
```
You: Clone https://github.com/rust-lang/rust to ~/projects/rust
Agent: Repository cloned to /home/user/projects/rust
```

## Configuration

The agent's `clone_repo` tool supports various options:

- `repo_url`: The HTTPS URL of the repository (required)
- `branch`: Specific branch, tag, or commit to checkout
- `depth`: Number indicating shallow clone depth
- `dest_dir`: Custom destination directory
- `token`: Personal access token for private repositories
- `username_hint`: Custom username for authentication

## Security

- Credentials are stored in temporary files with secure permissions (600)
- Credential files are automatically cleaned up after use
- Tokens are never logged or displayed in output

## Error Handling

The agent handles common error scenarios:
- Missing git executable
- Invalid repository URLs
- Authentication failures
- Network issues
- Permission errors

## Development

The agent is built using the ADK framework with a simple function tool for cloning repositories. To modify, edit `agent.py` and test using the ADK CLI:

```bash
# Run the agent
adk run

# Run with custom configuration
adk run --config custom_config.yaml
```

The agent follows standard ADK patterns using a function-based tool that integrates seamlessly with the LLM.