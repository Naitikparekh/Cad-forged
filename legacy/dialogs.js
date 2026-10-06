'use strict';
// Accessible in-app dialogs work in browsers that disable window.prompt.
const dialogStyle=document.createElement('style');dialogStyle.textContent='dialog{background:#252d39;color:#e5edf7;border:1px solid #718096;border-radius:6px;width:390px;padding:24px;box-shadow:0 12px 60px #0008}dialog::backdrop{background:#0008}dialog input{width:100%;margin:14px 0}dialog .actions{display:flex;justify-content:flex-end;gap:8px}dialog h2{font-size:17px;margin:0 0 12px}';document.head.append(dialogStyle);
const inputDialog=document.createElement('dialog');inputDialog.innerHTML='<form method="dialog"><h2>CadForge</h2><label id="dialogLabel" for="dialogValue"></label><input id="dialogValue" autocomplete="off"><div class="actions"><button value="cancel" type="button" id="dialogCancel">Cancel</button><button value="ok" id="dialogOk">OK</button></div></form>';document.body.append(inputDialog);
let pendingDialog=null;
function cadPrompt(message,initial=''){return new Promise(resolve=>{if(pendingDialog)pendingDialog(null);pendingDialog=resolve;$('dialogLabel').textContent=message;$('dialogValue').value=initial;$('dialogValue').hidden=false;inputDialog.showModal();$('dialogValue').focus();$('dialogValue').select()})}
function cadConfirm(message){const result=cadPrompt(message);$('dialogValue').hidden=true;$('dialogOk').focus();return result.then(v=>v!==null)}
inputDialog.querySelector('form').onsubmit=e=>{e.preventDefault();const value=$('dialogValue').value;inputDialog.close();const resolve=pendingDialog;pendingDialog=null;resolve?.(value)};
$('dialogCancel').onclick=()=>{inputDialog.close();const resolve=pendingDialog;pendingDialog=null;resolve?.(null)};
inputDialog.oncancel=e=>{e.preventDefault();$('dialogCancel').click()};


