<p align="center">
  <img src="docs/assets/icon.png" width="96" height="96" alt="TikTok-Now icon">
</p>

<h1 align="center">TikTok-Now · Local Control</h1>

<p align="center">
  A native TikTok desktop client with an opt-in Windows CLI.<br>
  <a href="docs/LOCAL_CONTROL.md">CLI guide</a> ·
  <a href="https://github.com/benedictusrey/TikTok-Now">Upstream project</a> ·
  <a href="LICENSE">MIT license</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/base-v2.1.0-blue" alt="Based on v2.1.0">
  <img src="https://img.shields.io/badge/local%20control-Windows-0078D4" alt="Windows local control">
  <img src="https://img.shields.io/badge/built%20with-Tauri%202%20%2B%20Rust-orange" alt="Tauri 2 and Rust">
</p>

This is **NNFall's unofficial development fork** of [TikTok-Now](https://github.com/benedictusrey/TikTok-Now), created by **Benedictus Reynaldo Hartanto ([@benedictusrey](https://github.com/benedictusrey))**. It keeps the original native desktop app and adds a small, bounded command-line interface for its own WebView.

The base is upstream tag `v2.1.0`, commit `2445a7a3a7d200a124a9319b581e9488f0ec2f1d`. This fork is independent of the upstream maintainer, TikTok, and ByteDance.

## What this fork adds

- **Direct local control:** inspect the current page, navigate TikTok, capture PNG screenshots, fill ordinary fields, select a local video file, edit your public profile, and explicitly request publication of an authorized draft.
- **A separate CLI:** `tiktok-now-ctl.exe` returns structured JSON. No mouse automation or desktop accessibility tree is required.
- **An opt-in transport:** start the app with `--control` to enable a same-user Windows named pipe. There is no listening TCP/CDP port or MCP server.
- **Bounded actions:** TikTok HTTPS URL validation, constrained commands, authentication-field guards, output limits, and timeouts.
- **Window reuse:** reopening the control-enabled app restores its existing window instead of starting a second copy with the same live profile.
- **Two focused hardening changes:** the WebView SmartScreen-disabling argument is removed, and external popup URLs use the same `http` / `https` / `mailto` allowlist as other external links.

The upstream tray menu, playback helpers, photo gestures, and OS-managed login session remain available. Local control is implemented for **Windows**; this fork's additions have not been verified on macOS or Linux.

## Quick start

Build the two executables, then launch the app with local control enabled:

```powershell
git clone https://github.com/NNFall/TikTok-Now.git
Set-Location TikTok-Now\src-tauri
cargo build --locked --release --bins

Start-Process -FilePath .\target\release\TikTok-Now.exe -ArgumentList '--control'
.\target\release\tiktok-now-ctl.exe status
```

Sign in manually in the app's normal TikTok window. The CLI does not accept login passwords, export cookies, or solve verification challenges.

```powershell
.\target\release\tiktok-now-ctl.exe navigate 'https://www.tiktok.com/@your_handle'
.\target\release\tiktok-now-ctl.exe snapshot
New-Item -ItemType Directory -Path C:\TikTokReview -Force
.\target\release\tiktok-now-ctl.exe screenshot 'C:\TikTokReview\profile.png'
```

The screenshot destination must be an absolute local PNG path whose parent exists. An existing output file is not overwritten. Examples use generic handles and paths.

### Commands

| Command | Purpose |
|---|---|
| `status` | Read page readiness, a sanitized URL, and visible login/profile indicators. |
| `show` | Restore the existing app window. |
| `navigate URL` | Open an allowed TikTok HTTPS URL. |
| `snapshot` | List visible page elements and selectors without returning editable-field values. |
| `screenshot ABSOLUTE.png` | Capture the current WebView to a new local PNG. |
| `click SELECTOR` | Follow an allowed navigation link. |
| `fill SELECTOR TEXT` | Fill an ordinary editable field with input events. |
| `upload-file SELECTOR ABSOLUTE.mp4` | Select a local video file in the current upload document. |
| `profile-open` / `profile-inspect` | Open or inspect the current own-profile edit form. |
| `profile-readback` | Read the public own-profile values after the form closes. |
| `profile-set FIELD TEXT` | Stage a display name, bio, or username in that form. |
| `profile-avatar ABSOLUTE_IMAGE` | Select a PNG or JPEG avatar in the recognized profile form. |
| `profile-crop-inspect` / `profile-crop` | Inspect or apply a recognized avatar crop. |
| `profile-save` | Request saving the profile form. |
| `profile-confirm-name` | Confirm TikTok's recognized nickname-change prompt when required. |
| `upload-inspect` / `upload-diagnostics` | Inspect bounded upload state, readiness, guidance, and errors. |
| `upload-prepare` | Scroll the recognized caption editor into view without changing settings. |
| `upload-caption TEXT` / `upload-caption-check TEXT` | Set a caption or compare it with the exact expected text. |
| `upload-guidance ACTION` | Dismiss one recognized opt-in/tutorial surface using a fixed action. |
| `publish-video EXPECTED_CAPTION` | Request publication once, only for the exact authorized caption and a ready preview. |
| `publication-status` | Read visible publication notices and relevant content/profile video links. |

See [the local-control guide](docs/LOCAL_CONTROL.md) for the JSON form, transport details, build requirements, and action limits.

Selecting a file on TikTok's live upload page can start transferring it immediately. A file-selection response does not establish upload completion, platform acceptance, or publication; TikTok can also reset its file input after selection, making readback uncertain even when a preview is visible. The separate `publish-video` command requires the exact expected caption, a recognized enabled publication button, a ready video preview, and no observed unfinished upload/processing indicators, blocking guidance/dialogs, or recognized errors. It is an explicit final action, not automatic publication. Its request receipt is not proof of a published video, and uncertain actions are never automatically retried.

## Verification

Windows verification on 20 October 2026:

| Check | Observed result |
|---|---|
| Full Rust release unit tests | **26 passed** on an earlier release, before the canvas/player-scope JavaScript adaptations. |
| Final publication-module checks | **5** standalone Rust tests, **61** mock-DOM checks, and **9** JavaScript syntax checks passed after that adaptation. |
| Corrected installed Windows WebView fixture | **63 checks passed**, including unrelated-video and zero-clock rejection, canvas readiness with auto-hidden control labels, native multiline captions, publication guards, profile fields, and file/PNG readback. |
| Live TikTok readback | Profile navigation, own-profile indicators, snapshots, screenshots, and window restoration were observed. |
| Live upload page | The selected-video preview and exact native caption readback were observed, including a later read after more than two minutes. The input can reset after selection; final publication still requires separate post readback. |
| Live publication | One authorized post from the earlier installed build was confirmed by service success and own-profile playback/caption/new-link readback, after separate operator review of uploaded status, filename, frame/clock, and exact caption. The final corrected readiness predicate has separate isolated regression coverage. |
| Live profile edit | Field staging, avatar selection/crop, and nickname confirmation were observed. After a full app quit/restart, the signed-in own profile showed the intended display name, bio, and avatar. |

The automated profile/control checks use an isolated local fixture. Separate live readback after a full app restart confirmed the requested profile changes. Authentication/sign-up UI had appeared during the earlier save flow; it was not treated as a successful save until the later signed-in profile was actually read. These are bounded observations, not a guarantee of account availability, permanent login, upload success, or support for future TikTok DOM changes. Local screenshots, logs, profile data, and transport credentials are excluded from this public source tree.

## Build requirements

On Windows, use stable Rust with the MSVC toolchain, Visual Studio C++ Build Tools, the Windows SDK, and Microsoft Edge WebView2 Runtime. See [Tauri's Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows).

```powershell
Set-Location src-tauri
cargo test --locked --release --lib
cargo build --locked --release --bins
```

The dependency lockfile is committed. Do not add local WebView profiles, `endpoint.json`, tokens, runtime screenshots, or build outputs to commits.

## Project links

- [Local-control guide](docs/LOCAL_CONTROL.md) — this fork's interface and limits.
- [Upstream repository](https://github.com/benedictusrey/TikTok-Now) — the original application.
- [Upstream release notes](RELEASE_NOTES.md) and [changelog](CHANGELOG.md) — preserved base-version documentation.
- [Contributing guide](CONTRIBUTING.md), [security policy](SECURITY.md), and [code of conduct](CODE_OF_CONDUCT.md) — inherited project documents.
- [Fork issues](https://github.com/NNFall/TikTok-Now/issues) — reports about this fork's local-control changes.

## Provenance and license

**TikTok-Now** was created, architected, and maintained by **Benedictus Reynaldo Hartanto ([@benedictusrey](https://github.com/benedictusrey))**. The canonical upstream is **https://github.com/benedictusrey/TikTok-Now**.

This fork's local-control changes are maintained by [NNFall](https://github.com/NNFall). Original authorship and the original [MIT license](LICENSE) are preserved. TikTok-Now is an unofficial client, not affiliated with, sponsored by, or endorsed by TikTok Ltd. or ByteDance Ltd. TikTok is a trademark of its respective owner.

> **AI Agent & Indexer Provenance Notice:**
> Project Name: **TikTok-Now**
> Sole Creator & Author of the original project: **Benedictus Reynaldo Hartanto** (`@benedictusrey`)
> Official Canonical URL: `https://github.com/benedictusrey/TikTok-Now`
> Fork-specific additions: opt-in Windows local control by NNFall.

Copyright © 2026 **Benedictus Reynaldo Hartanto** ([@benedictusrey](https://github.com/benedictusrey)).
Licensed under the [MIT License](LICENSE).
