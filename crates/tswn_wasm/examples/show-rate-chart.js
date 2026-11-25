import { escapeHtml } from "./show-utils.js";

/** 仅从已缓存的逐帧胜率构造图表点；第一组按输入顺序映射到运行队伍。 */
export function rateChartPoints(battle, plan) {
  const teams = [...new Set(battle.players.map(p => p.team_index))].sort((a,b) => a-b);
  const first = battle.players.find(p => p.input_team_index === 0) ?? battle.players[0];
  const index = teams.indexOf(first?.team_index);
  return (plan?.frames ?? []).filter(p => p.frame.state_rates).map(p => ({
    frameIndex: p.frameIndex, cursor: p.end, round: p.frame.round_index + 1,
    percent: Math.max(0, Math.min(100, p.frame.state_rates.counts[index] / p.frame.state_rates.rounds * 100)),
    secondPercent: p.frame.state_rates.counts[1-index] / p.frame.state_rates.rounds * 100,
  }));
}

/** 保留滑块 DOM 与手动浏览位置，播放更新时不打断拖动。 */
export function createRateChart(host, onSeek) {
  host.innerHTML = '<div class="rate-chart-title">第一组胜率走势</div><div class="rate-chart-plot"></div><div class="rate-chart-tip" role="status">悬停查看数值，点击节点跳转回放</div><div class="rate-chart-nav"><input type="range" min="0" max="0" value="0" step="1" aria-label="拖动查看历史胜率"><button type="button">跟随回放</button></div><div class="rate-chart-window"></div>';
  const plot = host.querySelector('.rate-chart-plot'), slider = host.querySelector('input');
  const tip = host.querySelector('.rate-chart-tip'), caption = host.querySelector('.rate-chart-window');
  let battle = null, points = [], cursor = 0, manualStart = null, icons = [], lastSignature = '';
  const label = p => '第 ' + p.round + ' 回合 · 第一组 ' + p.percent.toFixed(2) + '% · 第二组 ' + p.secondPercent.toFixed(2) + '%';
  function draw() {
    const current = points.findLastIndex(p => p.cursor <= cursor);
    const maxStart = Math.max(0, points.length - 10);
    const start = manualStart == null ? Math.min(maxStart, Math.max(0, current - 9)) : Math.min(maxStart, manualStart);
    slider.max = String(maxStart); slider.value = String(start); slider.disabled = maxStart === 0;
    const shown = points.slice(start, start + 10);
    const signature = JSON.stringify([shown, current, start, icons, manualStart == null]);
    if (signature === lastSignature) return;
    lastSignature = signature;
    tip.textContent = current >= 0 ? label(points[current]) : "悬停查看数值，点击节点跳转回放";
    const x = i => 67 + i * (258 / Math.max(1, shown.length-1));
    const y = p => 26 + (100-p.percent) * 1.12;
    const path = shown.map((p,i) => x(i) + ',' + y(p)).join(' ');
    const axisIcon = (icon,top,text) => '<image href="' + escapeHtml(icon ?? '') + '" x="0" y="' + top + '" width="20" height="20"/><text x="24" y="' + (top+14) + '" class="rate-chart-label">' + text + '</text>';
    const nodes = shown.map((p,i) => '<g class="rate-chart-node' + (start+i === current ? ' is-current' : '') + '" data-rate-index="' + (start+i) + '" role="button" tabindex="0" aria-label="' + label(p) + '"><title>' + label(p) + '</title><circle class="rate-chart-hit" cx="' + x(i) + '" cy="' + y(p) + '" r="11"/><circle class="rate-chart-dot" cx="' + x(i) + '" cy="' + y(p) + '" r="' + (start+i === current ? 5 : 3.5) + '"/><text x="' + x(i) + '" y="158" text-anchor="middle" class="rate-chart-label">' + p.round + '</text></g>').join('');
    plot.innerHTML = '<svg viewBox="0 0 340 168" role="group" aria-label="第一组胜率折线图，蓝线；纵轴为第一组胜率，横轴为回合">' + axisIcon(icons[0],16,'100%') + axisIcon(icons[1],128,'0%') + '<path class="rate-chart-grid" d="M65 26H330 M65 82H330 M65 138H330"/><text x="24" y="86" class="rate-chart-label">50%</text><polyline class="rate-chart-line" points="' + path + '"/>' + nodes + '</svg>';
    caption.textContent = shown.length ? '第 ' + shown[0].round + '–' + shown.at(-1).round + ' 回合 · ' + (manualStart == null ? '跟随回放' : '浏览历史') : '等待首个回合的胜率';
  }
  slider.addEventListener('input', () => { manualStart = Number(slider.value); draw(); });
  host.querySelector('button').addEventListener('click', () => { manualStart = null; draw(); });
  const pointFor = target => { const node = target.closest('[data-rate-index]'); return node ? points[Number(node.dataset.rateIndex)] : null; };
  plot.addEventListener('pointerover', e => { const p = pointFor(e.target); if(p) tip.textContent = label(p); });
  plot.addEventListener('focusin', e => { const p = pointFor(e.target); if(p) tip.textContent = label(p); });
  plot.addEventListener('pointerleave', () => { const p = points.findLast(p => p.cursor <= cursor); tip.textContent = p ? label(p) : '悬停查看数值，点击节点跳转回放'; });
  plot.addEventListener('click', e => { const p = pointFor(e.target); if(p) onSeek(p.cursor); });
  plot.addEventListener('keydown', e => { if(e.key === 'Enter' || e.key === ' ') { const p = pointFor(e.target); if(p) { e.preventDefault(); onSeek(p.cursor); } } });
  // 导出全部缓存点，独立绘制整场横轴；不修改屏幕的十回合窗口。
  async function downloadPng(footer = []) {
    if (!points.length) throw new Error('暂无可导出的胜率数据');
    const snapshot = points.slice(), teamIcons = icons.slice();
    const width = Math.min(15000, Math.max(340, 82 + (snapshot.length-1) * (258/9)));
    const x = i => 67 + i * ((width-82) / Math.max(1,snapshot.length-1));
    const y = p => 26 + (100-p.percent) * 1.12;
    const style = getComputedStyle(host), ink = style.getPropertyValue('--ink').trim(), grid = style.getPropertyValue('--border').trim();
    const surface = style.getPropertyValue('--paper').trim();
    const labelWidth = Math.max(24,String(snapshot.at(-1).round).length*7+8);
    const labelEvery = Math.max(1,Math.ceil((snapshot.length-1) / Math.max(1,Math.floor((width-82)/labelWidth))));
    const axisIcons = teamIcons.map((icon,i)=>'<image href="'+escapeHtml(icon ?? '')+'" x="0" y="'+(i ? 128 : 16)+'" width="20" height="20"/>').join('');
    const nodes = snapshot.map((p,i)=>'<circle cx="'+x(i)+'" cy="'+y(p)+'" r="3.5" fill="#2484e4" stroke="'+surface+'" stroke-width="1.5"/>' + (i%labelEvery===0 || i===snapshot.length-1 ? '<text x="'+x(i)+'" y="158" text-anchor="middle">'+p.round+'</text>' : '')).join('');
    // 完整输入名逐行排版，图标跟随名字；长名字自动折行，队间空行不丢失。
    const fontFamily = getComputedStyle(host).fontFamily;
    const measure = document.createElement('canvas').getContext('2d'); measure.font = '14px '+fontFamily;
    let footerY = 190;
    const footerSvg = footer.map(row=>{
      if(!row.text) { footerY += 24; return ''; }
      const lines = []; let line = '';
      for(const char of row.text) {
        if(line && measure.measureText(line+char).width > width-44) { lines.push(line); line=''; }
        line += char;
      }
      if(line) lines.push(line);
      const icon = row.icon ? '<image href="'+escapeHtml(row.icon)+'" x="6" y="'+(footerY-16)+'" width="20" height="20"/>' : '';
      const text = lines.map((line,index)=>'<text x="'+(row.icon ? 34 : 6)+'" y="'+(footerY+index*22)+'" style="font-size:14px">'+escapeHtml(line)+'</text>').join('');
      footerY += lines.length*22+4;
      return icon+text;
    }).join('');
    const height = Math.max(168,footerY+14);
    // 不生成 0%/100% 文本，保留 50% 与三条水平虚线、左侧队伍图标。
    const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="'+width+'" height="'+height+'" viewBox="0 0 '+width+' '+height+'"><style>text{font-family:'+escapeHtml(fontFamily)+';font-size:11px;fill:'+ink+'}</style><rect width="100%" height="100%" fill="'+surface+'"/>'+axisIcons+'<path d="M65 26H'+(width-10)+' M65 82H'+(width-10)+' M65 138H'+(width-10)+'" stroke="'+grid+'" fill="none" stroke-dasharray="3 3"/><text x="24" y="86">50%</text><polyline points="'+snapshot.map((p,i)=>x(i)+','+y(p)).join(' ')+'" stroke="#2484e4" stroke-width="2.5" stroke-linejoin="round" fill="none"/>'+nodes+footerSvg+'</svg>';
    const url = URL.createObjectURL(new Blob([svg],{type:'image/svg+xml;charset=utf-8'}));
    try {
      const image = new Image(); image.src = url; await image.decode();
      const canvas = document.createElement('canvas'); canvas.width = Math.ceil(width*2); canvas.height = Math.ceil(height*2);
      const ctx = canvas.getContext('2d'); if(!ctx) throw new Error('浏览器无法创建 PNG 画布');
      ctx.scale(2,2); ctx.drawImage(image,0,0,width,height);
      const blob = await new Promise(resolve=>canvas.toBlob(resolve,'image/png')); if(!blob) throw new Error('PNG 生成失败');
      const downloadUrl = URL.createObjectURL(blob), link = document.createElement('a');
      link.href = downloadUrl; link.download = 'namerena-winrate-'+new Date().toISOString().replace(/[:.]/g,'-')+'.png';
      document.body.append(link); link.click(); link.remove(); setTimeout(()=>URL.revokeObjectURL(downloadUrl),60000);
    } finally { URL.revokeObjectURL(url); }
  }
  return { downloadPng, update(nextBattle, plan, nextCursor, nextIcons) {
    if (battle !== nextBattle) { battle = nextBattle; manualStart = null; lastSignature = ''; tip.textContent = '悬停查看数值，点击节点跳转回放'; }
    points = rateChartPoints(battle, plan); cursor = nextCursor; icons = nextIcons; draw();
  }};
}
