//! Fixed DOM bodies for explicitly authorized TikTok video publication.
//!
//! These bodies run only through control_webview::evaluate, whose host and
//! authentication guard defines `fixture` and `visible`. There is no arbitrary
//! selector or script parameter. A click receipt is not proof of publication.
//!
//! Caption recognition is deliberately conservative: a visible labelled public
//! caption editor or a unique Draft.js editor on an upload page. These semantic
//! fallbacks are not a claim that a particular live TikTok layout was observed.
//! Unknown layouts, unfinished uploads, and dialogs fail closed.
//!
//! Observed 2026-10-20: post_video_button, mobile_preview_container, and the
//! Russian automatic-content-check prompt. Caption markup remains semantic
//! until the post-upload diagnostics can establish the exact live editor.
//! The observed mobile preview uses a displayed 1080x1440 canvas rather than
//! HTMLVideoElement; its readiness is reported as canvas UI evidence only.

/// A conservative local command limit, not a promise about TikTok's live limit.
pub const CAPTION_MAX_UTF16: usize = 2200;

pub fn validate_caption(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("Upload caption must not be empty".into());
    }
    if value.encode_utf16().count() > CAPTION_MAX_UTF16 {
        return Err(format!("Upload caption exceeds {CAPTION_MAX_UTF16} UTF-16 units"));
    }
    if value.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err("Upload caption contains unsupported control characters".into());
    }
    Ok(())
}

/// Never expose arbitrary DOM exception text through the local control API.
pub fn public_error(message: &str) -> Option<&'static str> {
    const MESSAGES: &[&str] = &[
        "Upload operation requires an ordinary TikTok upload page",
        "Upload caption editor must be unique",
        "Upload caption editor is not editable",
        "Upload caption exceeds live field maxlength",
        "Upload caption native insertion failed; inspect before retrying",
        "Upload caption readback did not match; inspect before retrying",
        "Upload caption focus could not be verified",
        "Upload caption selection could not be prepared",
        "Upload publication button must be unique",
        "Upload publication button is disabled",
        "Upload publication requires a nonempty caption",
        "Upload publication requires a ready video preview",
        "Upload current caption does not match the authorized publication",
        "Upload or processing is still in progress",
        "Upload page reports an error; inspect before retrying",
        "Upload dialog requires inspection before publication",
        "Upload publication was already requested; inspect before retrying",
        "Upload guidance action is unsupported",
        "Upload automatic-check prompt must be unique",
        "Upload automatic-check cancel button must be unique",
        "Upload tutorial panel must be unique",
        "Upload tutorial acknowledgement must be unique",
        "Upload guidance action is disabled",
        "Publication inspection requires an ordinary TikTok upload, content, or profile page",
    ];
    MESSAGES.iter().copied().find(|allowed| *allowed == message)
}

// User text becomes exactly one JS string literal. It never becomes a selector,
// property name, template literal, or executable fragment.
fn js_string(value: &str) -> String {
    let mut encoded = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            '<' => encoded.push_str("\\u003c"),
            '>' => encoded.push_str("\\u003e"),
            '&' => encoded.push_str("\\u0026"),
            '\u{2028}' => encoded.push_str("\\u2028"),
            '\u{2029}' => encoded.push_str("\\u2029"),
            c if (c as u32) < 0x20 => encoded.push_str(&format!("\\u{:04x}", c as u32)),
            c => encoded.push(c),
        }
    }
    encoded.push('"');
    encoded
}

const HELPERS: &str = r#"
function uScope(){
 const hosts=['www.tiktok.com','tiktok.com','studio.tiktok.com'];
 const path=decodeURIComponent(location.pathname).replace(/\/$/,'');
 const route=/^\/(?:tiktokstudio\/upload|creator-center\/upload|upload)$/.test(path);
 const localFixture=fixture&&(location.protocol==='tauri:'||location.hostname==='tauri.localhost');
 if(!localFixture&&!(location.protocol==='https:'&&hosts.includes(location.hostname)&&(!location.port||location.port==='443')&&route))throw new Error('Upload operation requires an ordinary TikTok upload page');
}
uScope();
const uText=e=>(e.innerText||e.textContent||'').trim().replace(/\s+/g,' ');
// Editors and the mobile video can be above the scrolled viewport. Distinguish
// displayed layout from viewport intersection; mutations scroll their target.
const uDisplayed=e=>{const r=e.getBoundingClientRect(),s=getComputedStyle(e);return r.width>0&&r.height>0&&s.display!=='none'&&s.visibility!=='hidden'&&Number(s.opacity)!==0&&(!e.checkVisibility||e.checkVisibility({checkOpacity:true,checkVisibilityCSS:true}));};
const uPending=()=>document.documentElement.dataset.tnPublishRequested==='true';
const uEnabled=e=>!e.disabled&&e.getAttribute('aria-disabled')!=='true'&&!e.closest('[inert]');
const uPublishLabel=s=>/^(Опубликовать|Post|Publish)$/i.test(s);
const uPublishButtons=()=>[...document.querySelectorAll('button')].filter(e=>uDisplayed(e)&&uPublishLabel(uText(e))&&(e.getAttribute('data-e2e')==='post_video_button'||(fixture&&(location.protocol==='tauri:'||location.hostname==='tauri.localhost'))));
const uCaptionLabel=s=>/^(Описание|Подпись|Описание видео|Добавьте описание|Description|Caption|Video description|Add a caption|Add description)$/i.test((s||'').trim().replace(/[:：]$/,''));
function uCaptionNames(e){
 const names=[e.getAttribute('aria-label'),e.getAttribute('placeholder'),e.getAttribute('data-placeholder')];
 if(e.labels)for(const label of e.labels)names.push(uText(label));
 const ids=e.getAttribute('aria-labelledby');
 if(ids)for(const id of ids.split(/\s+/)){const label=document.getElementById(id);if(label&&uDisplayed(label))names.push(uText(label));}
 // Only a row with this one editor may supply an unassociated caption label.
 let row=e.parentElement;
 for(let i=0;row&&row!==document.body&&i<4;i++,row=row.parentElement){
  if(row.querySelectorAll('textarea,[contenteditable=true],input[type=text],input:not([type])').length!==1)break;
  const labels=[...row.querySelectorAll('label,h1,h2,h3,h4,span,p,div')].filter(x=>uDisplayed(x)&&x.children.length===0&&uCaptionLabel(uText(x)));
  for(const label of labels)names.push(uText(label));
 }
 return names.filter(x=>typeof x==='string');
}
function uCaptionCandidates(){
 const editable=[...document.querySelectorAll('textarea,[contenteditable=true],input[type=text],input:not([type])')].filter(e=>uDisplayed(e)&&!e.closest('[role=dialog],[aria-modal=true],dialog[open]'));
 const labelled=editable.filter(e=>uCaptionNames(e).some(uCaptionLabel));
 if(labelled.length)return [...new Set(labelled)];
 // Draft.js is recognizable without guessing a current generated CSS class.
 // The fallback is permitted only for an unambiguous upload form.
 const draft=editable.filter(e=>e.isContentEditable&&e.classList.contains('public-DraftEditor-content'));
 return draft.length===1&&uPublishButtons().length===1?draft:[];
}
const uRead=e=>{
 if(e instanceof HTMLInputElement||e instanceof HTMLTextAreaElement)return e.value.replace(/\r\n?/g,'\n');
 if(e.classList.contains('public-DraftEditor-content')){
  // Draft's ContentBlocks are logical lines. innerText can add layout-derived
  // empty lines around block divs; do not trim/collapse meaningful whitespace.
  const blocks=[...e.querySelectorAll('[data-block=true]')].filter(b=>{const ancestor=b.parentElement?.closest('[data-block=true]');return !ancestor||!e.contains(ancestor);});
  if(blocks.length)return blocks.map(b=>b.textContent||'').join('\n').replace(/\r\n?/g,'\n');
 }
 return (e.innerText||e.textContent||'').replace(/\r\n?/g,'\n');
};
function uCaption(){const a=uCaptionCandidates();if(a.length!==1)throw new Error('Upload caption editor must be unique');return a[0];}
const uDialogs=()=>[...document.querySelectorAll('[role=dialog],[aria-modal=true],dialog[open]')].filter(visible);
const uAutoTitle=s=>s==='Включить автоматическую проверку контента?';
const uAckLabel=s=>s==='Понятно';
const uSmallest=items=>{const a=[...new Set(items)];return a.filter(e=>!a.some(other=>other!==e&&e.contains(other)));};
function uAutoPanels(){
 const headings=[...document.querySelectorAll('h1,h2,h3,h4,h5,h6,div,span,p,strong')].filter(e=>visible(e)&&uAutoTitle(uText(e)));
 const panels=[];
 for(const heading of headings){
  let e=heading.parentElement;
  for(let i=0;e&&e!==document.body&&i<8;i++,e=e.parentElement){
   const labels=[...e.querySelectorAll('button')].filter(visible).map(uText);
   const text=uText(e);
   if(labels.filter(s=>s==='Отмена').length===1&&labels.filter(s=>s==='Включить').length===1&&e.querySelectorAll('input[type=checkbox],[role=checkbox]').length===1&&/авторских прав/.test(text)&&text.length<1800){panels.push(e);break;}
  }
 }
 return uSmallest(panels);
}
function uTutorialPanels(){
 const panels=[];
 for(const button of [...document.querySelectorAll('button')].filter(e=>visible(e)&&uAckLabel(uText(e)))){
  let e=button.parentElement;
  for(let i=0;e&&e!==document.body&&i<8;i++,e=e.parentElement){
   const labels=[...e.querySelectorAll('button')].filter(visible).map(uText).filter(Boolean),text=uText(e);
   if(labels.length===1&&uAckLabel(labels[0])&&text.length>22&&text.length<1200&&/публикаци|редактир|настрой|видео|video|post/i.test(text)&&!text.includes('Включить автоматическую проверку контента?')&&e.querySelectorAll('input,textarea,[contenteditable=true]').length===0&&e.querySelectorAll('img,svg,canvas,picture,video').length>0){panels.push(e);break;}
  }
 }
 return uSmallest(panels);
}
function uGuidance(){
 return {automaticCheckPrompt:uAutoPanels().length,tutorialPanels:uTutorialPanels().length,unacknowledgedGuidanceButtons:[...document.querySelectorAll('button')].filter(e=>visible(e)&&uAckLabel(uText(e))).length};
}
function uProgress(){
 return [...document.querySelectorAll('progress,[role=progressbar]')].filter(uDisplayed).map(e=>{
  const raw=e.getAttribute('aria-valuenow')??(e.tagName==='PROGRESS'?String(e.value):null);
  const maxRaw=e.getAttribute('aria-valuemax')??(e.tagName==='PROGRESS'?String(e.max):'100');
  const value=raw===null?null:Number(raw),max=Number(maxRaw);
  return {value:Number.isFinite(value)?value:null,max:Number.isFinite(max)?max:null,busy:raw===null||!Number.isFinite(value)||!Number.isFinite(max)||max<=0||value<max};
 });
}
const uBusyText=s=>/^(?:Обрабатывается(?: видео)?|Обработка видео|Ид[её]т загрузка|Загружается(?: видео)?|Видео обрабатывается|Processing(?: video)?|Video is processing|Checking(?: video)?|Проверяется(?: видео)?)(?:[.\s…:]|$)/i.test(s)||/^(?:Uploading|Загрузка)(?: видео)?\s*[:.…]*\s*(?:\d{1,2}\s*%|\.\.\.|…)/i.test(s);
const uErrorText=s=>/^(?:Что-то пошло не так|Что то пошло не так|Не удалось (?:загрузить|опубликовать)|Ошибка (?:загрузки|публикации)|Upload failed|Failed to (?:upload|publish|post)|Couldn't (?:upload|publish|post)|Something went wrong)(?:[.\s…!:]|$)/i.test(s);
const uSuccessText=s=>/^(?:Видео (?:успешно )?опубликовано[.!]?|Your video (?:has been|was) (?:successfully )?published[.!]?|Video (?:successfully )?published[.!]?|Post published[.!]?)$/i.test(s);
function uLabels(test){
 const labels=[...document.querySelectorAll('[role=status],[role=alert],p,span,div,h1,h2,h3')].filter(e=>uDisplayed(e)&&e.children.length<=1).map(uText).filter(s=>s.length>0&&s.length<=160&&test(s));
 return [...new Set(labels)].slice(0,12);
}
function uClockSeconds(label){
 if(!/^\d{2}:\d{2}:\d{2}$/.test(label))return null;
 const [hours,minutes,seconds]=label.split(':').map(Number);
 return minutes<60&&seconds<60?hours*3600+minutes*60+seconds:null;
}
function uCanvasPreviews(){
 const containers=[...document.querySelectorAll('[data-e2e="mobile_preview_container"]')].filter(uDisplayed),result=[];
 for(const container of containers.slice(0,4)){
  const canvases=[...container.querySelectorAll('canvas')].filter(uDisplayed);
  const clocks=[...uText(container).matchAll(/\b(\d{2}:\d{2}:\d{2})\s*\/\s*(\d{2}:\d{2}:\d{2})\b/g)];
  const current=clocks.length===1?uClockSeconds(clocks[0][1]):null,duration=clocks.length===1?uClockSeconds(clocks[0][2]):null;
  const labels=[...container.querySelectorAll('button,[role=button],[aria-label],[title],span,div')].filter(uDisplayed).flatMap(e=>[uText(e),e.getAttribute('aria-label')||'',e.getAttribute('title')||'']).map(s=>s.trim());
  const play=labels.some(s=>/^(Воспроизвести|Пауза|Play|Pause)$/i.test(s));
  const mute=labels.some(s=>/^(Заглушить|Включить звук|Выключить звук|Mute|Unmute)$/i.test(s));
  for(const canvas of canvases.slice(0,4)){
   const canvasPreviewReady=containers.length===1&&canvases.length===1&&canvas.width>0&&canvas.height>0&&clocks.length===1&&duration!==null&&duration>0&&current!==null&&current>=0&&current<=duration&&play&&mute;
   result.push({kind:'canvas',canvasWidth:canvas.width,canvasHeight:canvas.height,containerCount:containers.length,canvasCount:canvases.length,clockCount:clocks.length,currentTimeLabel:clocks.length===1?clocks[0][1]:null,durationLabel:clocks.length===1?clocks[0][2]:null,duration,current,controls:{play,mute},inViewport:visible(canvas),canvasPreviewReady});
  }
 }
 return result;
}
function uPreviews(){
 const videos=[...document.querySelectorAll('video')].filter(uDisplayed).map(e=>({kind:'video',readyState:e.readyState,duration:Number.isFinite(e.duration)?e.duration:null,width:e.videoWidth,height:e.videoHeight,paused:e.paused,muted:e.muted,inViewport:visible(e)}));
 return [...videos,...uCanvasPreviews()];
}
const uPreviewReady=p=>p.kind==='canvas'?p.canvasPreviewReady===true:p.kind==='video'&&p.readyState>=2&&p.duration>0&&p.width>0&&p.height>0;
function uUploadFileLabels(){
 const labels=[...document.querySelectorAll('h1,h2,h3,h4,p,span,div')].filter(e=>uDisplayed(e)&&e.children.length<=1).map(uText).filter(s=>s.length>0&&s.length<=255);
 return {statusLabels:[...new Set(labels.filter(s=>/^(?:Загружено|Uploaded)(?:\s*\([\d.,]+\s*(?:B|KB|MB|GB|байт|КБ|МБ|ГБ)\))?[.!]?$/i.test(s)))].slice(0,6),filenameLabels:[...new Set(labels.filter(s=>/^[^/\\<>\r\n]{1,240}\.(?:mp4|mov|m4v|webm)$/i.test(s)))].slice(0,6)};
}
function uState(){
 const captions=uCaptionCandidates(),buttons=uPublishButtons(),previews=uPreviews(),progress=uProgress();
 const busy=uLabels(uBusyText),errors=uLabels(uErrorText),success=uLabels(uSuccessText),dialogs=uDialogs(),guidance=uGuidance();
 const reasons=[];
 if(uPending())reasons.push('publication_already_requested');
 if(captions.length!==1)reasons.push('caption_not_unique');
 else if(!uRead(captions[0]).trim())reasons.push('caption_empty');
 if(buttons.length!==1)reasons.push('publish_button_not_unique');
 else if(!uEnabled(buttons[0]))reasons.push('publish_button_disabled');
 if(!previews.some(uPreviewReady))reasons.push('ready_video_preview_missing');
 if(progress.some(p=>p.busy)||busy.length)reasons.push('upload_or_processing_in_progress');
 if(errors.length)reasons.push('upload_error_observed');
 if(dialogs.length||guidance.automaticCheckPrompt||guidance.unacknowledgedGuidanceButtons)reasons.push('dialog_requires_inspection');
 return {captions,buttons,previews,progress,busy,errors,success,dialogs,guidance,reasons};
}
function uPublicationReady(){
 const s=uState();
 if(uPending())throw new Error('Upload publication was already requested; inspect before retrying');
 if(s.dialogs.length||s.guidance.automaticCheckPrompt||s.guidance.unacknowledgedGuidanceButtons)throw new Error('Upload dialog requires inspection before publication');
 if(s.captions.length!==1)throw new Error('Upload caption editor must be unique');
 if(!uRead(s.captions[0]).trim())throw new Error('Upload publication requires a nonempty caption');
 if(s.buttons.length!==1)throw new Error('Upload publication button must be unique');
 if(!uEnabled(s.buttons[0]))throw new Error('Upload publication button is disabled');
 if(s.progress.some(p=>p.busy)||s.busy.length)throw new Error('Upload or processing is still in progress');
 if(s.errors.length)throw new Error('Upload page reports an error; inspect before retrying');
 if(!s.previews.some(uPreviewReady))throw new Error('Upload publication requires a ready video preview');
 return s.buttons[0];
}
"#;

fn body(action: &str) -> String {
    format!("{HELPERS}\n{action}")
}

/// Bounded metadata only: no cookie, storage, video source URL, or caption dump.
/// A success label is reported as an observation, not as copyright clearance.
pub fn inspect() -> String {
    body(r#"
const s=uState();
return {
 scope:'TikTok upload page',publicationRequested:uPending(),readyToRequestPublication:s.reasons.length===0,readinessReasons:s.reasons,
 captionEditors:s.captions.map(e=>({tag:e.tagName.toLowerCase(),contentEditable:e.isContentEditable,lengthUtf16:uRead(e).length,disabled:!uEnabled(e)||e.readOnly===true,maxLength:typeof e.maxLength==='number'&&e.maxLength>=0?e.maxLength:null})),
 fileInputs:[...document.querySelectorAll('input[type=file]')].map(e=>({accept:e.accept,multiple:e.multiple,disabled:e.disabled,files:[...e.files].map(f=>({name:f.name,size:f.size,type:f.type}))})),
 previews:s.previews,canvasPreviewReady:s.previews.some(p=>p.kind==='canvas'&&uPreviewReady(p)),progress:s.progress,busyLabels:s.busy,errorLabels:s.errors,
 publicationButtons:s.buttons.map(e=>({label:uText(e),disabled:!uEnabled(e)})),
 guidance:s.guidance,
 dialogs:s.dialogs.map(d=>({headings:[...d.querySelectorAll('h1,h2,h3,[role=heading]')].filter(visible).map(uText).filter(x=>x.length<=160).slice(0,4),buttons:[...d.querySelectorAll('button')].filter(visible).map(e=>({label:uText(e).slice(0,80),disabled:!uEnabled(e)})).slice(0,8)})),
 controls:[...document.querySelectorAll('select,[role=combobox],input[type=checkbox],[role=checkbox]')].filter(visible).map(e=>({role:e.getAttribute('role'),tag:e.tagName.toLowerCase(),label:(e.getAttribute('aria-label')||e.getAttribute('name')||'').slice(0,100),checked:typeof e.checked==='boolean'?e.checked:e.getAttribute('aria-checked'),value:e.tagName==='SELECT'?e.options[e.selectedIndex]?.text.slice(0,100):null})).slice(0,20),
 explicitPublicationSuccessObserved:s.success.length>0,publicationSuccessLabels:s.success,
 limit:'A click, upload completion, preview, or absence of errors does not prove publication or rights clearance.'
};
"#)
}

/// Prepare a single recognized editor for native `Input.insertText` in the
/// host. This body never inserts text, mutates Draft.js DOM contents, touches
/// React's private state, or writes the system clipboard. The host must send
/// native input as a separate step, then wait and use caption_matches.
pub fn caption_prepare_native(value: &str) -> Result<String, String> {
    validate_caption(value)?;
    Ok(body(&format!(r#"
if(uPending())throw new Error('Upload publication was already requested; inspect before retrying');
const guidance=uGuidance();
if(uDialogs().length||guidance.automaticCheckPrompt||guidance.unacknowledgedGuidanceButtons)throw new Error('Upload dialog requires inspection before publication');
const e=uCaption(),value={};
if(!uEnabled(e)||e.readOnly)throw new Error('Upload caption editor is not editable');
if(typeof e.maxLength==='number'&&e.maxLength>=0&&value.length>e.maxLength)throw new Error('Upload caption exceeds live field maxlength');
e.scrollIntoView({{block:'center',inline:'nearest',behavior:'instant'}});
e.focus();
if(document.activeElement!==e&&!e.contains(document.activeElement))throw new Error('Upload caption focus could not be verified');
let selectedLength=0;
if(e instanceof HTMLInputElement||e instanceof HTMLTextAreaElement){{
 e.setSelectionRange(0,e.value.length);selectedLength=e.selectionEnd-e.selectionStart;
 if(e.selectionStart!==0||e.selectionEnd!==e.value.length)throw new Error('Upload caption selection could not be prepared');
}}else if(e.isContentEditable){{
 const selection=window.getSelection();if(!selection)throw new Error('Upload caption selection could not be prepared');
 const walker=document.createTreeWalker(e,NodeFilter.SHOW_TEXT),nodes=[];
 let node;while((node=walker.nextNode())){{if((node.nodeValue||'').length>0)nodes.push(node);if(nodes.length>4096)throw new Error('Upload caption selection could not be prepared');}}
 const range=document.createRange();
 if(nodes.length){{range.setStart(nodes[0],0);const last=nodes[nodes.length-1];range.setEnd(last,last.nodeValue.length);}}
 else{{const leaf=e.querySelector('[data-offset-key]')||e;range.setStart(leaf,0);range.collapse(true);}}
 selection.removeAllRanges();selection.addRange(range);
 if(!selection.anchorNode||!selection.focusNode||!(selection.anchorNode===e||e.contains(selection.anchorNode))||!(selection.focusNode===e||e.contains(selection.focusNode)))throw new Error('Upload caption selection could not be prepared');
 selectedLength=selection.toString().length;
}}else throw new Error('Upload caption editor is not editable');
return {{nativeInputPrepared:true,focused:true,selectionWithinEditor:true,selectedLengthUtf16:selectedLength,previousLengthUtf16:uRead(e).length,expectedLengthUtf16:value.length,published:false,verifyWith:'native Input.insertText, then delayed upload-caption-check'}};
"#, js_string(value))))
}

/// Read the supplied expected value back as a boolean without returning it.
pub fn caption_matches(value: &str) -> Result<String, String> {
    validate_caption(value)?;
    Ok(body(&format!(
        "const e=uCaption(),expected={};return {{matches:uRead(e)===expected,lengthUtf16:uRead(e).length,expectedLengthUtf16:expected.length}};",
        js_string(value)
    )))
}

/// Move only the recognized editor into view. This changes scroll position,
/// not any field, preference, or publication state.
pub fn prepare_editor() -> String {
    body(r#"
if(uPending())throw new Error('Upload publication was already requested; inspect before retrying');
const guidance=uGuidance();
if(uDialogs().length||guidance.automaticCheckPrompt||guidance.unacknowledgedGuidanceButtons)throw new Error('Upload dialog requires inspection before publication');
const e=uCaption();e.scrollIntoView({block:'center',inline:'nearest',behavior:'instant'});
return {editorScrolledIntoView:true,inViewport:visible(e),captionLengthUtf16:uRead(e).length,published:false};
"#)
}

/// DOM diagnostics are limited to editable public upload fields, media
/// metadata, known upload containers, and guidance. Values and source URLs are
/// omitted; a file input reports names and sizes, never a local path.
pub fn diagnostics() -> String {
    body(r#"
const rect=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height};};
const describe=e=>({tag:e.tagName.toLowerCase(),type:e.getAttribute('type'),role:e.getAttribute('role'),dataE2e:e.getAttribute('data-e2e'),ariaLabel:(e.getAttribute('aria-label')||'').slice(0,180),placeholder:(e.getAttribute('placeholder')||e.getAttribute('data-placeholder')||'').slice(0,180),classes:typeof e.className==='string'?e.className.slice(0,300):'',displayed:uDisplayed(e),inViewport:visible(e),disabled:e.disabled===true,readOnly:e.readOnly===true,contentEditable:e.isContentEditable,rect:rect(e)});
const known=['mobile_preview_container','video_visibility_container','schedule_container','advanced_settings_container','post_video_button','discard_post_button'];
const headings=[...document.querySelectorAll('h1,h2,h3,h4,h5,h6,[role=heading]')].filter(visible).map(uText).filter(t=>t.length>0&&t.length<=200).slice(0,20);
const previewContainers=[...document.querySelectorAll('[data-e2e="mobile_preview_container"]')].filter(uDisplayed);
const mediaPreview={containerCount:previewContainers.length,clocks:[],videos:[],canvases:[],iframes:[],openShadowHosts:[],customElements:[],tagCounts:Object.create(null),scanLimitReached:false};
let visited=0;
const visit=(root,depth,inShadow)=>{
 if(depth>4){mediaPreview.scanLimitReached=true;return;}
 for(const e of [root,...root.querySelectorAll('*')]){
  if(++visited>1200){mediaPreview.scanLimitReached=true;return;}
  if(e.nodeType!==Node.ELEMENT_NODE)continue;
  const tag=e.tagName.toLowerCase(),metadata={tag,displayed:uDisplayed(e),inViewport:visible(e),rect:rect(e),inOpenShadow:inShadow};
  const countKey=Object.prototype.hasOwnProperty.call(mediaPreview.tagCounts,tag)||Object.keys(mediaPreview.tagCounts).length<64?tag:'other';mediaPreview.tagCounts[countKey]=(mediaPreview.tagCounts[countKey]||0)+1;
  if(tag==='video'&&mediaPreview.videos.length<12)mediaPreview.videos.push({...metadata,readyState:e.readyState,duration:Number.isFinite(e.duration)?e.duration:null,videoWidth:e.videoWidth,videoHeight:e.videoHeight,paused:e.paused,muted:e.muted});
  if(tag==='canvas'&&mediaPreview.canvases.length<12)mediaPreview.canvases.push({...metadata,width:e.width,height:e.height});
  if(tag==='iframe'&&mediaPreview.iframes.length<8)mediaPreview.iframes.push({...metadata,hasSrc:e.hasAttribute('src'),sandboxed:e.hasAttribute('sandbox'),title:(e.getAttribute('title')||'').slice(0,120)});
  if(tag.includes('-')&&mediaPreview.customElements.length<20)mediaPreview.customElements.push({...metadata,role:e.getAttribute('role')});
  if(e.shadowRoot&&e.shadowRoot.mode==='open'){
   if(mediaPreview.openShadowHosts.length<12)mediaPreview.openShadowHosts.push({tag,childElementCount:e.shadowRoot.childElementCount});
   visit(e.shadowRoot,depth+1,true);
  }
 }
};
for(const container of previewContainers.slice(0,2)){
 const clock=uText(container).match(/\b(\d{2}:\d{2}:\d{2})\s*\/\s*(\d{2}:\d{2}:\d{2})\b/);
 if(clock)mediaPreview.clocks.push({current:clock[1],duration:clock[2]});
 visit(container,0,false);
}
mediaPreview.elementsVisited=visited;
return {
 url:location.origin+location.pathname,title:document.title.slice(0,200),
 editableCandidates:[...document.querySelectorAll('textarea,[contenteditable=true],input[type=text],input:not([type])')].filter(uDisplayed).slice(0,20).map(describe),
 fileInputs:[...document.querySelectorAll('input[type=file]')].slice(0,10).map(e=>({tag:'input',type:'file',accept:e.accept,multiple:e.multiple,disabled:e.disabled,rect:rect(e),files:[...e.files].map(f=>({name:f.name,size:f.size,type:f.type}))})),
 videos:[...document.querySelectorAll('video')].slice(0,10).map(e=>({...describe(e),readyState:e.readyState,duration:Number.isFinite(e.duration)?e.duration:null,videoWidth:e.videoWidth,videoHeight:e.videoHeight,currentTime:e.currentTime,paused:e.paused,muted:e.muted})),
 headings,guidance:uGuidance(),mediaPreview,uploadedFile:uUploadFileLabels(),
 relevantNodes:[...document.querySelectorAll('[data-e2e]')].filter(e=>known.includes(e.getAttribute('data-e2e'))&&uDisplayed(e)).slice(0,16).map(e=>({...describe(e),text:uText(e).slice(0,500)})),
 guidancePanels:[...uAutoPanels(),...uTutorialPanels()].slice(0,4).map(e=>({text:uText(e).slice(0,1400),buttons:[...e.querySelectorAll('button')].filter(visible).map(b=>({label:uText(b),disabled:!uEnabled(b)}))})),
 limit:'Public upload DOM metadata only; no field values, cookies, storage, request tokens, or video source URLs.'
};
"#)
}

/// Dismiss only the two observed upload-onboarding surfaces. Cancellation of
/// the opt-in prompt leaves the account's automatic-check preference unchanged.
/// A tutorial acknowledgement never clicks Publish or changes an input.
pub fn dismiss_guidance(action: &str) -> Result<String, String> {
    let (panels, label, panel_error, button_error) = match action {
        "cancel-auto-check" => ("uAutoPanels()", "Отмена", "Upload automatic-check prompt must be unique", "Upload automatic-check cancel button must be unique"),
        "acknowledge-tutorial" => ("uTutorialPanels()", "Понятно", "Upload tutorial panel must be unique", "Upload tutorial acknowledgement must be unique"),
        _ => return Err("Upload guidance action is unsupported".into()),
    };
    Ok(body(&format!(r#"
if(uPending())throw new Error('Upload publication was already requested; inspect before retrying');
const panels={panels};if(panels.length!==1)throw new Error('{panel_error}');
const buttons=[...panels[0].querySelectorAll('button')].filter(e=>visible(e)&&uText(e)==={label});
if(buttons.length!==1)throw new Error('{button_error}');
if(!uEnabled(buttons[0]))throw new Error('Upload guidance action is disabled');
buttons[0].click();return {{guidanceActionRequested:{action},published:false,verifyWith:'upload-inspect'}};
"#, label=js_string(label), action=js_string(action))))
}

/// Separate read-only scope: publication may redirect from Upload to Content
/// or an ordinary profile/video route. Inspect only bounded public DOM nodes.
pub fn publication_inspect() -> String {
    r#"
const hosts=['www.tiktok.com','tiktok.com','studio.tiktok.com'];
const path=decodeURIComponent(location.pathname).replace(/\/$/,'');
const isUpload=/^\/(?:tiktokstudio\/upload|creator-center\/upload|upload)$/.test(path);
const isContent=/^\/(?:tiktokstudio\/content|creator-center\/content)$/.test(path);
const isProfile=/^\/@[^/]+(?:\/video\/\d+)?$/.test(path);
const localFixture=fixture&&(location.protocol==='tauri:'||location.hostname==='tauri.localhost');
if(!localFixture&&!(location.protocol==='https:'&&hosts.includes(location.hostname)&&(!location.port||location.port==='443')&&(isUpload||isContent||isProfile)))throw new Error('Publication inspection requires an ordinary TikTok upload, content, or profile page');
const text=e=>(e.innerText||e.textContent||'').trim().replace(/\s+/g,' ');
const postLink=a=>{try{const u=new URL(a.href,location.href);if(u.protocol==='https:'&&hosts.includes(u.hostname)&&(!u.port||u.port==='443')&&/^\/@[^/]+\/video\/\d+\/?$/.test(decodeURIComponent(u.pathname)))return {url:u.origin+u.pathname.replace(/\/$/,''),text:text(a).slice(0,200)};}catch{}return null;};
const links=root=>[...new Map([...root.querySelectorAll('a[href]')].filter(visible).map(postLink).filter(Boolean).map(a=>[a.url,a])).values()].slice(0,40);
const notices=[...document.querySelectorAll('[role=status],[role=alert]')].filter(visible).map(text).filter(s=>s.length>0&&s.length<=500).slice(0,16);
const headings=[...document.querySelectorAll('h1,h2,h3,[role=heading]')].filter(visible).map(text).filter(s=>s.length>0&&s.length<=300).slice(0,16);
const exactSuccess=s=>/^(?:Видео (?:успешно )?опубликовано[.!]?|Your video (?:has been|was) (?:successfully )?published[.!]?|Video (?:successfully )?published[.!]?|Post published[.!]?)$/i.test(s);
const exactError=s=>/^(?:Что-то пошло не так[.!]?|Что то пошло не так[.!]?|Something went wrong[.!]?|Не удалось (?:загрузить|опубликовать)(?: видео)?[.!]?|Upload failed[.!]?)$/i.test(s);
const errorLabels=[...new Set([...document.querySelectorAll('p,span,div,h1,h2,h3,[role=alert]')].filter(e=>visible(e)&&e.children.length<=1).map(text).filter(s=>s.length<=200&&exactError(s)))].slice(0,8);
const rows=isContent?[...document.querySelectorAll('tr,[role=row]')].filter(visible).filter(r=>links(r).length>0).slice(0,25).map(r=>({text:text(r).slice(0,600),videoLinks:links(r).slice(0,4)})):[];
return {url:location.origin+location.pathname,title:document.title.slice(0,200),pageType:isUpload?'upload':isContent?'content':isProfile?'profile-or-video':'fixture',notices,headings,errorLabels,videoLinks:links(document),contentRows:rows,explicitPublicationSuccessObserved:[...notices,...headings].some(exactSuccess),limit:'Visible public DOM only. A video link or an upload receipt alone does not prove a successful new publication or rights clearance.'};
"#.to_owned()
}

/// Typed final action. Caller must already hold authorization to publish.
/// Set the marker before dispatch: a timeout or thrown click may be uncertain,
/// and therefore must never trigger an automatic second publication attempt.
pub fn publish_request(expected_caption: &str) -> Result<String, String> {
    validate_caption(expected_caption)?;
    Ok(body(&format!(r#"
const button=uPublicationReady();
const expected={};
if(uRead(uCaption())!==expected)throw new Error('Upload current caption does not match the authorized publication');
button.scrollIntoView({{block:'center',inline:'nearest',behavior:'instant'}});
document.documentElement.dataset.tnPublishRequested='true';
button.click();
return {{publicationRequested:true,expectedCaptionMatched:true,published:false,noAutomaticRetry:true,verifyWith:'publication-status, then inspect the actual profile/video'}};
"#, js_string(expected_caption))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_limit_counts_utf16_and_rejects_empty_or_controls() {
        assert!(validate_caption("Кино 🎬\n#фильмы").is_ok());
        assert!(validate_caption(&"a".repeat(CAPTION_MAX_UTF16)).is_ok());
        assert!(validate_caption(&"🎬".repeat(CAPTION_MAX_UTF16 / 2)).is_ok());
        assert!(validate_caption(&format!("{}x", "🎬".repeat(CAPTION_MAX_UTF16 / 2))).is_err());
        assert!(validate_caption(" \n\t").is_err());
        assert!(validate_caption("caption\0hidden").is_err());
        assert!(validate_caption("caption\rhidden").is_err());
    }

    #[test]
    fn untrusted_text_is_one_escaped_string() {
        let hostile = "\";globalThis.unexpected=true;//</script>\\\n\u{2028}\u{2029}&";
        assert_eq!(js_string(hostile), "\"\\\";globalThis.unexpected=true;//\\u003c/script\\u003e\\\\\\n\\u2028\\u2029\\u0026\"");
        let script = caption_prepare_native(hostile).unwrap();
        assert!(script.contains(&format!("const e=uCaption(),value={};", js_string(hostile))));
        assert!(!script.contains("</script>"));
    }

    #[test]
    fn exact_error_allowlist_does_not_forward_page_text() {
        let safe = "Upload publication button is disabled";
        assert_eq!(public_error(safe), Some(safe));
        assert_eq!(public_error(&format!("{safe}: untrusted details")), None);
        assert_eq!(public_error("TypeError: session data"), None);
    }

    #[test]
    fn fixed_bodies_keep_scope_and_uncertain_publication_receipt() {
        for script in [inspect(), diagnostics(), prepare_editor(), caption_prepare_native("Caption").unwrap(), caption_matches("Caption").unwrap(), publish_request("Caption").unwrap()] {
            assert!(script.contains("uScope();"));
            assert!(!script.contains("localStorage"));
            assert!(!script.contains("document.cookie"));
        }
        let request = publish_request("Caption").unwrap();
        assert!(request.find("dataset.tnPublishRequested='true'").unwrap() < request.find("button.click()").unwrap());
        assert!(request.contains("published:false"));
        assert!(!request.contains("published:true"));
        let prepare = caption_prepare_native("Caption").unwrap();
        assert!(prepare.contains("nativeInputPrepared:true"));
        assert!(!prepare.contains("execCommand"));
        assert!(!prepare.contains("textContent=value"));
    }

    #[test]
    fn publication_requires_expected_caption_and_guidance_actions_are_fixed() {
        assert!(publish_request("").is_err());
        assert!(publish_request("Expected").unwrap().contains("uRead(uCaption())!==expected"));
        assert!(dismiss_guidance("cancel-auto-check").unwrap().contains("uAutoPanels()"));
        assert!(dismiss_guidance("acknowledge-tutorial").unwrap().contains("uTutorialPanels()"));
        assert!(dismiss_guidance("publish").is_err());
        assert!(!publication_inspect().contains("uScope();"));
        assert!(publication_inspect().contains("const isProfile="));
        assert!(publication_inspect().contains("const isContent="));
    }
}
