# Release preparation

A release must identify the installed components and their compatibility,
not just a repository tag. MiniFuse Backend, MiniFuse Control and MiniFuse for
Omarchy retain independent source histories and component versions.

## Installation preview

The `installer-preview-2026-10-29` release distributes the already validated
`minifuse-linux-installer-2026-10-29.tar.gz` and its external SHA-256 file.
It is a prerelease, with installer source at
`656862e1537bdae6f7068f00cf50e6f54262be72`. The archive's build record separately
identifies backend packages, GUI source/recipe and plugin source.

The delivered archive SHA-256 is
`a2f98a053369e1a1ada2bc4b9aa81a90b7967ab1694a91ac4df7f9ed68a9fefd`.
Publication does not regenerate or relabel this artifact. Its historical source
URLs and build records remain intact; the repository rename is covered by
[project names and compatibility](PROJECT_NAMES.md).

## Subsequent releases

1. Review clean source revisions, licenses, source pins and release scope. Keep
   runtime component versions distinct from package build and installer revisions.
2. Require passing CI on exact PR heads and integrated main revisions. Review
   package contents and source/recipe/archive digests; do not ship temporary
   merge artifacts or rebuild an unchanged approved artifact silently.
3. Follow [the reproduction checklist](REPRODUCING.md) for the selected targets.
   Record build-only, package, real-machine and hardware evidence separately.
4. Assemble a new kit in a new directory. Verify every internal checksum, extract
   the final archive into an isolated directory and verify again. Retain source
   archives, licenses and `BUILD-RECORD.json` alongside native packages.
5. Test the documented install profiles, read-only mode, upgrade and removal.
   Read-only commands must not ask for privileges or change session configuration.
6. Prepare a prerelease with exact component versions, supported distribution,
   architecture, runtime requirements, known limitations and an upgrade/removal
   guide. Check archive/checksum names against the README commands.
7. Upload to a draft, download and compare published asset digests, then publish.
   Do not mark a preview stable or latest by default. Preserve previous releases.
8. Verify public download links, badges and support forms after publication.

The Arch source exporter is documented in [packaging](../packaging/arch/README.md).
The complete-kit assembler is `tools/prepare_installation_kit.py`; it combines
an existing verified base kit and the current reviewed installer revision.
Never overwrite a delivered kit to update its documentation or branding.

Package signing, an authenticated APT repository, COPR delivery and AUR
publication are separate milestones in [the distribution roadmap](DISTRIBUTION.md).
Checksums alone are integrity references, not signatures. A successful build or
one existing-machine report is not universal hardware/distribution acceptance.
