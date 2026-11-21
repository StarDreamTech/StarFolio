'use strict';
(() => {
  const panel=$('aboutPanel'), auto=$('autoUpdate'), check=$('checkUpdate'), install=$('installUpdate');
  const close=$('dismissAbout'), state=$('updateState'), pref='starfolio.autoUpdate';
  let supported=false, busy=false, version;
  const sync=()=>{check.disabled=busy; install.disabled=busy; close.disabled=busy; install.hidden=!version};
  const show=()=>{if(!panel.open)panel.showModal()};
  $('aboutBtn').onclick=show;
  $('githubBtn').onclick=()=>invoke('open_link',{url:'https://github.com/StarDreamTech/StarFolio'}).catch(e=>status('无法打开 GitHub：'+e,true));
  close.onclick=()=>{if(!busy)panel.close()};
  panel.addEventListener('cancel',event=>{if(busy)event.preventDefault()});
  auto.onchange=()=>{
    try{window.localStorage.setItem(pref,auto.checked?'on':'off')}
    catch{state.textContent='无法记住自动检查设置。'}
  };
  async function checkNow(automatic){
    if(!supported||busy)return;
    busy=true; version=null; sync(); state.textContent='正在检查更新…';
    try{
      version=await invoke('check_update');
      state.textContent=version?`发现新版本 ${version}。安装前会校验更新包的签名。`:'已是最新版本。';
      if(version&&automatic)show();
    }catch(e){state.textContent='检查更新失败：'+e}
    finally{busy=false;sync()}
  }
  check.onclick=()=>checkNow(false);
  install.onclick=async()=>{
    if(busy||!version)return;
    busy=true;sync();state.textContent='正在下载并校验签名，完成后将重启…';
    try{await invoke('install_update',{version})}
    catch(e){version=null;state.textContent='未能安装更新，请重新检查：'+e;busy=false;sync()}
  };
  invoke('app_info').then(info=>{
    $('appVersion').textContent=`版本 ${info.version}`;
    supported=info.update_supported;
    $('autoUpdateRow').hidden=check.hidden=!supported;
    if(!supported){state.textContent='此安装包未启用应用内更新。';return}
    state.textContent='可手动检查；启用自动检查后，仅在启动时访问 GitHub。';
    try{auto.checked=window.localStorage.getItem(pref)==='on'}catch{/* Opt-out by default. */}
    if(auto.checked)return checkNow(true);
  }).catch(e=>{state.textContent='无法读取更新设置：'+e});
})();
