# Git Submodule Setup Guide for AutoDev
## Step-by-Step Instructions

### Prerequisites
- Git installed
- Access to `AIRS-Foundry/core-orchestrator` and `AIRS-Foundry/memory-mcp-server` repos
- GitHub credentials configured

---

## Step 1: Initialize Git Repository

If `autodev` isn't already a git repo:

```bash
cd "c:\AIRS Foundry\autodev"
git init
git branch -M main
```

If you want to connect to a remote:

```bash
git remote add origin https://github.com/AIRS-Foundry/autodev.git
```

---

## Step 2: Add Submodules

**Important:** Replace the URLs with your actual repository URLs.

```bash
# Create the submodules directory first
mkdir submodules

# Add core-orchestrator as a submodule
git submodule add https://github.com/AIRS-Foundry/core-orchestrator.git submodules/core-orchestrator

# Add memory-mcp-server as a submodule
git submodule add https://github.com/AIRS-Foundry/memory-mcp-server.git submodules/memory-mcp-server
```

This will:
- Clone both repos into `submodules/`
- Create a `.gitmodules` file that tracks the submodule configuration
- Add entries to `.git/config`

---

## Step 3: Configure Submodules to Track Main Branch

Edit `.gitmodules` to add branch tracking:

```bash
# Open .gitmodules in your editor
notepad .gitmodules
```

Make sure it looks like this:

```ini
[submodule "submodules/core-orchestrator"]
    path = submodules/core-orchestrator
    url = https://github.com/AIRS-Foundry/core-orchestrator.git
    branch = main

[submodule "submodules/memory-mcp-server"]
    path = submodules/memory-mcp-server
    url = https://github.com/AIRS-Foundry/memory-mcp-server.git
    branch = main
```

---

## Step 4: Commit the Submodule Configuration

```bash
git add .gitmodules submodules/
git commit -m "feat: add core-orchestrator and memory-mcp-server as submodules"
```

---

## Step 5: When Someone Else Clones Your Repo

When another developer clones `autodev`, they need to initialize submodules:

```bash
# Clone the main repo
git clone https://github.com/AIRS-Foundry/autodev.git
cd autodev

# Initialize and fetch submodules
git submodule init
git submodule update

# Or do both at once with:
git submodule update --init --recursive
```

**Even easier:** Clone with submodules in one command:

```bash
git clone --recurse-submodules https://github.com/AIRS-Foundry/autodev.git
```

---

## Working with Submodules

### Updating to Latest Submodule Commits

```bash
# Update all submodules to their latest main branch
git submodule update --remote --merge

# Or update a specific submodule
cd submodules/core-orchestrator
git pull origin main
cd ../..

# Commit the updated submodule pointer
git add submodules/core-orchestrator
git commit -m "chore: update core-orchestrator to latest"
```

### Editing Code in a Submodule

**Workflow:**

1. Navigate into the submodule:
   ```bash
   cd submodules/core-orchestrator
   ```

2. Check current status:
   ```bash
   git status
   git branch
   ```

3. Make sure you're on main (not detached HEAD):
   ```bash
   git checkout main
   git pull origin main
   ```

4. Create a feature branch (optional but recommended):
   ```bash
   git checkout -b fix/stdio-transport
   ```

5. Make your changes to the code

6. Commit and push **inside the submodule**:
   ```bash
   git add .
   git commit -m "fix: implement stdio MCP transport"
   git push origin fix/stdio-transport
   ```

7. If you pushed to a branch, merge it on GitHub (PR), then:
   ```bash
   git checkout main
   git pull origin main
   ```

8. Return to parent repo and update the submodule pointer:
   ```bash
   cd ../..  # back to autodev root
   git add submodules/core-orchestrator
   git commit -m "chore: bump core-orchestrator (stdio fix included)"
   git push
   ```

### Checking Submodule Status

```bash
# See which commit each submodule is on
git submodule status

# See if submodules have uncommitted changes
git submodule foreach git status
```

---

## Do You Need to Run `uv venv` or `uv sync`?

### Short Answer: **Not in the parent repo, only inside submodules if you're developing them.**

### Detailed Explanation:

1. **In the parent repo (`autodev/`):**
   - **No** — you don't run `uv venv` or `uv sync` here
   - The submodules are just tracked as git references
   - You won't run Python code directly from the parent repo during development
   - The build scripts (`build/bundle-core.sh`) will handle installing dependencies when creating PyInstaller bundles

2. **Inside a submodule (e.g., `submodules/core-orchestrator/`):**
   - **Yes, if you want to develop/test it locally**
   - When you're working on core-orchestrator code and want to test it:
     ```bash
     cd submodules/core-orchestrator
     
     # If the repo uses uv for dependency management
     uv venv
     source .venv/bin/activate  # On Windows: .venv\Scripts\activate
     uv sync
     
     # Now you can run the orchestrator locally for testing
     python src/main.py
     ```
   
3. **For building sidecars:**
   - The build scripts will handle dependencies
   - PyInstaller bundles everything needed (Python runtime + all deps)
   - Example `build/bundle-core.sh`:
     ```bash
     #!/bin/bash
     cd submodules/core-orchestrator
     
     # Install dependencies in a temporary venv
     python -m venv build_venv
     source build_venv/bin/activate
     pip install -r requirements.txt
     pip install pyinstaller
     
     # Bundle into standalone binary
     pyinstaller --onefile --name core-orchestrator src/main.py
     
     # Copy to sidecars/
     cp dist/core-orchestrator ../../sidecars/
     
     # Cleanup
     deactivate
     rm -rf build_venv
     ```

### Summary Table

| Context | Run `uv venv`/`uv sync`? | Why? |
|---------|--------------------------|------|
| Parent repo (`autodev/`) | ❌ No | Submodules are just git pointers, no Python execution here |
| Inside submodule for development | ✅ Yes | To test the Python code locally before bundling |
| During CI/CD build | ❌ No (use pip) | PyInstaller scripts handle dependencies |

---

## Common Issues & Solutions

### Issue: "Submodule is in detached HEAD state"

```bash
cd submodules/core-orchestrator
git checkout main
git pull origin main
```

### Issue: "Changes not staged for commit" in submodule

This means you edited code inside the submodule but didn't commit it:

```bash
cd submodules/core-orchestrator
git status  # See what changed
git add .
git commit -m "fix: your changes"
git push origin main
```

### Issue: Submodule shows modified but you didn't change anything

Someone else updated the submodule upstream. Pull latest:

```bash
git submodule update --remote --merge
git add submodules/
git commit -m "chore: sync submodules to latest upstream"
```

### Issue: Accidentally edited files in submodule without committing

Option 1 - Discard changes:
```bash
cd submodules/core-orchestrator
git checkout .
```

Option 2 - Stash changes:
```bash
cd submodules/core-orchestrator
git stash
# Apply later with: git stash pop
```

---

## Quick Reference Commands

```bash
# Add submodule
git submodule add <url> <path>

# Clone repo with submodules
git clone --recurse-submodules <repo-url>

# Initialize submodules after clone
git submodule update --init --recursive

# Update all submodules to latest
git submodule update --remote --merge

# Update specific submodule
cd <submodule-path>
git pull origin main

# Check submodule status
git submodule status

# Run command in all submodules
git submodule foreach <command>

# Example: check status of all submodules
git submodule foreach git status
```

---

## Next Steps After Setup

1. **Create build scripts:**
   - `build/bundle-core.sh`
   - `build/bundle-memory-mcp.sh`
   - `build/fetch-redis.sh`

2. **Add to `.gitignore`:**
   ```
   sidecars/
   *.pyc
   __pycache__/
   .venv/
   build_venv/
   ```

3. **Test the submodule workflow:**
   - Make a small change in `core-orchestrator`
   - Commit and push
   - Update the submodule pointer in `autodev`
   - Verify another team member can pull and get the update

4. **Configure Tauri sidecar management:**
   - Add sidecar declarations to `tauri.conf.json`
   - Implement Rust code to launch/monitor sidecars
   - Add health check logic

---

## Resources

- [Git Submodules Official Docs](https://git-scm.com/book/en/v2/Git-Tools-Submodules)
- [Tauri Sidecar Documentation](https://tauri.app/v1/guides/building/sidecar)
- [PyInstaller Documentation](https://pyinstaller.org/en/stable/)
