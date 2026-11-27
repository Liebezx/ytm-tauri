# Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

## Signed artifacts

When released through the SignPath GitHub Actions pipeline (`.github/workflows/release-signpath.yml`), the following Windows artifacts built directly from public source on GitHub-hosted runners are submitted for Authenticode signing:

- `yt-music-tauri.exe` (application binary)
- `YouTube Music_*_x64-setup.exe` (NSIS installer)
- `YouTube Music_*_x64_en-US.msi` (MSI installer)

Note that Windows displays **SignPath Foundation** as the certificate publisher for binaries signed under the open-source program. Third-party runtime components bundled or invoked at install time (such as the Microsoft Edge WebView2 bootstrapper/runtime) retain Microsoft's own publisher signatures.

## Team roles and build integrity

- All builds are produced deterministically from tracked commits/tags on `windows-latest` GitHub Actions runners via `.github/workflows/release-signpath.yml`.
- Code review, repository maintenance, and SignPath signing approvals are restricted to the repository maintainer(s) with two-factor authentication (2FA) enabled on both GitHub and SignPath.
- Unsigned local builds (`npm run build:unsigned`) are for local testing only and are never published as signed releases.

## Privacy and network behavior

This application loads `https://music.youtube.com` inside WebView2 and only communicates externally for user-enabled features (optional Discord Rich Presence, Last.fm/ListenBrainz scrobbling, Spotify playlist import, synced lyrics providers, SponsorBlock segments, and GitHub Releases update checks). No telemetry or analytics are collected by this wrapper.
