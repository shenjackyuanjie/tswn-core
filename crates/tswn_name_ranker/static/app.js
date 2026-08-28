const $=id=>document.getElementById(id);
async function json(url,options){const r=await fetch(url,options);const body=await r.json().catch(()=>({}));if(!r.ok)throw new Error(body.error||`${r.status}`);return body}
async function post(url,data){return json(url,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(data)})}
function show(id,value){$(id).textContent=typeof value==="string"?value:JSON.stringify(value,null,2)}
$('add').onclick=async()=>{try{show('addOut',await post('/api/names/add',{text:$('names').value}))}catch(e){show('addOut',e.message)}};
async function loadExpansions(){try{const x=await json('/api/expansions');$('expansionList').innerHTML=x.names.length?`<p>当前扩容：</p><pre>${esc(x.names.join('\n'))}</pre>`:'<p class="muted">当前没有扩容号</p>'}catch(e){$('expansionList').textContent=e.message}}
async function setExpansions(add){try{show('expansionOut',await post(add?'/api/expansions/add':'/api/expansions/remove',{text:$('expansionNames').value}));loadExpansions()}catch(e){show('expansionOut',e.message)}}
$('addExpansion').onclick=()=>setExpansions(true);
$('removeExpansion').onclick=()=>setExpansions(false);
$('targetFile').onchange=async e=>{const f=e.target.files[0];if(f)$('targets').value=await f.text()};
$('import').onclick=async()=>{try{show('targetOut',await post('/api/targets/import',{text:$('targets').value}))}catch(e){show('targetOut',e.message)}};
$('run').onclick=async()=>{try{await post('/api/recompute',{outer_workers:Number($('workers').value),skip_archived:$('skipArchived').checked});poll()}catch(e){$('progress').textContent=e.message}};
$('refresh').onclick=loadResults;
$('export').onclick=()=>{window.location.href='/api/results/export'};
async function loadResults(){
  try{
    const rows=await json('/api/results');
    if(!rows.length){$('results').innerHTML='<p class="muted">暂无结果</p>';return}
    $('results').innerHTML=`<table class="rank-table folded-rank-table"><thead><tr><th>Rank</th><th>Score</th><th>Text-Type</th><th><span class="fold-icon"></span>Name</th></tr></thead><tbody>${rows.map(renderResultGroup).join('')}</tbody></table>`;
    document.querySelectorAll('.result-parent.has-partners').forEach(row=>{
      row.addEventListener('click',()=>{
        const next=row.dataset.expanded!=='true';
        row.dataset.expanded=String(next);
        row.classList.toggle('expanded',next);
        const icon=row.querySelector('.fold-icon');
        if(icon)icon.textContent=next?'▾':'▸';
        document.querySelectorAll(`.partner-row[data-parent-rank="${row.dataset.rank}"]`).forEach(child=>child.hidden=!next);
      });
    });
  }catch(e){$('results').textContent=e.message}
}
function renderResultGroup(row){
  const partners=Array.isArray(row.partners)?row.partners:[];
  const hasPartners=partners.length>0;
  const parent=`<tr class="result-parent ${hasPartners?'has-partners':''}" data-rank="${Number(row.rank)}" data-expanded="false"><td>${Number(row.rank)}</td><td>${Number(row.score).toFixed(6)}</td><td>${esc(row.text_type)}</td><td class="name-cell"><span class="fold-icon">${hasPartners?'▸':''}</span>${esc(row.name)}</td></tr>`;
  const children=partners.map(partner=>`<tr class="partner-row" data-parent-rank="${Number(row.rank)}" hidden><td>${Number(partner.rank)}</td><td>${Number(partner.win_rate).toFixed(6)}</td><td>${esc(partner.text_type)}</td><td class="name-cell partner-name"><span class="partner-marker">↳</span>${esc(partner.name)}</td></tr>`).join('');
  return parent+children;
}
function esc(x){const d=document.createElement('div');d.textContent=x;return d.innerHTML}
let timer;async function poll(){clearTimeout(timer);try{const s=await json('/api/status');const pct=s.pair_total?Math.min(100,s.pair_done*100/s.pair_total):0;$('statusBadge').className=`badge ${s.state}`;$('statusBadge').textContent=s.state;$('progressSummary').textContent=`组合 ${s.pair_done}/${s.pair_total} · ${pct.toFixed(2)}%`;$('progressBar').style.width=`${pct}%`;$('progress').textContent=`拟合步骤：${s.iteration}\n${s.message}`;if(s.state==='running')timer=setTimeout(poll,1000);else loadResults()}catch(e){$('statusBadge').className='badge error';$('statusBadge').textContent='error';$('progress').textContent=e.message}}
poll();loadResults();loadExpansions();
