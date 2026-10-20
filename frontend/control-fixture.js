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
