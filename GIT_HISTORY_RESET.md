# Clean Git History Migration (Hey work)

Use these steps after you verify the app changes and want a new GitHub repo with no upstream history.

## 1) Create a new clean git history

```bash
rm -rf .git
git init
git add .
git commit -m "Initial commit: Hey work beta"
```

## 2) Connect your new GitHub repository

```bash
git branch -M main
git remote add origin <your-new-repo-url>
git push -u origin main
```

## 3) Recommended post-push checks

- Confirm repository name, description, and topics match Hey work branding.
- Confirm no old upstream references remain in README/docs.
- Create a `v0.1.0` tag to trigger Windows release workflow:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## 4) Shareable Windows installer after release

- Open GitHub Releases for your repo.
- Download the generated `.exe` installer from `nsis` artifacts.
- Share that installer file with your Windows friends.
