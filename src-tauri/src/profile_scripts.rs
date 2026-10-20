//! Fixed DOM operations for the current account's Edit profile form.
//!
//! These are bodies for control_webview::evaluate, not a script endpoint. The
//! caller must prepend its host/authentication guard (which defines `visible`
//! and `fixture`), bind the operation to the main WebView, and require explicit
//! authorization for save/avatar selection. A requested click is never proof
//! that TikTok accepted or persisted a change.
//!
//! Observed 2026-10-20: user-page/user-title/user-subtitle/user-bio/user-avatar
//! and button[data-e2e="edit-profile-entrance"] on the own profile. The live
//! form's public fields, image accept list, and separate set-username-popup
//! nickname confirmation were also observed. Crop containers use bounded
//! semantic discovery: unknown/ambiguous forms deliberately fail closed.

pub const DISPLAY_NAME_MAX: usize = 30;
pub const BIO_MAX: usize = 80;
pub const USERNAME_MAX: usize = 24;

/// Only these exact, locally generated DOM errors may be shown by the parent
/// evaluator. Never forward an arbitrary exception or a prefix match.
pub fn public_error(message: &str) -> Option<&'static str> {
    const MESSAGES: &[&str] = &[
        "Profile page must be unique",
        "Profile edit entrance must be unique",
        "Profile username must be unique",
        "Profile edit dialog must be unique",
        "Profile displayname field must be unique",
        "Profile username field must be unique",
        "Profile bio field must be unique",
        "Profile save button must be unique",
        "Profile image file input must be unique",
        "Profile crop dialog must be unique",
        "Profile crop apply button must be unique",
        "Profile name confirmation must be unique",
        "Profile name confirmation button must be unique",
        "Profile name confirmation is not recognized",
        "Profile operation requires the current own profile page",
        "Profile page identity does not match its URL",
        "Profile edit dialog requires a profile page",
        "Profile action is disabled",
        "Profile edit dialog is already open",
        "Profile field is unsupported",
        "Profile value exceeds live field maxlength",
        "Profile field has no native value setter",
        "Profile field readback did not match",
    ];
    MESSAGES.iter().copied().find(|allowed| *allowed == message)
}

// Nickname 30 and the username character/end-period rules are documented at:
// https://support.tiktok.com/en/getting-started/setting-up-your-profile/changing-your-nickname
// https://support.tiktok.com/en/getting-started/setting-up-your-profile/changing-your-username
// Bio 80 / username 24 are conservative local limits, also bounded by the live
// form's maxlength. UTF-16 counting intentionally agrees with HTML maxlength;
// it is not a promise that TikTok counts extended grapheme clusters the same way.
pub fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

fn canonical_field(field: &str) -> Result<&'static str, String> {
    match field {
        "displayname" | "display-name" | "name" => Ok("displayname"),
        "bio" => Ok("bio"),
        "username" => Ok("username"),
        _ => Err("Profile field must be displayname, bio, or username".into()),
    }
}

pub fn validate_field(field: &str, value: &str) -> Result<(), String> {
    let field = canonical_field(field)?;
    if value.chars().any(|c| c.is_control() && !(field == "bio" && c == '\n')) {
        return Err("Profile text contains unsupported control characters".into());
    }
    let limit = match field {
        "displayname" => DISPLAY_NAME_MAX,
        "bio" => BIO_MAX,
        "username" => USERNAME_MAX,
        _ => unreachable!(),
    };
    if utf16_len(value) > limit {
        return Err(format!("Profile {field} exceeds {limit} UTF-16 units"));
    }
    if field != "bio" && (value.trim().is_empty() || value.trim() != value) {
        return Err("Profile name must be nonempty without surrounding whitespace".into());
    }
    if field == "username" {
        // This local command supports the conservative ASCII subset needed for
        // the channel. It does not claim all Unicode letters are server-invalid.
        if !value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.') {
            return Err("Profile username supports ASCII letters, digits, underscores, and periods".into());
        }
        if value.ends_with('.') {
            return Err("Profile username cannot end with a period".into());
        }
    }
    Ok(())
}

/// Quote user text as one JavaScript string. No user value is interpolated into
/// a CSS selector, property name, or executable fragment.
fn js_string(value: &str) -> String {
    let mut encoded = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            '\u{8}' => encoded.push_str("\\b"),
            '\u{c}' => encoded.push_str("\\f"),
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
const pText=e=>(e.innerText||e.textContent||'').trim();
const pOne=(items,label)=>{const a=[...new Set(items)];if(a.length!==1)throw new Error('Profile '+label+' must be unique');return a[0];};
const pEditText=s=>/^(Изменить профиль|Редактировать профиль|Edit profile)$/i.test(s);
const pSaveText=s=>/^(Сохранить|Save)$/i.test(s);
const pInputs=root=>[...root.querySelectorAll('input:not([type]),input[type=text],textarea')].filter(e=>visible(e)&&!e.disabled&&!e.readOnly);
const pButtons=root=>[...root.querySelectorAll('button,[role=button]')].filter(visible);
function pOwn(){
 const path=decodeURIComponent(location.pathname).replace(/\/$/,'');
 const match=path.match(/^\/@([^/]+)$/);
 if(!fixture&&!match)throw new Error('Profile operation requires the current own profile page');
 const page=pOne([...document.querySelectorAll('[data-e2e="user-page"]')].filter(visible),'page');
 const edit=pOne([...page.querySelectorAll('button[data-e2e="edit-profile-entrance"]')].filter(e=>visible(e)&&pEditText(pText(e))),'edit entrance');
 const title=pOne([...page.querySelectorAll('[data-e2e="user-title"]')].filter(visible),'username');
 const subtitle=pOne([...page.querySelectorAll('[data-e2e="user-subtitle"]')].filter(visible),'username');
 const primary=pText(title),secondary=pText(subtitle);
 // TikTok's visible title/subtitle roles change after a nickname is set.
 // Establish identity from the route and either explicit public heading.
 const username=fixture?(page.dataset.fixtureUsername||primary.replace(/^@/,'')):match[1];
 const isHandle=s=>s.replace(/^@/,'').toLowerCase()===username.toLowerCase();
 if(!username||(!isHandle(primary)&&!isHandle(secondary)))throw new Error('Profile page identity does not match its URL');
 const displayname=isHandle(secondary)?primary:secondary;
 return {page,edit,username,displayname};
}
function pPublic(own){
 const read=sel=>{const es=[...own.page.querySelectorAll(sel)].filter(visible);return es.length===1?pText(es[0]):null;};
 const images=[...own.page.querySelectorAll('[data-e2e="user-avatar"] img')].filter(visible);
 return {username:own.username,displayname:own.displayname,bio:read('[data-e2e="user-bio"]'),avatarPresent:images.length>0};
}
function pTitledDialogs(titleTest){
 const explicit=[...document.querySelectorAll('[role=dialog],[aria-modal=true],dialog[open]')].filter(e=>visible(e)&&[...e.querySelectorAll('h1,h2,h3,[role=heading],div,span')].some(h=>visible(h)&&titleTest(pText(h))));
 let candidates=explicit;
 if(candidates.length===0){
  // Some TikTok builds omit dialog roles. Use only the smallest labelled form
  // containing multiple public fields plus its own Save button, never body.
  candidates=[];
  const titles=[...document.querySelectorAll('h1,h2,h3,[role=heading],div,span')].filter(e=>visible(e)&&titleTest(pText(e)));
  for(const heading of titles){
   let e=heading.parentElement;
   for(let i=0;e&&i<6;i++,e=e.parentElement){
    if(e===document.body||e===document.documentElement)break;
    if(visible(e)&&pInputs(e).length>=2&&pButtons(e).some(b=>pSaveText(pText(b)))){candidates.push(e);break;}
   }
  }
 }
 const unique=[...new Set(candidates)];
 return unique.filter(e=>!unique.some(other=>other!==e&&e.contains(other)));
}
function pDialog(){
 if(!fixture&&!/^\/@[^/]+\/?$/.test(decodeURIComponent(location.pathname)))throw new Error('Profile edit dialog requires a profile page');
 return pOne(pTitledDialogs(pEditText),'edit dialog');
}
const pFieldNames={
 displayname:/^(имя|отображаемое имя|никнейм|name|nickname|display name)$/i,
 username:/^(имя пользователя|username|user name|uniqueid|unique_id)$/i,
 bio:/^(описание|биография|о себе|bio|biography|signature)$/i
};
function pFieldLabels(e,dialog){
 const labels=[e.getAttribute('aria-label'),e.getAttribute('placeholder'),e.getAttribute('name'),e.id];
 if(e.labels)for(const label of e.labels)labels.push(pText(label));
 const labelled=e.getAttribute('aria-labelledby');
 if(labelled)for(const id of labelled.split(/\s+/)){const label=document.getElementById(id);if(label&&dialog.contains(label))labels.push(pText(label));}
 // Unassociated visible row labels are accepted only while that row contains
 // exactly one editable public field, so Name never selects Username.
 let row=e.parentElement;
 for(let i=0;row&&row!==dialog&&i<4;i++,row=row.parentElement){
  if(pInputs(row).length!==1)break;
  labels.push(pText(row).split('\n').map(s=>s.trim()).filter(Boolean)[0]);
 }
 return labels.filter(s=>typeof s==='string'&&s.length).map(s=>s.trim().replace(/[:：]$/,''));
}
function pField(dialog,field){
 const test=pFieldNames[field];if(!test)throw new Error('Profile field is unsupported');
 return pOne(pInputs(dialog).filter(e=>pFieldLabels(e,dialog).some(label=>test.test(label))),field+' field');
}
function pImageInputs(dialog){
 return [...dialog.querySelectorAll('input[type=file]')].filter(e=>{
  if(e.disabled||e.multiple)return false;
  const types=(e.getAttribute('accept')||'').toLowerCase().split(',').map(x=>x.trim()).filter(Boolean);
  return types.length>0&&types.every(x=>x.startsWith('image/')||/^\.(png|jpg|jpeg|webp|gif|bmp|heic|heif|tif|tiff)$/.test(x));
 });
}
function pEnabled(button){if(button.disabled||button.getAttribute('aria-disabled')==='true')throw new Error('Profile action is disabled');}
const pCropTitle=s=>/^(Редактировать фото|Изменить фото|Обрезать фото|Edit photo|Crop photo)$/i.test(s);
// Russian TikTok currently translates Apply as "Подать заявку" in this crop
// dialog. This is accepted only under an image-crop title and preview.
const pCropApply=e=>/^(Применить|Подать заявку|Apply|Submit)$/i.test(pText(e));
const pCropHeadings=()=>[...document.querySelectorAll('h1,h2,h3,[role=heading],div,span,p,strong')].filter(e=>visible(e)&&pCropTitle(pText(e)));
const pCropPreviews=d=>[...d.querySelectorAll('canvas,img,[role=img],[style*="background-image"]')].filter(visible);
function pCropCandidates(){
 const eligible=d=>pCropPreviews(d).length>0&&pButtons(d).filter(pCropApply).length===1;
 let dialogs=[...document.querySelectorAll('[role=dialog],[aria-modal=true],dialog[open]')].filter(d=>visible(d)&&eligible(d)&&pCropHeadings().some(h=>d.contains(h)));
 if(!dialogs.length){
  for(const heading of pCropHeadings()){
   let e=heading.parentElement;
   for(let i=0;e&&i<8;i++,e=e.parentElement){if(e===document.body||e===document.documentElement)break;if(visible(e)&&eligible(e)){dialogs.push(e);break;}}
  }
 }
 dialogs=[...new Set(dialogs)];return dialogs.filter(d=>!dialogs.some(other=>other!==d&&d.contains(other)));
}
"#;

fn body(operation: &str) -> String {
    format!("{HELPERS}\n{operation}")
}

pub fn open() -> String {
    body(r#"
const own=pOwn();
if(pTitledDialogs(pEditText).length>0)throw new Error('Profile edit dialog is already open');
pEnabled(own.edit);own.edit.click();
return {profile_edit_requested:true,username:own.username,verify_with:'profile-inspect'};
"#)
}

/// Read only the public profile values and its explicitly recognized edit
/// fields. Unknown inputs never expose their values in a diagnostic response.
pub fn inspect() -> String {
    body(r#"
const dialog=pDialog();
const result={dialogOpen:true,fields:[],fileInputs:[],buttons:[]};
 for(const e of pInputs(dialog)){
  const labels=pFieldLabels(e,dialog);const fields=Object.keys(pFieldNames).filter(k=>labels.some(s=>pFieldNames[k].test(s)));
  result.fields.push({field:fields.length===1?fields[0]:null,tag:e.tagName.toLowerCase(),type:e.getAttribute('type')||'text',labels:labels.map(s=>s.slice(0,120)),maxlength:e.maxLength>=0?e.maxLength:null,value:fields.length===1?e.value:null});
 }
 result.fileInputs=[...dialog.querySelectorAll('input[type=file]')].map(e=>({accept:e.getAttribute('accept'),multiple:e.multiple,disabled:e.disabled,imageOnly:pImageInputs(dialog).includes(e)}));
 result.buttons=pButtons(dialog).map(e=>({text:pText(e).slice(0,100),disabled:!!e.disabled||e.getAttribute('aria-disabled')==='true'}));
return result;
"#)
}

/// Public values after the edit modal closed. Requires the own-profile Edit
/// entrance, unlike inspect(), which is confined to the open edit modal.
pub fn readback() -> String {
    body("const own=pOwn();return {profile:pPublic(own)};")
}

/// Modify one form field only; never clicks Save or submits a form.
pub fn field_fill(field: &str, value: &str) -> Result<String, String> {
    validate_field(field, value)?;
    let field = canonical_field(field)?;
    Ok(body(&format!(r#"
const dialog=pDialog();const field={field};const text={value};const e=pField(dialog,field);
if(e.maxLength>=0&&text.length>e.maxLength)throw new Error('Profile value exceeds live field maxlength');
const proto=e instanceof HTMLTextAreaElement?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;
const setter=Object.getOwnPropertyDescriptor(proto,'value')?.set;
if(!setter)throw new Error('Profile field has no native value setter');
e.focus();setter.call(e,text);
e.dispatchEvent(new InputEvent('input',{{bubbles:true,inputType:'insertText',data:text}}));
e.dispatchEvent(new Event('change',{{bubbles:true}}));e.blur();
if(e.value!==text)throw new Error('Profile field readback did not match');
return {{filled:true,field,value:e.value,saved:false}};
"#, field=js_string(field), value=js_string(value))))
}

/// Explicit final action. Call only after all filled fields/avatar are reviewed;
/// success means requested, not saved. Read the profile again after completion.
pub fn save() -> String {
    body(r#"
const dialog=pDialog();
const button=pOne(pButtons(dialog).filter(e=>pSaveText(pText(e))),'save button');
pEnabled(button);button.click();
return {profile_save_requested:true,verify_with:'profile-inspect or profile-readback'};
"#)
}

/// TikTok can require this second, separate confirmation for a display-name
/// change. Its observed data-e2e says username, but the popup explicitly says
/// nickname and seven days. This action is not a generic dialog confirmer.
pub fn confirm_name() -> String {
    body(r#"
pOwn();
const popup=pOne([...document.querySelectorAll('[data-e2e="set-username-popup"]')].filter(visible),'name confirmation');
const title=popup.querySelector('[data-e2e="set-username-popup-title"]');
const desc=popup.querySelector('[data-e2e="set-username-popup-desc"]');
if(!title||!desc||! /^(Установить никнейм\?|Set nickname\?|Set your nickname\?)$/i.test(pText(title))||! /7/.test(pText(desc)))throw new Error('Profile name confirmation is not recognized');
const button=pOne([...popup.querySelectorAll('button[data-e2e="set-username-popup-confirm"]')].filter(e=>visible(e)&&/^(Подтвердить|Confirm)$/i.test(pText(e))),'name confirmation button');
pEnabled(button);button.click();return {profile_name_confirmation_requested:true,verify_with:'profile-readback after reload'};
"#)
}

/// Evaluate with returnByValue=false. Use only the returned element's objectId
/// for DOM.setFileInputFiles; input discovery itself cannot open a file picker.
pub fn avatar_input() -> String {
    body(r#"
const dialog=pDialog();return pOne(pImageInputs(dialog),'image file input');
"#)
}

/// Crop confirmation is deliberately separate from profile-save. Only a
/// semantically labelled crop/photo dialog containing image/canvas preview and
/// exactly one Apply button is eligible. If TikTok uses another UI, inspect it
/// and update this fixed operation instead of adding an arbitrary click escape.
pub fn crop_apply() -> String {
    body(r#"
if(!fixture&&!/^\/@[^/]+\/?$/.test(decodeURIComponent(location.pathname)))throw new Error('Profile edit dialog requires a profile page');
const dialog=pOne(pCropCandidates(),'crop dialog');
const button=pOne(pButtons(dialog).filter(pCropApply),'crop apply button');
pEnabled(button);button.click();return {crop_apply_requested:true,profile_saved:false,verify_with:'profile-inspect'};
"#)
}

/// Bounded crop diagnostics: no arbitrary DOM dump, input values, or scripts.
pub fn crop_inspect() -> String {
    body(r#"
if(!fixture&&!/^\/@[^/]+\/?$/.test(decodeURIComponent(location.pathname)))throw new Error('Profile edit dialog requires a profile page');
const dialogs=pCropCandidates();
return {cropCandidateCount:dialogs.length,titles:[...new Set(pCropHeadings().map(pText))],candidates:dialogs.map(d=>({previewCount:pCropPreviews(d).length,buttons:pButtons(d).map(b=>({text:pText(b).slice(0,100),disabled:!!b.disabled||b.getAttribute('aria-disabled')==='true'}))}))};
"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_limits_count_characters_not_utf8_bytes() {
        assert_eq!(utf16_len("Кино"), 4);
        assert!(validate_field("displayname", &"Я".repeat(30)).is_ok());
        assert!(validate_field("displayname", &"Я".repeat(31)).is_err());
        assert!(validate_field("bio", &"Я".repeat(80)).is_ok());
        assert!(validate_field("bio", &"Я".repeat(81)).is_err());
    }

    #[test]
    fn emoji_agree_with_html_utf16_maxlength() {
        assert_eq!(utf16_len("🎬"), 2);
        assert_eq!(utf16_len("👩‍💻"), 5);
        assert!(validate_field("displayname", &"🎬".repeat(15)).is_ok());
        assert!(validate_field("displayname", &"🎬".repeat(16)).is_err());
        assert!(validate_field("bio", &"🎬".repeat(40)).is_ok());
        assert!(validate_field("bio", &format!("{}Я", "🎬".repeat(40))).is_err());
    }

    #[test]
    fn username_rules_are_bounded_and_do_not_silently_rewrite() {
        for value in ["MovieDemo", "your_handle", "movie.demo", "a_2", "123"] {
            assert!(validate_field("username", value).is_ok(), "{value}");
        }
        for value in ["", " ", " kino", "kino ", "@kino", "кино", "kino-ff", "kino.", "kino\n"] {
            assert!(validate_field("username", value).is_err(), "{value}");
        }
        assert!(validate_field("username", &"a".repeat(24)).is_ok());
        assert!(validate_field("username", &"a".repeat(25)).is_err());
    }

    #[test]
    fn profile_fields_reject_unknown_and_control_input() {
        assert!(validate_field("password", "text").is_err());
        assert!(validate_field("name", "Movie\nDemo").is_err());
        assert!(validate_field("bio", "Кино\nМоменты").is_ok());
        assert!(validate_field("bio", "Movie\0Demo").is_err());
        assert!(validate_field("bio", "").is_ok());
    }

    #[test]
    fn js_quoting_cannot_end_the_string_or_embed_markup() {
        assert_eq!(js_string("a\"\\\n\r\t\u{8}\u{c}"), "\"a\\\"\\\\\\n\\r\\t\\b\\f\"");
        assert_eq!(js_string("<>&\u{2028}\u{2029}\u{1}"), "\"\\u003c\\u003e\\u0026\\u2028\\u2029\\u0001\"");
        assert_eq!(js_string("Кино🎬"), "\"Кино🎬\"");
        let value = "\");document.body.remove();//";
        let script = field_fill("bio", value).unwrap();
        assert!(script.contains(&format!("const text={};const e=pField", js_string(value))));
        assert!(!script.contains("const text=\"\");document.body.remove()"));
    }

    #[test]
    fn user_text_is_never_interpolated_as_css() {
        let value = "x\"] input[type=password], [name=\"x";
        let script = field_fill("bio", value).unwrap();
        assert!(script.contains(&format!("const text={}", js_string(value))));
        assert!(!script.contains(&format!("querySelector({}", js_string(value))));
        assert!(!script.contains(".submit()"));
        assert!(field_fill("bio];alert(1)//", "ok").is_err());
    }

    #[test]
    fn saving_and_selecting_are_separate_fixed_operations() {
        let fill = field_fill("name", "MovieDemo").unwrap();
        assert!(fill.contains("saved:false"));
        assert!(!fill.contains("button.click()"));
        assert!(open().contains("own.edit.click()"));
        assert!(save().contains("profile_save_requested:true"));
        assert!(avatar_input().contains("return pOne(pImageInputs(dialog)"));
        assert!(!avatar_input().contains(".click()"));
    }

    #[test]
    fn only_exact_local_errors_are_public() {
        assert!(public_error("Profile image file input must be unique").is_some());
        assert!(public_error("Profile secret: arbitrary remote data").is_none());
        assert!(public_error("Profile save button must be unique: remote suffix").is_none());
    }
}
