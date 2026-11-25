/** 胜率浮窗只在标题栏拖动、右下角缩放；正文保持独立交互和滚动。 */
export function setupRateWindow(panel, onExport) {
  panel.innerHTML = '<div class="rate-window-header"><b>当前队伍胜率</b><span class="rate-window-actions"><button type="button" class="rate-window-reset" title="恢复左下角默认位置和大小">复位</button><button type="button" class="rate-export-png" disabled title="战斗结束后可导出整场胜率走势">导出 PNG</button></span></div><div class="rate-window-content"></div><div class="rate-window-resize" title="拖动调整窗口大小" aria-label="拖动调整窗口大小"></div>';
  const header = panel.querySelector('.rate-window-header'), handle = panel.querySelector('.rate-window-resize');
  let gesture = null;
  const clamp = (n,min,max) => Math.max(min, Math.min(max,n));
  function bounds() { return { width: Math.max(1, window.innerWidth-16), height: Math.max(1,window.innerHeight-16) }; }
  function apply(left,top,width,height) {
    const room = bounds();
    width = clamp(width,Math.min(280,room.width),room.width);
    height = clamp(height,Math.min(180,room.height),room.height);
    Object.assign(panel.style,{left:clamp(left,8,window.innerWidth-width-8)+'px',top:clamp(top,8,window.innerHeight-height-8)+'px',bottom:'auto',width:width+'px',height:height+'px',maxHeight:'calc(100vh - 16px)'});
  }
  function start(event,resize) {
    if(event.button !== 0 || event.target.closest('button')) return;
    const rect = panel.getBoundingClientRect();
    gesture = {id:event.pointerId,x:event.clientX,y:event.clientY,left:rect.left,top:rect.top,width:rect.width,height:rect.height,resize};
    event.currentTarget.setPointerCapture(event.pointerId);
    panel.classList.add('is-adjusting'); event.preventDefault();
  }
  function move(event) {
    if(!gesture || gesture.id !== event.pointerId) return;
    const g=gesture,dx=event.clientX-g.x,dy=event.clientY-g.y;
    if(g.resize) {
      const room=bounds();
      apply(g.left,g.top,Math.min(room.width,g.width+dx),Math.min(room.height,g.height+dy));
    } else apply(g.left+dx,g.top+dy,g.width,g.height);
  }
  function stop(event) { if(gesture?.id === event.pointerId) { gesture=null;panel.classList.remove('is-adjusting'); } }
  for(const [element,resize] of [[header,false],[handle,true]]) {
    element.addEventListener('pointerdown',e=>start(e,resize));
    element.addEventListener('pointermove',move);
    element.addEventListener('pointerup',stop);
    element.addEventListener('pointercancel',stop);
    element.addEventListener('lostpointercapture',stop);
  }
  header.querySelector('.rate-window-reset').addEventListener('click',()=>{panel.removeAttribute('style');});
  header.querySelector('.rate-export-png').addEventListener('click',()=>onExport());
  // 视口缩小时保留可见标题栏，避免浮窗留在屏幕外。
  window.addEventListener('resize',()=>{if(!panel.hidden && panel.style.top) {const r=panel.getBoundingClientRect();apply(r.left,r.top,r.width,r.height);}});
}
