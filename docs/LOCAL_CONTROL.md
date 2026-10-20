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

File selection checks the local file and attempts to read back its selected name and size. The current implementation targets a file input in the top-level document; a cross-origin iframe is outside that implementation's verified scope. TikTok can reset that input after handling the selection. In that case, selection readback can be uncertain even when the live page shows the intended video preview: inspect the upload state before deciding what happened, and do not automatically select the file again.

Selecting the file on a live upload page can immediately start a transfer. File selection, transfer completion, processing completion, platform acceptance, and publication are separate states. Publication is available only through the separate typed final action below; an uncertain upload or publication request is not automatically retried.

### Inspect and prepare an upload

These commands operate on recognized ordinary TikTok upload routes:

```powershell
.\target\release\tiktok-now-ctl.exe upload-inspect
.\target\release\tiktok-now-ctl.exe upload-diagnostics
.\target\release\tiktok-now-ctl.exe upload-prepare
.\target\release\tiktok-now-ctl.exe upload-caption 'A memorable movie scene. #movies'
.\target\release\tiktok-now-ctl.exe upload-caption-check 'A memorable movie scene. #movies'
.\target\release\tiktok-now-ctl.exe upload-inspect
```

`upload-inspect` reports the caption editor's metadata, selected-file metadata, decoded preview readiness, progress, recognized errors, publication-button state, and blocking guidance. `upload-diagnostics` adds bounded structural metadata for supported upload elements, including read-only canvas and open-shadow-root observations. The presence of a canvas or shadow root does not by itself establish a ready decoded preview. These commands do not expose cookies, session storage, request tokens, or video-source URLs.

`upload-prepare` only scrolls the recognized caption editor into view. It does not change caption text, visibility, scheduling, or content-check preferences. `upload-caption` prepares and focuses the recognized caption editor, inserts text through WebView2's native `Input.insertText` path, and performs a separate exact readback. Draft.js block text is read with its line breaks preserved; the command does not insert raw HTML. `upload-caption-check` compares the current caption with the supplied text and returns a boolean plus lengths without dumping its contents.

A live editor focus/reconciliation can leave pre-existing text, such as an initial filename prefix. Exact readback rejects that state. Inspect the current editor before any intentional further entry; an observed success required an exact match after that inspection. The command does not automatically repeat native insertion.

The local caption limit is 2,200 UTF-16 units, with a nonempty value required; the live field's maximum is checked too. This is a conservative client bound, not a statement of TikTok's current service limit.

Two fixed guidance actions are supported when their specific recognized panels are present:

| Action | Effect |
|---|---|
| `upload-guidance cancel-auto-check` | Cancel the observed automatic-content-check opt-in prompt; do not enable or disable an account preference. |
| `upload-guidance acknowledge-tutorial` | Acknowledge the observed informational upload tutorial. |

Neither action clicks Publish or accepts an unknown dialog. Inspect again after a guidance action. Unknown, ambiguous, disabled, authentication-related, or unfinished states fail closed.

### Request publication explicitly

`publish-video` is a separate final command for a video and caption the account owner has **explicitly authorized for publication**. It is not part of navigation, file selection, caption filling, or the generic `click` command. It uses the draft's current visibility and other settings rather than silently changing them.

After reviewing the selected video, its ready preview, caption match, and current settings:

```powershell
.\target\release\tiktok-now-ctl.exe publish-video 'A memorable movie scene. #movies'
.\target\release\tiktok-now-ctl.exe publication-status
```

Before dispatch, the command requires:

- Exactly one recognized caption editor whose text matches `EXPECTED_CAPTION`.
- A recognized enabled `post_video_button` with a publication label.
- A displayed ready preview through one of the two recognized routes described below.
- No observed unfinished progress indicator or recognized upload/processing label, blocking dialog/guidance, or recognized upload error.
- No publication request already marked in the current document.


The preview predicate accepts either:

- A displayed HTML video **inside the unique recognized `mobile_preview_container`**, with `readyState >= 2`, positive duration, and positive decoded frame dimensions. Tutorial or advertisement videos elsewhere in Studio do not qualify.
- Exactly one displayed `mobile_preview_container` containing exactly one displayed canvas with positive dimensions, exactly one parseable `HH:MM:SS / HH:MM:SS` clock pair with a positive total duration and current time within its range, and exact play/pause plus mute/unmute labels from that player's DOM schema.

Those control labels may be auto-hidden by the player. Their exact DOM labels identify the recognized schema; they do not establish that a control is currently visible, enabled, or a valid click target. The preview container and canvas themselves must still be displayed.

The canvas route establishes **recognized player UI readiness**, not an HTML video's decoder state or a pixel analysis of the film frame. A generic canvas, ambiguous container/canvas/clock, zero total duration, or missing player controls is not sufficient. Uploaded-file labels and filenames are available in diagnostics; they are observations rather than a new-publication receipt.
The pending marker is set **before** clicking. A successful command receipt reports `publicationRequested: true` and `published: false`: it establishes a request, not a completed publication. The marker prevents another request in that document, not duplicate posts across reloads or restarts. A timeout or thrown click can be uncertain; inspect the result before considering any further action. There is no automatic retry.

`publication-status` is read-only and supports recognized upload, content-list, profile, and video routes. It reports visible notices, headings, relevant video links, and bounded content rows. Read back the actual new post and its current state to establish the outcome. A link, preview, request receipt, absence of errors, or an older post is not sufficient proof of a successful new publication. Publication is also not copyright clearance.

No arbitrary JavaScript, arbitrary button selector, network session API, or general dialog-confirmation command is introduced by this workflow.

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

Profile readback matches the handle in the current profile URL against the observed title/subtitle fields. It supports either observed ordering of display name and username instead of assuming that a single DOM field always represents the handle.

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

- **26 full release unit tests passed on an earlier release, before the canvas/player-scope JavaScript adaptations:** external URL schemes, typed CLI/profile/publication-command validation, caption/profile limits, paths, JavaScript quoting, sanitization, and a real native named-pipe round trip, shutdown, and stale descriptor.
- **Final publication-module checks passed after that adaptation:** 5 standalone Rust tests, 61 mock-DOM checks, and 9 JavaScript syntax checks. This is a separate scope from the preceding full Rust run.
- **63 native WebView fixture checks passed against the corrected final installed Windows GUI:** a ready decoded tutorial video outside the upload preview is rejected; a zero-total clock is rejected; a recognized canvas with auto-hidden control labels is accepted. The same run covers native multiline captions, typed publication guards, both profile field orders, snapshots, safe navigation, PNG output, synthetic file/avatar selection, and crop/save/readback stages. No fixture file was transmitted or posted.
- **Live readback:** own-profile navigation, edit-profile indicators, screenshots, window restoration, and the upload form. The selected-video preview and native caption text were observed with exact caption comparisons, including another successful read more than two minutes later. These observations did not establish publication.
- **Separate live profile-edit observation:** form fields, avatar selection/crop, and nickname confirmation were exercised. Authentication/sign-up UI appeared during the save flow. After the user fully quit and reopened the app, the signed-in own profile was re-read and the intended display name, bio, and avatar were present. That later observation established persistence across that restart; authentication was not automated.
- **Separate live publication observation on the earlier installed development build:** before the single authorized publication, the operator separately reviewed uploaded status, the exact selected filename, film frame/canvas and clock, and the expected caption. Service success plus own-profile playback, caption, and new-video-link readback then confirmed that post. This action preceded the final preview-scope correction. The corrected build's readiness behavior is supported by the isolated regression checks above, rather than that earlier live action.
- **Not established by those checks:** indefinite login, profile or post availability; copyright clearance; cross-platform compatibility of the additions.

The fixture assets are `frontend/control-fixture.html` and `frontend/control-fixture.js`. Run the app with `--control-test` only in an isolated test environment: it uses a separate WebView profile, but the same-user transport must not be shared with a live control instance.

TikTok can change its DOM and authentication behavior independently of this fork. Refresh selectors and verify actual page state instead of assuming a previous response still describes the current account.

## Attribution

TikTok-Now is an independent unofficial open-source application created by **Benedictus Reynaldo Hartanto ([@benedictusrey](https://github.com/benedictusrey))**. Canonical source: https://github.com/benedictusrey/TikTok-Now. The [MIT license](../LICENSE) and original copyright remain intact. This fork adds opt-in Windows local control.
