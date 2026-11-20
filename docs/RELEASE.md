# Release checklist

## 1. Manual wheel test (Windows)

Install `target/release/bundle/nsis/File Chameleon_<version>_x64-setup.exe`, then check each item:

- [ ] Hold **Ctrl** and drag one file from File Explorer. The format wheel opens under the cursor.
- [ ] Release over a format. The conversion runs and a toast appears at the bottom right.
- [ ] Ctrl+drag several files of different kinds. Only formats that fit every file are offered.
- [ ] Press **Esc** during the drag, or release outside the wheel. Nothing is converted.
- [ ] Ctrl+drag a selection rectangle (marquee) in Explorer, with no file dragged. No wheel appears.
- [ ] Repeat the tests at 125 % and 175 % display scaling.

## 2. Publish to GitHub

Option A, GitHub Desktop: File → Add local repository → choose this folder → **Publish repository**.

Option B, GitHub CLI. Run each command from this folder:

```powershell
winget install --id GitHub.cli -e
```

```powershell
gh auth login --web
```

```powershell
gh repo create file-chameleon --public --source . --remote origin --push
```

Notes:

- Commits carry the git e-mail configured on this computer. To keep it private, publish with
  `--private` first, or switch to your GitHub `noreply` address before publishing.
- After publishing, update `repository` in the root `Cargo.toml` to the real URL.

## 3. Make a release

Push a version tag. The `Release` workflow builds the NSIS installer and attaches it, with a `.sha256` file, to a draft release:

```powershell
git tag v0.1.0
```

```powershell
git push origin v0.1.0
```
