document.querySelector('#fixture-button').addEventListener('click',()=>{
 document.querySelector('#fixture-result').textContent='Clicked successfully';
 const first=document.querySelector('[data-e2e="user-title"]'),second=document.querySelector('[data-e2e="user-subtitle"]');
 const old=first.textContent;first.textContent=second.textContent;second.textContent=old;
});
document.querySelector('#fixture-auth').addEventListener('click',()=>{document.querySelector('#fixture-login').hidden=false;});
document.querySelector('#caption').addEventListener('input',e=>{document.querySelector('#fixture-caption').textContent='Caption characters: '+e.target.value.length;});
document.querySelector('#fixture-file').addEventListener('change',e=>{document.querySelector('#fixture-result').textContent='Selected '+e.target.files[0]?.name;});
document.querySelector('[data-e2e="edit-profile-entrance"]').addEventListener('click',()=>{document.querySelector('#fixture-profile').hidden=false;});
document.querySelector('#fixture-avatar').addEventListener('change',()=>{document.querySelector('#fixture-crop').hidden=false;});
document.querySelector('#fixture-apply').addEventListener('click',()=>{document.querySelector('#fixture-crop').hidden=true;});
let pendingProfile=null;
function applyProfile(){
 document.querySelector('[data-e2e="user-title"]').textContent=pendingProfile.name;
 document.querySelector('[data-e2e="user-bio"]').textContent=pendingProfile.bio;
 document.querySelector('[data-e2e="user-subtitle"]').textContent=pendingProfile.username;
 pendingProfile=null;
}
document.querySelector('#fixture-save').addEventListener('click',()=>{
 pendingProfile={name:document.querySelector('#fixture-name').value,bio:document.querySelector('#fixture-bio').value,username:document.querySelector('#fixture-username').value};
 document.querySelector('#fixture-profile').hidden=true;
 if(pendingProfile.name!==document.querySelector('[data-e2e="user-title"]').textContent)document.querySelector('#fixture-confirm').hidden=false;
 else applyProfile();
});
document.querySelector('[data-e2e="set-username-popup-confirm"]').addEventListener('click',()=>{
 applyProfile();document.querySelector('#fixture-confirm').hidden=true;
});
// Only the isolated fixture sets synthetic preview metadata; no real upload.
Object.defineProperties(document.querySelector('#fixture-video'),{
 readyState:{get:()=>3},duration:{get:()=>12},videoWidth:{get:()=>270},videoHeight:{get:()=>480}
});
document.querySelector('#fixture-upload-mode').addEventListener('click',()=>{
 document.querySelector('#fixture-basic-caption').hidden=true;
 document.querySelector('#publish').hidden=true;
 document.querySelector('[data-e2e="user-page"]').hidden=true;
 document.querySelector('#fixture-upload').hidden=false;
});
document.querySelector('#fixture-upload-progress').addEventListener('click',()=>{const e=document.querySelector('#fixture-progress');e.hidden=!e.hidden;});
document.querySelector('#fixture-canvas-mode').addEventListener('click',()=>{const video=document.querySelector('#fixture-video'),canvas=document.querySelector('#fixture-canvas-player');video.hidden=!video.hidden;canvas.hidden=!canvas.hidden;});
document.querySelector('#fixture-canvas-clock').addEventListener('click',()=>{const clock=document.querySelector('#fixture-clock');clock.textContent=clock.textContent==='00:00:00 / 00:01:02'?'00:00:00 / 00:00:00':'00:00:00 / 00:01:02';});
document.querySelector('#fixture-auto-prompt').addEventListener('click',()=>{document.querySelector('#fixture-auto').hidden=false;});
document.querySelector('#fixture-auto-cancel').addEventListener('click',()=>{document.querySelector('#fixture-auto').hidden=true;});
document.querySelector('#fixture-tutorial-prompt').addEventListener('click',()=>{document.querySelector('#fixture-tutorial').hidden=false;});
document.querySelector('#fixture-tutorial-ack').addEventListener('click',()=>{document.querySelector('#fixture-tutorial').hidden=true;});
let fixturePostCount=0;
document.querySelector('#fixture-post').addEventListener('click',()=>{
 fixturePostCount++;
 document.querySelector('#fixture-result').textContent='Fixture publication requests: '+fixturePostCount;
 const link=document.createElement('a');link.href='https://www.tiktok.com/@your_handle/video/1234567890123456789';link.textContent='View local test post';document.querySelector('#fixture-upload').append(link);
});
