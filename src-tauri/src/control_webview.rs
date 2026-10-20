//! Bounded local control of this application's main WebView. No public CDP port,
//! raw script endpoint, cookie API, or automatic publication is exposed.

use base64::Engine;
use serde_json::{json, Value};
use std::{fs::OpenOptions, io::{Read, Write}, path::{Component, PathBuf, Prefix}, sync::mpsc, time::{Duration,Instant}};
use tauri::Manager;
use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
use windows::core::HSTRING;

const JS_GUARD: &str = r#"
const fixture = document.documentElement.dataset.controlFixture === 'true';
const visible = e => {const r=e.getBoundingClientRect();const s=getComputedStyle(e);return r.width>0 && r.height>0 && r.bottom>0 && r.right>0 && r.top<innerHeight && r.left<innerWidth && Number(s.opacity)!==0 && s.display!=='none' && s.visibility!=='hidden' && (!e.checkVisibility || e.checkVisibility({checkOpacity:true,checkVisibilityCSS:true}));};
const allowedHost = ['www.tiktok.com','tiktok.com','studio.tiktok.com'].includes(location.hostname);
if (!(allowedHost || (__ALLOW_FIXTURE__ && fixture))) throw new Error('Unsupported page');
if (/\/(login|signup|passport|auth|captcha|messages|inbox|settings)(\/|$)/i.test(decodeURIComponent(location.pathname))) throw new Error('Manual or private page');
if ([...document.querySelectorAll('input[type=password],input[type=tel],input[type=email],input[autocomplete=one-time-code],input[autocomplete=username],input[autocomplete=current-password],input[name*=otp i]')].some(visible)) throw new Error('Manual authentication required');
if ([...document.querySelectorAll('[role=dialog],[data-e2e*=login],[data-e2e*=captcha]')].some(e=>visible(e)&&/log.?in|sign.?in|войти|вход|QR|captcha|код подтверждения/i.test(e.innerText))) throw new Error('Manual authentication dialog');
const unique = selector => {const a=document.querySelectorAll(selector);if(a.length!==1) throw new Error('Selector must match exactly one element');return a[0];};
"#;

fn allowed_url(url: &tauri::Url) -> bool {
    url.scheme() == "https" && url.port_or_known_default() == Some(443) && url.username().is_empty() && url.password().is_none()
        && matches!(url.host_str(), Some("www.tiktok.com" | "tiktok.com" | "studio.tiktok.com"))
        && !url.path().split('/').any(|p| matches!(p.to_ascii_lowercase().as_str(),
            "login" | "signup" | "passport" | "auth" | "captcha" | "messages" | "inbox" | "settings"))
}

fn fixture_url(url: &tauri::Url) -> bool {
    (url.scheme()=="tauri" || url.host_str()==Some("tauri.localhost")) && url.path().ends_with("control-fixture.html")
}

#[derive(Clone,Copy)]
struct Limits { fixture: bool, deadline: Instant }

fn disk_path(raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if !matches!(path.components().next(), Some(Component::Prefix(p))
        if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))) || !path.is_absolute() {
        return Err("An absolute local-drive path is required; network paths are not supported".into());
    }
    Ok(path)
}

fn string<'a>(request: &'a Value, key: &str) -> Result<&'a str, String> {
    request.get(key).and_then(Value::as_str).filter(|v| !v.is_empty())
        .ok_or_else(|| format!("Missing string field: {key}"))
}

fn cdp(window: &tauri::WebviewWindow, method: &str, params: Value, limits: Limits) -> Result<Value, String> {
    if Instant::now() >= limits.deadline { return Err("Command deadline expired before dispatch".into()); }
    let expected=window.url().map_err(|_| "Cannot determine current page")?;
    if !allowed_url(&expected) && !(limits.fixture && fixture_url(&expected)) { return Err("Current page is outside local control scope".into()); }
    let (tx, rx) = mpsc::sync_channel(1);
    let method = method.to_owned();
    let parameters = params.to_string();
    window.with_webview(move |view| {
        if Instant::now() >= limits.deadline { let _=tx.try_send(Err("Command expired before UI dispatch".into())); return; }
        let tx_callback = tx.clone();
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |hr, response| {
            let result = hr.map_err(|_| "WebView rejected control command".to_string())
                .and_then(|_| serde_json::from_str::<Value>(&response).map_err(|_| "Invalid WebView response".into()));
            let _ = tx_callback.try_send(result);
            Ok(())
        }));
        let result = unsafe {
            view.controller().CoreWebView2().and_then(|core| {
                let mut raw=windows::core::PWSTR::null();
                core.Source(&mut raw)?;
                let source=webview2_com::take_pwstr(raw);
                if source != expected.as_str() { let _=tx.try_send(Err("Page changed before dispatch; inspect current state".into())); return Ok(()); }
                core.CallDevToolsProtocolMethod(&HSTRING::from(&method), &HSTRING::from(&parameters), &handler)
            })
        };
        if result.is_err() { let _ = tx.try_send(Err("Cannot dispatch WebView control command".into())); }
    }).map_err(|_| "Cannot reach main WebView".to_string())?;
    let value = rx.recv_timeout(limits.deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| "WebView command timed out; inspect current state before retrying".to_string())??;
    if value.get("error").is_some() { return Err("WebView protocol rejected command".into()); }
    Ok(value)
}

fn evaluate(window: &tauri::WebviewWindow, body: &str, limits: Limits, by_value: bool) -> Result<Value, String> {
    let guard = JS_GUARD.replace("__ALLOW_FIXTURE__", if limits.fixture { "true" } else { "false" });
    let script = format!("(()=>{{{guard}\n{body}\n}})()");
    expression(window,&script,limits,by_value)
}

fn expression(window: &tauri::WebviewWindow, script: &str, limits: Limits, by_value: bool) -> Result<Value,String> {
    let value = cdp(window, "Runtime.evaluate", json!({"expression":script,"returnByValue":by_value,"awaitPromise":true,"timeout":8000}),limits)?;
    if value.get("exceptionDetails").is_some() {
        let description=value.pointer("/exceptionDetails/exception/description").and_then(Value::as_str).unwrap_or("");
        let first=description.lines().next().unwrap_or("").strip_prefix("Error: ").unwrap_or("");
        if ["Unsupported page","Manual or private page","Manual authentication required","Manual authentication dialog","Selector must match exactly one element","Element is not visible","Only safe navigation links are supported","Not a safe navigation link","Not a file input"].contains(&first) {
            return Err(first.to_owned());
        }
        if let Some(safe) = crate::profile_scripts::public_error(first) { return Err(safe.to_owned()); }
        return Err("Page rejected operation; check page, selector, and manual-login state".into());
    }
    if by_value { Ok(value.pointer("/result/value").cloned().unwrap_or(Value::Null)) }
    else { Ok(value.get("result").cloned().unwrap_or(Value::Null)) }
}

fn execute(app: &tauri::AppHandle, request: &Value, fixture: bool) -> Result<Value, String> {
    let window = app.get_webview_window("main").ok_or("Main window is unavailable")?;
    let command = string(request, "command")?;
    let limits=Limits{fixture,deadline:Instant::now()+Duration::from_secs(15)};
    if command=="show" {
        window.show().map_err(|_| "Cannot show main window")?;
        window.set_focus().map_err(|_| "Cannot focus main window")?;
        return Ok(json!({"window_shown":true,"fixture":fixture}));
    }
    let url = window.url().map_err(|_| "Cannot determine current page")?;
    let on_fixture = fixture && fixture_url(&url);
    if command == "navigate" {
        let next = tauri::Url::parse(string(request,"url")?).map_err(|_| "Invalid URL")?;
        if !allowed_url(&next) { return Err("Navigation is limited to ordinary HTTPS TikTok pages".into()); }
        if fixture { return Err("Fixture mode cannot navigate to the live account".into()); }
        window.navigate(next).map_err(|_| "Navigation request failed")?;
        return Ok(json!({"navigation_requested":true,"verify_with":"status"}));
    }
    if !on_fixture && !allowed_url(&url) {
        return Err("Control is unavailable on authentication, private, or non-TikTok pages".into());
    }
    match command {
        "status" => expression(&window, r#"(()=>{
const visible=e=>{const r=e.getBoundingClientRect();const s=getComputedStyle(e);return r.width>0&&r.height>0&&r.bottom>0&&r.right>0&&r.top<innerHeight&&r.left<innerWidth&&Number(s.opacity)!==0&&s.display!=='none'&&s.visibility!=='hidden'&&(!e.checkVisibility||e.checkVisibility({checkOpacity:true,checkVisibilityCSS:true}));};
const credentialFields=[...document.querySelectorAll('input[type=password],input[type=tel],input[type=email],input[autocomplete=one-time-code],input[autocomplete=username],input[autocomplete=current-password],input[name*=otp i]')].filter(visible);
return {url:location.origin+location.pathname,title:document.title.slice(0,200),readyState:document.readyState,
  ownProfile:[...document.querySelectorAll('button,a')].some(e=>visible(e)&&/^(Изменить профиль|Edit profile)$/i.test((e.innerText||e.textContent||'').trim())),
  loginPrompt:[...document.querySelectorAll('button,a')].some(e=>visible(e)&&/^(Войти|Log in)$/i.test((e.innerText||e.textContent||'').trim())),
  manualEntryFieldCount:credentialFields.length,fixture:document.documentElement.dataset.controlFixture==='true'};
})()
"#, limits, true),
        "snapshot" => evaluate(&window, r#"
document.querySelectorAll('[data-tn-control-id]').forEach(e=>e.removeAttribute('data-tn-control-id'));
let n=0; const elements=[];
for(const e of document.querySelectorAll('a,button,input:not([type=password]):not([type=hidden]),textarea,[contenteditable=true],[role=button],h1,h2,h3,[data-e2e]')){
  if(!visible(e)||elements.length>=250) continue;
  const id='tnc_'+(++n);e.setAttribute('data-tn-control-id',id);
  const r=e.getBoundingClientRect();
  elements.push({selector:'[data-tn-control-id="'+id+'"]',tag:e.tagName.toLowerCase(),type:e.getAttribute('type'),
    text:((e.matches('input,textarea,[contenteditable=true]') || e.querySelector('input,textarea,[contenteditable=true]')) ? (e.getAttribute('aria-label')||e.getAttribute('placeholder')||'') : (e.innerText||e.getAttribute('aria-label')||'')).trim().slice(0,180),
    dataE2e:e.getAttribute('data-e2e'),rect:{x:r.x,y:r.y,width:r.width,height:r.height}});
}
return {url:location.origin+location.pathname,elements};
"#, limits, true),
        "click" => {
            let selector = serde_json::to_string(string(request,"selector")?).map_err(|_| "Invalid selector")?;
            evaluate(&window, &format!(r#"
const e=unique({selector});if(!visible(e)) throw new Error('Element is not visible');
if(fixture){{if(!['fixture-button','fixture-auth'].includes(e.id)) throw new Error('Only fixture test buttons are allowed');e.click();return {{clicked:true}};}}
const a=e.closest('a[href]');if(!a) throw new Error('Only safe navigation links are supported');
const u=new URL(a.href,location.href);
if(u.protocol!=='https:' || !['www.tiktok.com','tiktok.com','studio.tiktok.com'].includes(u.hostname) || !(u.pathname==='/' || /^\/(@|search|foryou|following|explore|tiktokstudio\/upload|creator-center\/upload)/.test(u.pathname))) throw new Error('Not a safe navigation link');
location.assign(u.href);return {{navigation_requested:true}};
"#), limits, true)
        }
        "fill" => {
            let selector = serde_json::to_string(string(request,"selector")?).map_err(|_| "Invalid selector")?;
            let text = request.get("text").and_then(Value::as_str).ok_or("Missing text")?;
            if text.len()>16000 { return Err("Text is too long".into()); }
            let text = serde_json::to_string(text).map_err(|_| "Invalid text")?;
            evaluate(&window,&format!(r#"
const e=unique({selector});if(!visible(e)) throw new Error('Element is not visible');
if(e.matches('input[type=password],input[type=file],input[type=hidden],input[type=email]')) throw new Error('Sensitive or file field refused');
if(e.isContentEditable){{e.focus();e.textContent={text};e.dispatchEvent(new InputEvent('input',{{bubbles:true,inputType:'insertText'}}));}}
else if(e instanceof HTMLInputElement || e instanceof HTMLTextAreaElement){{
 const proto=e instanceof HTMLTextAreaElement?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;
 Object.getOwnPropertyDescriptor(proto,'value').set.call(e,{text});e.dispatchEvent(new Event('input',{{bubbles:true}}));e.dispatchEvent(new Event('change',{{bubbles:true}}));
}}else throw new Error('Element is not editable');return {{filled:true}};
"#),limits,true)
        }
        "screenshot" => {
            let path=disk_path(string(request,"path")?)?;
            if path.extension().and_then(|x|x.to_str()).map(|x|x.eq_ignore_ascii_case("png"))!=Some(true) { return Err("Screenshot output must be a PNG path".into()); }
            evaluate(&window,"return {allowed:true};",limits,true)?;
            let shot=cdp(&window,"Page.captureScreenshot",json!({"format":"png","fromSurface":true,"captureBeyondViewport":false}),limits)?;
            let data=shot.get("data").and_then(Value::as_str).ok_or("No PNG returned by WebView")?;
            if data.len()>32*1024*1024 { return Err("Screenshot exceeds size limit".into()); }
            let bytes=base64::engine::general_purpose::STANDARD.decode(data).map_err(|_| "Invalid PNG payload")?;
            if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return Err("WebView did not return a PNG".into()); }
            let mut output=OpenOptions::new().write(true).create_new(true).open(&path)
                .map_err(|_| "Cannot create screenshot: parent must exist and output must not already exist")?;
            output.write_all(&bytes).map_err(|_| "Cannot save screenshot")?;
            Ok(json!({"path":path,"bytes":bytes.len(),"scope":"main WebView viewport"}))
        }
        "profile-open" => evaluate(&window, &crate::profile_scripts::open(), limits, true),
        "profile-inspect" => evaluate(&window, &crate::profile_scripts::inspect(), limits, true),
        "profile-readback" => evaluate(&window, &crate::profile_scripts::readback(), limits, true),
        "profile-set" => {
            let body=crate::profile_scripts::field_fill(string(request,"field")?, request.get("text").and_then(Value::as_str).ok_or("Missing text")?)?;
            evaluate(&window, &body, limits, true)
        }
        "profile-save" => evaluate(&window, &crate::profile_scripts::save(), limits, true),
        "profile-confirm-name" => evaluate(&window, &crate::profile_scripts::confirm_name(), limits, true),
        "profile-crop" => evaluate(&window, &crate::profile_scripts::crop_apply(), limits, true),
        "profile-crop-inspect" => evaluate(&window, &crate::profile_scripts::crop_inspect(), limits, true),
        "profile-avatar" => {
            let path=disk_path(string(request,"path")?)?.canonicalize().map_err(|_| "Avatar file does not exist")?;
            let ext=path.extension().and_then(|v|v.to_str()).unwrap_or("").to_ascii_lowercase();
            let size=path.metadata().map_err(|_| "Cannot inspect avatar file")?.len();
            if !path.is_file() || !matches!(ext.as_str(),"png"|"jpg"|"jpeg") || size==0 || size>10*1024*1024 {
                return Err("Choose a local PNG or JPEG avatar up to 10 MiB".into());
            }
            let mut header=[0u8;12];
            let count=OpenOptions::new().read(true).open(&path).map_err(|_| "Cannot read avatar")?
                .read(&mut header).map_err(|_| "Cannot read avatar signature")?;
            let signature_ok=if ext=="png" {count>=8 && header.starts_with(b"\x89PNG\r\n\x1a\n")} else {count>=3 && header.starts_with(&[0xff,0xd8,0xff])};
            if !signature_ok {return Err("Avatar contents do not match its image extension".into());}
            let remote=evaluate(&window,&crate::profile_scripts::avatar_input(),limits,false)?;
            let id=remote.get("objectId").and_then(Value::as_str).ok_or("Profile avatar input is unavailable in the current frame")?;
            cdp(&window,"DOM.setFileInputFiles",json!({"objectId":id,"files":[path]}),limits)?;
            let result=cdp(&window,"Runtime.callFunctionOn",json!({"objectId":id,"functionDeclaration":"function(){return {selected:this.files.length===1,name:this.files[0]?.name,size:this.files[0]?.size};}","returnByValue":true}),limits);
            let _=cdp(&window,"Runtime.releaseObject",json!({"objectId":id}),limits);
            let verified=result?.pointer("/result/value").cloned().unwrap_or(Value::Null);
            let expected_name=path.file_name().and_then(|v|v.to_str()).ok_or("Invalid avatar filename")?;
            if verified.get("selected").and_then(Value::as_bool)!=Some(true)
                || verified.get("name").and_then(Value::as_str)!=Some(expected_name)
                || verified.get("size").and_then(Value::as_u64)!=Some(size) {
                return Err("Avatar selection could not be verified; inspect before retrying".into());
            }
            Ok(json!({"selection":verified,"profile_saved":false,"verify_with":"profile-inspect"}))
        }
        "upload-file" => {
            if !on_fixture && !url.path().contains("upload") { return Err("Open the TikTok upload page first".into()); }
            let path=disk_path(string(request,"path")?)?.canonicalize().map_err(|_| "Media file does not exist")?;
            let ext=path.extension().and_then(|v|v.to_str()).unwrap_or("").to_ascii_lowercase();
            if !path.is_file() || !matches!(ext.as_str(),"mp4"|"mov"|"m4v"|"webm") { return Err("Choose an existing local MP4/MOV/M4V/WebM file".into()); }
            let selector=serde_json::to_string(string(request,"selector")?).map_err(|_| "Invalid selector")?;
            let remote=evaluate(&window,&format!("const e=unique({selector});if(!e.matches('input[type=file]')) throw new Error('Not a file input');return e;"),limits,false)?;
            let id=remote.get("objectId").and_then(Value::as_str).ok_or("File input is unavailable in the current frame")?;
            cdp(&window,"DOM.setFileInputFiles",json!({"objectId":id,"files":[path]}),limits)?;
            let verified=evaluate(&window,&format!("const e=unique({selector});return {{selected:e.files.length===1,name:e.files[0]?.name,size:e.files[0]?.size}};"),limits,true)?;
            let expected_name=path.file_name().and_then(|v|v.to_str()).ok_or("Invalid media filename")?;
            let expected_size=path.metadata().map_err(|_| "Cannot verify selected file")?.len();
            if verified.get("selected").and_then(Value::as_bool)!=Some(true)
                || verified.get("name").and_then(Value::as_str)!=Some(expected_name)
                || verified.get("size").and_then(Value::as_u64)!=Some(expected_size) {
                return Err("File selection could not be verified; transfer may have started. Inspect before retrying".into());
            }
            Ok(json!({"selection":verified,"published":false,"limit":"File selection may start TikTok transfer; processing and publication are separate"}))
        }
        _ => Err("Unsupported command".into()),
    }
}

pub fn handle_request(app: &tauri::AppHandle, request: Value, fixture: bool) -> Value {
    match execute(app,&request,fixture) {
        Ok(data)=>json!({"ok":true,"data":data}),
        Err(error)=>json!({"ok":false,"error":error}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn navigation_is_tiktok_only_and_excludes_manual_pages(){
        for u in ["https://www.tiktok.com/@test","https://www.tiktok.com/tiktokstudio/upload","https://studio.tiktok.com/upload"] {assert!(allowed_url(&tauri::Url::parse(u).unwrap()));}
        for u in ["http://www.tiktok.com/","https://www.tiktok.com.evil.test/","https://user:secret@www.tiktok.com/","https://www.tiktok.com/login","https://www.tiktok.com/messages","file:///C:/private"] {assert!(!allowed_url(&tauri::Url::parse(u).unwrap()));}
    }
    #[test] fn file_paths_cannot_silently_be_network_paths(){
        assert!(disk_path(r"D:\exports\clip.mp4").is_ok());
        assert!(disk_path(r"\\server\share\clip.mp4").is_err());
        assert!(disk_path(r"relative\clip.mp4").is_err());
    }
}
