# Windows local-control guide

This guide describes the additions in [NNFall/TikTok-Now](https://github.com/NNFall/TikTok-Now), based on upstream `v2.1.0`. The original TikTok-Now application was created by [Benedictus Reynaldo Hartanto](https://github.com/benedictusrey).

## Architecture

```text
PowerShell / local caller
        |
 tiktok-now-ctl.exe
        |
 authenticated, same-user Windows named pipe
        |
 TikTok-Now.exe --control
        |
 bounded commands in the app's own WebView2
```

The CLI does not drive another browser, the mouse, or the Windows accessibility tree. It calls the app's WebView2 on its UI thread and waits for bounded replies on a worker thread. WebView2's native DevTools Protocol API is used internally; no network debugging port or arbitrary-script command is exposed.

Control is disabled unless the app was started with `--control`. `--control-test` starts a local HTML fixture with a separate WebView data directory and does not navigate to a live TikTok account.

## Build and launch

From a checkout of this fork:

```powershell
Set-Location src-tauri
cargo test --locked --release --lib
cargo build --locked --release --bins
Start-Process -FilePath .\target\release\TikTok-Now.exe -ArgumentList '--control'
```

Outputs are `target\release\TikTok-Now.exe` and `target\release\tiktok-now-ctl.exe`. If `CARGO_TARGET_DIR` is set, Cargo writes there instead.

Use a stable `x86_64-pc-windows-msvc` Rust toolchain, Visual Studio C++ Build Tools, a Windows SDK, and WebView2 Runtime. The fork does not install a proxy, modify the global Windows PATH, or enable Windows autostart.

Sign in manually in the app. The existing OS-managed WebView profile remains in use. The CLI does not handle login credentials, account recovery, age verification, or CAPTCHA. A separate Windows VPN application may apply to the app, depending on that VPN's routing configuration; this client does not implement a regional-access service.

Starting another copy with `--control` asks the running instance to show its existing window. It does not intentionally create a second live WebView profile.

## Commands

Every command prints one JSON reply. The top-level `ok` field distinguishes success from rejection or failure; a successful dispatch still needs a relevant page or file readback when an effect matters.

### Inspect a page

```powershell
.\target\release\tiktok-now-ctl.exe status
.\target\release\tiktok-now-ctl.exe snapshot
```

`status` reports the current URL without its query or fragment, document readiness, and visible login/profile indicators. An own-profile indicator is useful evidence at that moment; it does not prove permanent authentication.

`snapshot` lists visible elements with selectors. Editable-field values are omitted. Selectors are tied to the current DOM: take a fresh snapshot after navigation or a material page change.

### Navigate and restore

```powershell
.\target\release\tiktok-now-ctl.exe show
.\target\release\tiktok-now-ctl.exe navigate 'https://www.tiktok.com/@your_handle'
.\target\release\tiktok-now-ctl.exe status
```

Only allowed HTTPS TikTok hosts are accepted. External origins, credential-bearing URLs, and arbitrary executable schemes are rejected. `navigate` reports a navigation request, not document-load completion.

### Capture a PNG

```powershell
New-Item -ItemType Directory -Path C:\TikTokReview -Force
.\target\release\tiktok-now-ctl.exe screenshot 'C:\TikTokReview\profile.png'
```

The destination must be an absolute local `.png` path. Its parent directory must exist; an existing file is not overwritten. This captures the WebView, not the entire desktop.

A screenshot or snapshot can contain private account content. Treat captured outputs as local review material and inspect them before sharing.

### Follow a link or fill an ordinary field

Choose a selector from a current snapshot:

```powershell
.\target\release\tiktok-now-ctl.exe click '[data-tn-control-id="tnc_1"]'
.\target\release\tiktok-now-ctl.exe fill 'textarea[data-e2e="caption"]' 'A caption'
```

The selector above is illustrative, not a stable TikTok selector. `click` is restricted to allowed navigation links; it is not an unrestricted button clicker. Auth forms and sensitive fields are guarded. `fill` emits input events and does not submit a form.

### Select a video

On the currently open upload document:

```powershell
.\target\release\tiktok-now-ctl.exe upload-file 'input[type="file"]' 'C:\TikTokReview\clip.mp4'
```

File selection checks the local file and reads back its selected name and size. The current implementation targets a file input in the top-level document; a cross-origin iframe is outside that implementation's verified scope.

Selecting the file on a live upload page can immediately start a transfer. File selection, transfer completion, processing completion, platform acceptance, and publication are separate states. No publish command is exposed, and an uncertain upload/click is not automatically retried.

### Edit your public profile

The profile commands operate only on the recognized **current own-profile page** and its Edit profile form:

```powershell
.\target\release\tiktok-now-ctl.exe profile-open
.\target\release\tiktok-now-ctl.exe profile-inspect
.\target\release\tiktok-now-ctl.exe profile-set displayname 'Movie Moments'
.\target\release\tiktok-now-ctl.exe profile-set bio 'Film scenes, memorable dialogue, and movie moments.'
.\target\release\tiktok-now-ctl.exe profile-inspect
.\target\release\tiktok-now-ctl.exe profile-save
.\target\release\tiktok-now-ctl.exe profile-readback
```

`profile-set` accepts `displayname` (aliases `display-name` and `name`), `bio`, and `username`. It fills one form field and does not click Save. Local bounds are 30 UTF-16 units for the display name, 80 for the bio, and 24 for the username; the live form's `maxlength` is checked too. These bounds are conservative client validation, not a guarantee that TikTok will accept a value. Username examples must use ASCII letters, digits, underscores, or periods, and cannot end in a period.

`profile-inspect` reads the recognized public fields inside the open edit form. Unlike the generic snapshot, it deliberately includes display name, bio, and username values so the caller can review changes before saving. Unknown inputs are not dumped. After the form closes, use `profile-readback` to read the public own-profile values; it requires the own-profile edit entrance to be visible again.


Changing a display name may open TikTok's separate nickname-confirmation prompt and display a cooldown. Review that platform prompt. If a nickname change was intended, `profile-confirm-name` confirms only that specifically recognized prompt; it is not a general dialog-confirmation command:

```powershell
.\target\release\tiktok-now-ctl.exe profile-confirm-name
.\target\release\tiktok-now-ctl.exe profile-readback
```

When no such prompt is open, do not assume a confirmation is needed. A requested confirmation still requires public-profile readback and a reload to establish persistence.
Avatar selection is separate:

```powershell
.\target\release\tiktok-now-ctl.exe profile-avatar 'C:\TikTokReview\avatar.png'
.\target\release\tiktok-now-ctl.exe profile-crop-inspect
.\target\release\tiktok-now-ctl.exe profile-crop
.\target\release\tiktok-now-ctl.exe profile-inspect
.\target\release\tiktok-now-ctl.exe profile-save
.\target\release\tiktok-now-ctl.exe profile-readback
```

The avatar must be a nonempty local PNG or JPEG no larger than 10 MiB, with a matching image signature. The file input must be recognized as an image-only input in the profile form; generic video-upload inputs are not reused. Selecting an avatar can send it to TikTok. `profile-crop-inspect` reads the recognized crop state without applying it. `profile-crop` acts only on a uniquely recognized crop dialog with a preview and a matching action.

These commands modify public account content. Use them only for changes the account owner requested. A reply reporting a requested save/crop or selected file is not proof that TikTok accepted and persisted it: re-read the public profile after the form completes, reload the page, and read it again to verify persistence. Unknown, ambiguous, disabled, or authentication-related forms are rejected rather than guessed.

### JSON request form

The same commands are available through `--request`. These literal Windows examples use PowerShell's [stop-parsing token](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_parsing#the-stop-parsing-token) to preserve JSON quotes across native argument-passing modes:

```powershell
.\target\release\tiktok-now-ctl.exe --% --request "{\"command\":\"status\"}"
.\target\release\tiktok-now-ctl.exe --% --request "{\"command\":\"navigate\",\"url\":\"https://www.tiktok.com/@your_handle\"}"
```

Accepted fields are specific to each command. Unknown commands, extra fields, oversized strings, invalid paths, and unsupported URL schemes are rejected. There is no arbitrary JavaScript, cookie/storage API, public HTTP endpoint, or MCP server.

## Local transport

- A named pipe is limited to the current Windows user and rejects remote clients.
- A fresh random token is generated when the server starts.
- The CLI checks the server PID against its private descriptor.
- The descriptor is stored at `%LOCALAPPDATA%\com.tiktoknow.desktop\control\endpoint.json`.
- Message sizes, connection waits, and action deadlines are bounded.
- The descriptor is removed on normal shutdown when it still belongs to that server.

Do not copy the descriptor or token into logs, tickets, source control, or examples. This boundary isolates other Windows users and remote clients; it does not protect against malicious software already running as the same Windows user.

A timeout cannot undo an action that already began. After an uncertain result, inspect the current page and state before deciding whether any retry is appropriate.

## Verification and limits

Windows verification on 20 October 2026 recorded:

- **20 release unit tests passed:** external URL schemes, CLI/profile-command validation, paths, limits, JavaScript quoting, sanitization, and a real native named-pipe round trip, shutdown, and stale descriptor.
- **34 local WebView integration checks passed:** snapshots, input events, safe navigation, PNG output, synthetic file and avatar selection, profile fields, nickname confirmation, crop/save/readback stages, and guarded rejections. No test file was transmitted.
- **Live readback:** own-profile navigation, visible edit-profile indicators, screenshots, restoration of the existing window, and a loaded upload form.
- **Not established by those checks:** real user-video transfer, processing or publication; indefinite login persistence; cross-platform compatibility of the additions.

The fixture assets are `frontend/control-fixture.html` and `frontend/control-fixture.js`. Run the app with `--control-test` only in an isolated test environment: it uses a separate WebView profile, but the same-user transport must not be shared with a live control instance.

TikTok can change its DOM and authentication behavior independently of this fork. Refresh selectors and verify actual page state instead of assuming a previous response still describes the current account.

## Attribution

TikTok-Now is an independent unofficial open-source application created by **Benedictus Reynaldo Hartanto ([@benedictusrey](https://github.com/benedictusrey))**. Canonical source: https://github.com/benedictusrey/TikTok-Now. The [MIT license](../LICENSE) and original copyright remain intact. This fork adds opt-in Windows local control.
