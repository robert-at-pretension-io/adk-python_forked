# Copyright 2025 Google LLC
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import os
import subprocess
import tempfile
from pathlib import Path
from typing import Optional

from google.adk import Agent
from google.adk.tools.tool_context import ToolContext


def clone_repo(
    repo_url: str,
    branch: Optional[str] = None,
    depth: Optional[int] = None,
    dest_dir: Optional[str] = None,
    token: Optional[str] = None,
    username_hint: Optional[str] = None,
    tool_context: ToolContext = None
) -> dict[str, str | bool]:
    """Clone a Git repository with optional authentication and configuration.
    
    Args:
        repo_url: Full HTTPS URL of the repository (e.g., https://github.com/user/project.git)
        branch: Optional branch, tag or commit to checkout after clone
        depth: Shallow clone depth (implies --no-tags --shallow-submodules)
        dest_dir: Absolute/relative directory for the clone. If omitted, a temp dir is used
        token: Personal/Deploy access token for private repos
        username_hint: Username to pair with the token (defaults to host-specific value)
        tool_context: ADK tool context for access to services
        
    Returns:
        Dict with success status, message, and cloned path if successful
    """
    # Check if git exists
    from shutil import which
    if which("git") is None:
        return {
            "success": False,
            "message": "`git` executable not found in PATH."
        }
    
    # Set up destination directory
    tmp_holder = None
    if dest_dir:
        dest_path = Path(dest_dir).expanduser().resolve()
        dest_path.parent.mkdir(parents=True, exist_ok=True)
    else:
        tmp_holder = tempfile.TemporaryDirectory(prefix="adk_clone_")
        dest_path = Path(tmp_holder.name) / "repo"
        dest_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Set up environment and credentials
    env = os.environ.copy()
    cred_file = None
    
    if token:
        # Determine username based on repo host
        if username_hint:
            username = username_hint
        elif "github.com" in repo_url:
            username = "x-access-token"
        else:
            username = "oauth2"
        
        # Parse host from URL
        from urllib.parse import urlparse
        parsed_url = urlparse(repo_url)
        host = parsed_url.netloc
        
        # Create credential line
        cred_line = f"https://{username}:{token}@{host}\n"
        
        # Create temporary credential file
        cred_file = Path(tempfile.gettempdir()) / f"git-cred-{os.urandom(6).hex()}"
        cred_file.write_text(cred_line)
        os.chmod(cred_file, 0o600)
        
        # Configure git to use credential helper
        env.update({
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_ASKPASS": "echo",
            "GIT_CREDENTIAL_HELPER": f"store --file={cred_file}",
        })
    
    # Build git command
    cmd = ["git", "clone"]
    if branch:
        cmd.extend(["--branch", branch])
    if depth:
        cmd.extend(["--depth", str(depth), "--shallow-submodules", "--no-tags"])
    cmd.extend([repo_url, str(dest_path)])
    
    try:
        # Execute clone command
        result = subprocess.run(
            cmd,
            env=env,
            check=True,
            capture_output=True,
            text=True
        )
        
        # Persist temporary directory if used
        if tmp_holder:
            tmp_holder._finalizer.detach()  # pylint: disable=protected-access
        
        return {
            "success": True,
            "message": f"Repository cloned to {dest_path}",
            "cloned_path": str(dest_path)
        }
        
    except subprocess.CalledProcessError as e:
        return {
            "success": False,
            "message": f"git clone failed: {e.stderr.strip() if e.stderr else str(e)}"
        }
    finally:
        # Clean up credential file if created
        if cred_file and cred_file.exists():
            try:
                cred_file.unlink(missing_ok=True)
            except OSError:
                pass


root_agent = Agent(
    model='gemini-2.0-flash',
    name='git_repo_cloner',
    description='A Git repository cloning assistant',
    instruction="""
    You are a Git repository cloning assistant. Your primary function is to help users 
    clone Git repositories.
    
    When a user requests to clone a repository:
    1. Extract the repository URL from their message
    2. Identify any optional parameters (branch, depth, destination directory, authentication token)
    3. Call the clone_repo tool with the appropriate parameters
    4. Report the result to the user
    
    If the user doesn't provide a repository URL clearly, ask them to provide one.
    Always use the clone_repo tool to handle cloning requests.
    """,
    tools=[clone_repo],
)