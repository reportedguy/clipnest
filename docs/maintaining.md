# Maintaining ClipNest

Short notes for whoever reviews and releases.

## Reviewing a pull request

1. Read the description and the linked issue: does it solve a real problem, and does it keep ClipNest local-first (no new network calls, telemetry or accounts)?
2. Wait for CI. A red check means the author has something to fix; point at the failing step.
3. Read the diff in the "Files changed" tab. Leave comments on specific lines; use "Request changes" for anything that must change, "Comment" for suggestions.
4. For UI or capture changes, try it locally:

   ```bash
   gh pr checkout <number>
   pnpm install
   pnpm tauri dev --config '{"identifier":"com.clipnest.dev"}'
   ```

5. Check that user-visible changes are in CHANGELOG.md under "Unreleased".
6. Thank the contributor, even when asking for changes.

## Merging

- Use **Squash and merge**, so each pull request becomes one commit on `main`. Edit the squashed message into a Conventional Commit (`feat: …`, `fix: …`) if needed.
- Dependabot pull requests: merge once CI is green. Read the changelog of major-version bumps before merging.
- Delete the branch after merging (GitHub offers a button).

## Releasing

See [releasing.md](releasing.md): bump the version, move "Unreleased" in the changelog, tag `vX.Y.Z`, then review and publish the draft release the workflow creates.

## Issues

- Label new issues (`bug`, `enhancement`, `docs`…) and add `good first issue` to small, well-described ones.
- Ask for the ClipNest version, Windows version and steps if a bug report lacks them.
- Close duplicates with a link to the original.
