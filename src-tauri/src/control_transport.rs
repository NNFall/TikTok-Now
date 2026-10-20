//! Opt-in, same-user local control. No TCP listener, arbitrary script command,
//! page-facing Tauri command or cookie export is exposed. Publication uses a
//! separate typed command with exact-caption and preview readiness checks.
//! The host callback must implement its own action/authentication restrictions
//! and must finish within the deadline; timing out cannot undo a started action.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc},
    thread,
    time::{Duration, Instant},
};
use std::os::windows::{ffi::OsStrExt, io::{AsRawHandle, FromRawHandle, OwnedHandle}};
use windows::{
    core::{BOOL, PCWSTR, PWSTR},
    Win32::{
        Foundation::{GetLastError, LocalFree, HANDLE, HLOCAL,
            ERROR_ALREADY_EXISTS, ERROR_NO_DATA,
            ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING,
            GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Security::{Authorization::{ConvertSidToStringSidW,
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
            SE_FILE_OBJECT}, Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
            GetSecurityDescriptorDacl, GetTokenInformation, PSECURITY_DESCRIPTOR,
            SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
            DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION},
        Storage::FileSystem::{CreateDirectoryW, CreateFileW,
            ReadFile, WriteFile, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_FIRST_PIPE_INSTANCE,
            FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_NONE, OPEN_EXISTING,
            PIPE_ACCESS_DUPLEX, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT},
        System::{Pipes::{ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
            GetNamedPipeClientProcessId, GetNamedPipeServerProcessId, SetNamedPipeHandleState, PIPE_NOWAIT,
            PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE},
            Threading::{GetCurrentProcess, OpenProcessToken}},
    },
};

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_DESCRIPTOR_BYTES: u64 = 4096;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const EXECUTION_TIMEOUT: Duration = Duration::from_secs(20);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const PROTOCOL_VERSION: u32 = 1;

pub type Handler = Arc<dyn Fn(Value) -> Value + Send + Sync + 'static>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Endpoint {
    version: u32,
    pipe: String,
    server_pid: u32,
    token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    token: String,
    request: Value,
}

/// Keep this guard alive for as long as local control is enabled.
pub struct ControlServer {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    endpoint_path: PathBuf,
}

impl ControlServer {
    pub fn endpoint_path(&self) -> &Path { &self.endpoint_path }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Pipe I/O and callback waits check stop. Do not block GUI shutdown on
        // a faulty host callback; its one worker cannot execute queued actions.
        if let Some(worker) = self.thread.take() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !worker.is_finished() && Instant::now() < deadline {
                thread::sleep(POLL_INTERVAL);
            }
            if worker.is_finished() { let _ = worker.join(); }
        }
    }
}

fn error(message: &str) -> Value { json!({"ok": false, "error": message}) }

fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

fn raw(handle: &OwnedHandle) -> HANDLE { HANDLE(handle.as_raw_handle()) }

fn owned(handle: HANDLE) -> Result<OwnedHandle, String> {
    if handle == INVALID_HANDLE_VALUE || handle.is_invalid() {
        Err("Windows could not create the local control handle".into())
    } else {
        // SAFETY: the caller passes a newly owned native handle exactly once.
        Ok(unsafe { OwnedHandle::from_raw_handle(handle.0) })
    }
}

fn current_user_sid() -> Result<String, String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|_| "Cannot identify the current Windows user")?;
        let token = owned(token)?;
        let mut bytes = 0;
        let _ = GetTokenInformation(raw(&token), TokenUser, None, 0, &mut bytes);
        if bytes == 0 || bytes > 64 * 1024 { return Err("Invalid Windows user token".into()); }
        // TOKEN_USER includes pointers; allocate with native pointer alignment.
        let mut storage = vec![0usize; (bytes as usize).div_ceil(std::mem::size_of::<usize>())];
        GetTokenInformation(raw(&token), TokenUser, Some(storage.as_mut_ptr().cast()), bytes, &mut bytes)
            .map_err(|_| "Cannot read the current Windows user identity")?;
        let user = &*(storage.as_ptr() as *const TOKEN_USER);
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text)
            .map_err(|_| "Cannot encode the current Windows user identity")?;
        let result = text.to_string().map_err(|_| "Invalid Windows SID encoding".to_string());
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        result
    }
}

struct PrivateSecurity(PSECURITY_DESCRIPTOR);

impl PrivateSecurity {
    fn new(sid: &str, directory: bool) -> Result<Self, String> {
        let inheritance = if directory { "OICI" } else { "" };
        let sddl = wide(format!("O:{sid}D:P(A;{inheritance};GA;;;{sid})"));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()), 1, &mut descriptor, None,
        ) }.map_err(|_| "Cannot create a private Windows security descriptor")?;
        Ok(Self(descriptor))
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0.0,
            bInheritHandle: BOOL(0),
        }
    }

    fn protect_directory(&self, path: &Path) -> Result<(), String> {
        let path = wide(path);
        unsafe {
            let mut present = BOOL(0);
            let mut defaulted = BOOL(0);
            let mut acl = std::ptr::null_mut();
            GetSecurityDescriptorDacl(self.0, &mut present, &mut acl, &mut defaulted)
                .map_err(|_| "Cannot read the private control directory ACL")?;
            if !present.as_bool() || acl.is_null() { return Err("Missing private control ACL".into()); }
            SetNamedSecurityInfoW(PCWSTR(path.as_ptr()), SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                None, None, Some(acl), None)
                .ok().map_err(|_| "Cannot restrict the control directory to this Windows user".to_string())
        }
    }
}

impl Drop for PrivateSecurity {
    fn drop(&mut self) { unsafe { let _ = LocalFree(Some(HLOCAL(self.0.0))); } }
}

fn descriptor_path() -> Result<PathBuf, String> {
    let app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or("LOCALAPPDATA is unavailable for local control")?;
    let root = PathBuf::from(app_data);
    if !root.is_absolute() { return Err("LOCALAPPDATA must be an absolute directory".into()); }
    Ok(root.join("com.tiktoknow.desktop").join("control").join("endpoint.json"))
}

fn refuse_reparse(path: &Path) -> Result<(), String> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            return Err("Local control paths must not be links or reparse points".into());
        }
    }
    Ok(())
}

fn write_private_endpoint(path: &Path, endpoint: &Endpoint, sid: &str) -> Result<(), String> {
    let directory = path.parent().ok_or("Missing control directory")?;
    let app_directory = directory.parent().ok_or("Missing app data directory")?;
    refuse_reparse(app_directory)?;
    fs::create_dir_all(app_directory).map_err(|_| "Cannot create the app data directory")?;
    refuse_reparse(directory)?;
    let security = PrivateSecurity::new(sid, true)?;
    let attributes = security.attributes();
    let directory_wide = wide(directory);
    if unsafe { CreateDirectoryW(PCWSTR(directory_wide.as_ptr()), Some(&attributes)) }.is_err()
        && unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
        return Err("Cannot create the private control directory".into());
    }
    security.protect_directory(directory)?;
    refuse_reparse(path)?;
    if path.exists() {
        fs::remove_file(path).map_err(|_| "Cannot replace a stale control descriptor")?;
    }
    let security = PrivateSecurity::new(sid, false)?;
    let attributes = security.attributes();
    let path_wide = wide(path);
    let handle = unsafe { CreateFileW(PCWSTR(path_wide.as_ptr()), GENERIC_WRITE.0,
        FILE_SHARE_NONE, Some(&attributes), CREATE_NEW, FILE_ATTRIBUTE_NORMAL, None) }
        .map_err(|_| "Cannot create the private control descriptor")?;
    let file = owned(handle)?;
    let encoded = serde_json::to_vec(endpoint).map_err(|_| "Cannot encode control descriptor")?;
    let mut written = 0;
    unsafe { WriteFile(raw(&file), Some(&encoded), Some(&mut written), None) }
        .map_err(|_| "Cannot write the private control descriptor")?;
    if written as usize != encoded.len() { return Err("Incomplete control descriptor write".into()); }
    Ok(())
}

fn read_endpoint(path: &Path) -> Result<Endpoint, String> {
    refuse_reparse(path)?;
    if let Some(parent) = path.parent() { refuse_reparse(parent)?; }
    let file = fs::File::open(path).map_err(|_| "Local control is off; start the app with --control")?;
    if file.metadata().map_err(|_| "Cannot inspect the control descriptor")?.len() > MAX_DESCRIPTOR_BYTES {
        return Err("Invalid local control descriptor size".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_DESCRIPTOR_BYTES + 1).read_to_end(&mut bytes)
        .map_err(|_| "Cannot read the local control descriptor")?;
    if bytes.len() as u64 > MAX_DESCRIPTOR_BYTES { return Err("Invalid local control descriptor size".into()); }
    let endpoint: Endpoint = serde_json::from_slice(&bytes).map_err(|_| "Invalid local control descriptor")?;
    let sid = current_user_sid()?;
    if endpoint.version != PROTOCOL_VERSION || endpoint.pipe != pipe_name(&sid)
        || endpoint.token.len() != 64 || !endpoint.token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid or incompatible local control descriptor".into());
    }
    Ok(endpoint)
}

fn pipe_name(sid: &str) -> String { format!(r"\\.\pipe\TikTokNow.Control.{sid}") }

fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok().map_err(|_| "Cannot generate local control authentication")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn token_matches(left: &str, right: &str) -> bool {
    if left.len() != 64 || right.len() != 64 { return false; }
    left.bytes().zip(right.bytes()).fold(0u8, |diff, (a,b)| diff | (a ^ b)) == 0
}

fn check_deadline(stop: &AtomicBool, deadline: Instant) -> Result<(), String> {
    if stop.load(Ordering::Acquire) { return Err("Local control is shutting down".into()); }
    if Instant::now() >= deadline { return Err("Local control operation timed out".into()); }
    Ok(())
}

fn read_exact(handle: HANDLE, buffer: &mut [u8], stop: &AtomicBool, deadline: Instant) -> Result<(), String> {
    let mut offset = 0;
    while offset < buffer.len() {
        check_deadline(stop, deadline)?;
        let mut read = 0;
        let result = unsafe { ReadFile(handle, Some(&mut buffer[offset..]), Some(&mut read), None) };
        if result.is_ok() && read > 0 { offset += read as usize; continue; }
        let code = unsafe { GetLastError() };
        if result.is_err() && code != ERROR_NO_DATA {
            return Err("Local control connection closed while reading".into());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

fn write_all(handle: HANDLE, bytes: &[u8], stop: &AtomicBool, deadline: Instant) -> Result<(), String> {
    let mut offset = 0;
    while offset < bytes.len() {
        check_deadline(stop, deadline)?;
        let mut written = 0;
        let result = unsafe { WriteFile(handle, Some(&bytes[offset..]), Some(&mut written), None) };
        if result.is_err() { return Err("Local control connection closed while writing".into()); }
        offset += written as usize;
        if written == 0 { thread::sleep(POLL_INTERVAL); }
    }
    Ok(())
}

fn read_frame(handle: HANDLE, maximum: usize, stop: &AtomicBool, deadline: Instant) -> Result<Vec<u8>, String> {
    let mut header = [0u8;4];
    read_exact(handle, &mut header, stop, deadline)?;
    let length = u32::from_le_bytes(header) as usize;
    validate_frame_length(length, maximum)?;
    let mut bytes = vec![0u8;length];
    read_exact(handle, &mut bytes, stop, deadline)?;
    Ok(bytes)
}

fn validate_frame_length(length: usize, maximum: usize) -> Result<(), String> {
    if length == 0 || length > maximum { Err("Local control message exceeds the size limit".into()) } else { Ok(()) }
}

fn write_frame(handle: HANDLE, value: &Value, maximum: usize, stop: &AtomicBool, deadline: Instant) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "Cannot encode the local control reply")?;
    validate_frame_length(bytes.len(), maximum)?;
    write_all(handle, &(bytes.len() as u32).to_le_bytes(), stop, deadline)?;
    write_all(handle, &bytes, stop, deadline)
}

/// Validate a flat public command. The host must separately restrict actions
/// using the actual current URL/DOM; a CSS selector is not an authorization.
pub fn validate_request(value: &Value) -> Result<(), String> {
    let object = value.as_object().ok_or("A local control command must be a JSON object")?;
    let command = object.get("command").and_then(Value::as_str).ok_or("Missing control command")?;
    let keys: &[&str] = match command {
        "status" | "snapshot" | "show" | "profile-open" | "profile-inspect" | "profile-readback" | "profile-save" | "profile-confirm-name" | "profile-crop" | "profile-crop-inspect" => &["command"],
        "navigate" => &["command", "url"],
        "screenshot" => &["command", "path"],
        "click" => &["command", "selector"],
        "fill" => &["command", "selector", "text"],
        "upload-file" => &["command", "selector", "path"],
        "profile-set" => &["command", "field", "text"],
        "profile-avatar" => &["command", "path"],
        "upload-inspect" | "upload-diagnostics" | "upload-prepare" | "publication-status" => &["command"],
        "upload-caption" | "upload-caption-check" | "publish-video" => &["command", "text"],
        "upload-guidance" => &["command", "action"],
        _ => return Err("Unsupported control command".into()),
    };
    if object.len() != keys.len() || object.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err("Unexpected or missing control command fields".into());
    }
    for key in keys.iter().filter(|key| **key != "command") {
        let field = object.get(*key).and_then(Value::as_str).ok_or("Control command fields must be strings")?;
        let maximum = match *key { "text" => 8192, "path" => 32767, _ => 2048 };
        if field.len() > maximum || field.contains('\0') || (field.is_empty() && *key != "text") {
            return Err("Invalid control command field length".into());
        }
    }
    if command == "navigate" {
        let url = tauri::Url::parse(object["url"].as_str().unwrap()).map_err(|_| "Invalid navigation URL")?;
        let host = url.host_str().unwrap_or("");
        if url.scheme() != "https" || !(host == "tiktok.com" || host.ends_with(".tiktok.com"))
            || !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
            return Err("Navigation is limited to HTTPS TikTok pages".into());
        }
    }
    if command == "profile-set" {
        crate::profile_scripts::validate_field(object["field"].as_str().unwrap(), object["text"].as_str().unwrap())?;
    }
    if matches!(command,"upload-caption" | "upload-caption-check" | "publish-video") {
        crate::publish_scripts::validate_caption(object["text"].as_str().unwrap())?;
    }
    if command == "upload-guidance" && !matches!(object["action"].as_str().unwrap(),"cancel-auto-check" | "acknowledge-tutorial") {
        return Err("Unsupported upload guidance action".into());
    }
    if matches!(command, "screenshot" | "upload-file" | "profile-avatar") {
        let path = Path::new(object["path"].as_str().unwrap());
        if !path.is_absolute() || path.to_string_lossy().starts_with(r"\\") {
            return Err("A local absolute file path is required".into());
        }
        if command == "screenshot" && !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
            return Err("Screenshot output must be a PNG path".into());
        }
        if command == "profile-avatar" && !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg")) {
            return Err("Profile avatar must be a local PNG or JPEG file".into());
        }
    }
    Ok(())
}

fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace(['-', '_'], "");
    ["cookie", "token", "password", "passwd", "secret", "authorization", "credential", "localstorage", "sessionstorage"]
        .iter().any(|sensitive| key.contains(sensitive))
}

/// Defense in depth for host errors/metadata. Host snapshots must avoid input
/// values and storage themselves; redaction cannot reliably inspect arbitrary
/// prose or image pixels for secrets.
fn sanitize(value: Value, token: &str) -> Value {
    match value {
        Value::Object(object) => Value::Object(object.into_iter().map(|(key,value)| {
            let value = if sensitive_key(&key) { Value::String("[REDACTED]".into()) } else { sanitize(value,token) };
            (key,value)
        }).collect()),
        Value::Array(array) => Value::Array(array.into_iter().map(|v| sanitize(v,token)).collect()),
        Value::String(value) => Value::String(value.replace(token,"[REDACTED]")),
        value => value,
    }
}

/// Start only when the GUI was explicitly launched with --control.
pub fn start_server(handler: Handler) -> Result<ControlServer, String> {
    let sid = current_user_sid()?;
    let endpoint = Endpoint { version: PROTOCOL_VERSION, pipe: pipe_name(&sid),
        server_pid: std::process::id(), token: new_token()? };
    let security = PrivateSecurity::new(&sid, false)?;
    let attributes = security.attributes();
    let name = wide(&endpoint.pipe);
    let native = unsafe { CreateNamedPipeW(PCWSTR(name.as_ptr()),
        PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
        1, 64 * 1024, 64 * 1024, 0, Some(&attributes)) };
    let pipe = owned(native).map_err(|_| "Local control is already running or its private pipe cannot be created")?;
    let path = descriptor_path()?;
    write_private_endpoint(&path, &endpoint, &sid)?;
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let thread_path = path.clone();
    let server = thread::Builder::new().name("tiktok-control-pipe".into()).spawn(move || {
        // Rendezvous: at most one active host action and no queued future
        // actions if a client times out. This worker never receives the token.
        let (dispatch, jobs) = mpsc::sync_channel::<(Value,mpsc::Sender<Value>)>(0);
        let _ = thread::Builder::new().name("tiktok-control-host".into()).spawn(move || {
            while let Ok((request,reply)) = jobs.recv() {
                let _ = reply.send(handler(request));
            }
        });
        while !thread_stop.load(Ordering::Acquire) {
            let connected = unsafe { ConnectNamedPipe(raw(&pipe), None) };
            if connected.is_ok() {
                // In PIPE_NOWAIT mode TRUE after DisconnectNamedPipe means
                // available for a new client, not an established connection.
                thread::sleep(POLL_INTERVAL);
                continue;
            }
            if connected.is_err() {
                let code = unsafe { GetLastError() };
                if code == ERROR_PIPE_LISTENING { thread::sleep(POLL_INTERVAL); continue; }
                if code != ERROR_PIPE_CONNECTED {
                    let _ = unsafe { DisconnectNamedPipe(raw(&pipe)) };
                    thread::sleep(POLL_INTERVAL);
                    continue;
                }
            }
            let mut client_pid = 0;
            if unsafe { GetNamedPipeClientProcessId(raw(&pipe), &mut client_pid) }.is_err()
                || client_pid == 0 {
                let _ = unsafe { DisconnectNamedPipe(raw(&pipe)) };
                continue;
            }
            let response = (|| -> Result<Value,String> {
                let bytes = read_frame(raw(&pipe), MAX_REQUEST_BYTES, &thread_stop, Instant::now()+IO_TIMEOUT)?;
                let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|_| "Invalid control request JSON")?;
                if envelope.version != PROTOCOL_VERSION || !token_matches(&envelope.token,&endpoint.token) {
                    return Err("Local control authentication failed".into());
                }
                validate_request(&envelope.request)?;
                let (reply, result) = mpsc::channel();
                let mut job = (envelope.request,reply);
                let dispatch_deadline = Instant::now()+Duration::from_millis(100);
                loop {
                    match dispatch.try_send(job) {
                        Ok(()) => break,
                        Err(mpsc::TrySendError::Full(returned)) => {
                            // Permit worker startup scheduling without adding
                            // an action queue or retrying a dispatched action.
                            if Instant::now() >= dispatch_deadline { return Err("Local control host is busy".into()); }
                            check_deadline(&thread_stop,dispatch_deadline)?;
                            job = returned;
                            thread::sleep(POLL_INTERVAL);
                        },
                        Err(mpsc::TrySendError::Disconnected(_)) => return Err("Local control host stopped".into()),
                    }
                }
                let deadline = Instant::now()+EXECUTION_TIMEOUT;
                loop {
                    check_deadline(&thread_stop,deadline)?;
                    match result.recv_timeout(Duration::from_millis(50)) {
                        Ok(value) => return Ok(sanitize(value,&endpoint.token)),
                        Err(mpsc::RecvTimeoutError::Timeout) => (),
                        Err(_) => return Err("Local control host stopped".into()),
                    }
                }
            })().unwrap_or_else(|message| error(&message));
            let encoded_size = serde_json::to_vec(&response).map_or(MAX_RESPONSE_BYTES+1, |b| b.len());
            let response = if encoded_size > MAX_RESPONSE_BYTES { error("Local control reply exceeds the size limit") } else { response };
            if write_frame(raw(&pipe), &response, MAX_RESPONSE_BYTES, &thread_stop, Instant::now()+IO_TIMEOUT).is_ok() {
                // DisconnectNamedPipe discards unread data. Await one client
                // ACK instead of a potentially unbounded FlushFileBuffers.
                let _ = read_exact(raw(&pipe),&mut [0u8;1],&thread_stop,Instant::now()+Duration::from_secs(1));
            }
            let _ = unsafe { DisconnectNamedPipe(raw(&pipe)) };
        }
        drop(dispatch);
        drop(pipe);
        // Do not delete another server's descriptor after a stop/restart race.
        if let Ok(found) = read_endpoint(&thread_path) {
            if found.server_pid == endpoint.server_pid && token_matches(&found.token,&endpoint.token) {
                let _ = fs::remove_file(&thread_path);
            }
        }
    }).map_err(|_| "Cannot start the local control thread")?;
    Ok(ControlServer { stop, thread: Some(server), endpoint_path: path })
}

/// One command per authenticated local connection. Never retries an action
/// after sending it: a missing response is an uncertain result, not failure
/// proof, and repeating an upload/click could duplicate its effect.
pub fn send_request(request: Value) -> Result<Value,String> {
    validate_request(&request)?;
    let endpoint = read_endpoint(&descriptor_path()?)?;
    let payload = json!({"version":PROTOCOL_VERSION,"token":endpoint.token,"request":request});
    let size = serde_json::to_vec(&payload).map_err(|_| "Cannot encode the control request")?.len();
    validate_frame_length(size,MAX_REQUEST_BYTES)?;
    let name = wide(&endpoint.pipe);
    let deadline = Instant::now()+Duration::from_secs(3);
    let pipe = loop {
        let opened = unsafe { CreateFileW(PCWSTR(name.as_ptr()), GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_NONE,None,OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(SECURITY_SQOS_PRESENT.0 | SECURITY_IDENTIFICATION.0),None) };
        match opened {
            Ok(handle) => break owned(handle)?,
            Err(_) => {
                if unsafe { GetLastError() } != ERROR_PIPE_BUSY || Instant::now() >= deadline {
                    return Err("Local control is unavailable or stale; start the app with --control".into());
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    };
    let mut server_pid = 0;
    unsafe { GetNamedPipeServerProcessId(raw(&pipe),&mut server_pid) }
        .map_err(|_| "Cannot verify the local control server")?;
    if server_pid != endpoint.server_pid { return Err("Stale local control descriptor; server identity changed".into()); }
    let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
    unsafe { SetNamedPipeHandleState(raw(&pipe),Some(&mode),None,None) }
        .map_err(|_| "Cannot set bounded local control I/O")?;
    let stop = AtomicBool::new(false);
    let deadline = Instant::now()+CLIENT_TIMEOUT;
    write_frame(raw(&pipe),&payload,MAX_REQUEST_BYTES,&stop,deadline)?;
    let reply = read_frame(raw(&pipe),MAX_RESPONSE_BYTES,&stop,deadline)
        .map_err(|_| "No complete response; the action may have started. Inspect before retrying")?;
    let value: Value = serde_json::from_slice(&reply).map_err(|_| "Invalid local control reply")?;
    let _ = write_all(raw(&pipe),&[1],&stop,Instant::now()+Duration::from_secs(1));
    Ok(sanitize(value,&endpoint.token))
}

fn parse_cli(args: &[String]) -> Result<Value,String> {
    let strings: Vec<&str> = args.iter().map(String::as_str).collect();
    let request = match strings.as_slice() {
        ["--request", encoded] => {
            if encoded.len() > MAX_REQUEST_BYTES { return Err("Control request exceeds the size limit".into()); }
            serde_json::from_str(encoded).map_err(|_| "Invalid control request JSON")?
        },
        ["status"] => json!({"command":"status"}),
        ["show"] => json!({"command":"show"}),
        ["snapshot"] => json!({"command":"snapshot"}),
        ["navigate",url] => json!({"command":"navigate","url":url}),
        ["screenshot",path] => json!({"command":"screenshot","path":path}),
        ["click",selector] => json!({"command":"click","selector":selector}),
        ["fill",selector,text] => json!({"command":"fill","selector":selector,"text":text}),
        ["upload-file",selector,path] => json!({"command":"upload-file","selector":selector,"path":path}),
        ["profile-open"] => json!({"command":"profile-open"}),
        ["profile-inspect"] => json!({"command":"profile-inspect"}),
        ["profile-readback"] => json!({"command":"profile-readback"}),
        ["profile-set",field,text] => json!({"command":"profile-set","field":field,"text":text}),
        ["profile-avatar",path] => json!({"command":"profile-avatar","path":path}),
        ["profile-crop"] => json!({"command":"profile-crop"}),
        ["profile-crop-inspect"] => json!({"command":"profile-crop-inspect"}),
        ["profile-save"] => json!({"command":"profile-save"}),
        ["profile-confirm-name"] => json!({"command":"profile-confirm-name"}),
        ["upload-inspect"] => json!({"command":"upload-inspect"}),
        ["upload-diagnostics"] => json!({"command":"upload-diagnostics"}),
        ["upload-prepare"] => json!({"command":"upload-prepare"}),
        ["upload-caption",text] => json!({"command":"upload-caption","text":text}),
        ["upload-caption-check",text] => json!({"command":"upload-caption-check","text":text}),
        ["upload-guidance",action] => json!({"command":"upload-guidance","action":action}),
        ["publish-video",text] => json!({"command":"publish-video","text":text}),
        ["publication-status"] => json!({"command":"publication-status"}),
        _ => return Err("Use status, show, snapshot, navigate URL, screenshot ABSOLUTE_PNG, click SELECTOR, fill SELECTOR TEXT, upload-file SELECTOR ABSOLUTE_FILE, profile-open, profile-inspect, profile-readback, profile-set FIELD TEXT, profile-avatar ABSOLUTE_IMAGE, profile-crop, profile-save, profile-confirm-name, or --request JSON".into()),
    };
    validate_request(&request)?;
    Ok(request)
}

/// Console entry point. Pass argv[1..]; prints exactly one JSON result.
pub fn run_cli(args: &[String]) -> i32 {
    let (result,exit) = match parse_cli(args) {
        Err(message) => (error(&message),2),
        Ok(request) => match send_request(request) {
            Ok(value) => { let exit = if value.get("ok").and_then(Value::as_bool) == Some(true) { 0 } else { 1 }; (value,exit) },
            Err(message) => (error(&message),1),
        },
    };
    println!("{}",result);
    exit
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> { values.iter().map(|s|s.to_string()).collect() }

    #[test]
    fn cli_parses_only_the_bounded_public_command_set() {
        assert_eq!(parse_cli(&args(&["status"])).unwrap(),json!({"command":"status"}));
        assert_eq!(parse_cli(&args(&["fill","textarea[data-e2e=caption]","A caption"])).unwrap()["text"],"A caption");
        assert!(parse_cli(&args(&["eval","document.cookie"])).is_err());
        assert!(parse_cli(&args(&["--request",r#"{"command":"status","script":"x"}"#])).is_err());
        assert!(parse_cli(&args(&["--request","not-json"])).is_err());
    }

    #[test]
    fn navigation_rejects_non_tiktok_executable_and_credential_urls() {
        for url in ["https://tiktok.com.evil.test/","http://www.tiktok.com/","file:///C:/x",
            "javascript:alert(1)","https://user:pass@www.tiktok.com/","https://www.tiktok.com:9443/"] {
            assert!(validate_request(&json!({"command":"navigate","url":url})).is_err(),"{url}");
        }
        assert!(validate_request(&json!({"command":"navigate","url":"https://www.tiktok.com/@your_handle"})).is_ok());
    }

    #[test]
    fn frame_and_field_limits_are_enforced_before_allocation_or_dispatch() {
        assert!(validate_frame_length(0,MAX_REQUEST_BYTES).is_err());
        assert!(validate_frame_length(MAX_REQUEST_BYTES+1,MAX_REQUEST_BYTES).is_err());
        assert!(validate_frame_length(MAX_REQUEST_BYTES,MAX_REQUEST_BYTES).is_ok());
        assert!(validate_request(&json!({"command":"click","selector":"x".repeat(2049)})).is_err());
        assert!(validate_request(&json!({"command":"fill","selector":"textarea","text":"x".repeat(8193)})).is_err());
        assert!(validate_request(&json!({"command":"click","selector":"x\u{0}y"})).is_err());
    }

    #[test]
    fn profile_commands_reject_credentials_overlong_text_and_non_images() {
        assert!(parse_cli(&args(&["profile-open"])).is_ok());
        assert!(parse_cli(&args(&["profile-readback"])).is_ok());
        assert!(parse_cli(&args(&["profile-confirm-name"])).is_ok());
        assert!(parse_cli(&args(&["profile-set","displayname","Movie Demo"])).is_ok());
        assert!(parse_cli(&args(&["profile-set","password","anything"])).is_err());
        assert!(parse_cli(&args(&["profile-set","bio",&"Я".repeat(81)])).is_err());
        assert!(validate_request(&json!({"command":"profile-save","selector":"button"})).is_err());
        assert!(parse_cli(&args(&["profile-avatar",r"D:\assets\avatar.png"])).is_ok());
        assert!(parse_cli(&args(&["profile-avatar",r"D:\assets\movie.mp4"])).is_err());
    }

    #[test]
    fn publication_commands_are_typed_and_require_an_expected_caption() {
        assert!(parse_cli(&args(&["upload-inspect"])).is_ok());
        assert!(parse_cli(&args(&["publication-status"])).is_ok());
        assert!(parse_cli(&args(&["publish-video","A film scene"])).is_ok());
        assert!(parse_cli(&args(&["publish-video"])).is_err());
        assert!(parse_cli(&args(&["publish-video",""])).is_err());
        assert!(parse_cli(&args(&["upload-guidance","click-any-button"])).is_err());
        assert!(parse_cli(&args(&["upload-guidance","cancel-auto-check"])).is_ok());
        assert!(validate_request(&json!({"command":"publish-video","text":"A caption","selector":"button"})).is_err());
        assert!(validate_request(&json!({"command":"upload-inspect","script":"anything"})).is_err());
    }

    #[test]
    fn file_commands_require_local_absolute_paths_and_png_screenshots() {
        assert!(validate_request(&json!({"command":"screenshot","path":r"D:\review\frame.png"})).is_ok());
        assert!(validate_request(&json!({"command":"screenshot","path":"frame.png"})).is_err());
        assert!(validate_request(&json!({"command":"screenshot","path":r"D:\review\frame.exe"})).is_err());
        assert!(validate_request(&json!({"command":"upload-file","selector":"input[type=file]","path":r"\\server\share\x.mp4"})).is_err());
    }

    #[test]
    fn replies_redact_sensitive_fields_recursively_and_the_transport_token() {
        let token = "a".repeat(64);
        let value = sanitize(json!({"ok":true,"data":{"url":"https://www.tiktok.com/",
            "access_token":"sensitive","items":[{"cookies":"sensitive"}],
            "message":format!("internal {token}")}}),&token);
        assert_eq!(value["data"]["access_token"],"[REDACTED]");
        assert_eq!(value["data"]["items"][0]["cookies"],"[REDACTED]");
        assert_eq!(value["data"]["message"],"internal [REDACTED]");
        assert_eq!(value["data"]["url"],"https://www.tiktok.com/");
        assert!(!value.to_string().contains("sensitive"));
    }

    #[test]
    fn authentication_requires_exact_full_length_token() {
        assert!(token_matches(&"a".repeat(64),&"a".repeat(64)));
        assert!(!token_matches(&"a".repeat(64),&format!("{}b","a".repeat(63))));
        assert!(!token_matches("", ""));
        assert!(!token_matches(&"a".repeat(63),&"a".repeat(63)));
    }

    #[test]
    fn native_mock_transport_handles_two_requests_stops_and_detects_stale_endpoint() {
        // No GUI, WebView, TikTok navigation, file upload, or account request.
        // FILE_FLAG_FIRST_PIPE_INSTANCE makes this fail safely if another
        // opt-in control server is already running for this Windows user.
        use std::sync::atomic::AtomicUsize;
        let calls = Arc::new(AtomicUsize::new(0));
        let host_calls = calls.clone();
        let server = start_server(Arc::new(move |request| {
            let call = host_calls.fetch_add(1,Ordering::AcqRel)+1;
            json!({"ok":true,"data":{"command":request["command"],"call":call}})
        })).expect("isolated local mock control server");
        let endpoint_path = server.endpoint_path().to_path_buf();
        let endpoint = read_endpoint(&endpoint_path).expect("private endpoint readable by its owner");
        for expected in 1..=2 {
            let reply = send_request(json!({"command":"status"})).expect("framed native mock round trip");
            assert_eq!(reply["ok"],true);
            assert_eq!(reply["data"]["call"],expected);
        }
        assert_eq!(calls.load(Ordering::Acquire),2);
        let started = Instant::now();
        drop(server);
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!endpoint_path.exists(),"normal stop removes only its own descriptor");
        write_private_endpoint(&endpoint_path,&endpoint,&current_user_sid().unwrap()).unwrap();
        let stale = send_request(json!({"command":"status"}));
        let _ = fs::remove_file(&endpoint_path);
        assert!(stale.is_err(),"stale descriptor cannot dispatch a new action");
        assert_eq!(calls.load(Ordering::Acquire),2);
    }
}
