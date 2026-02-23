--[[
  view.lua — Force-directed dependency graph HTML generator

  Combines analyze (metrics/percepts) and parse (edges) data into
  a self-contained HTML file with an interactive module-level graph.

  Visual encoding:
    circle size   = total lines of code in module
    circle color  = architectural layer (app/domain/infra/port/cli)
    border ring   = complexity hue (green=low → red=high)
    ring width    = max cyclomatic complexity
    edges         = import dependencies
]]

local M = {}

local LAYER_META = {
  app    = { color = "#58a6ff", label = "Application" },
  domain = { color = "#7ee787", label = "Domain" },
  infra  = { color = "#f0883e", label = "Infrastructure" },
  port   = { color = "#d2a8ff", label = "Port" },
  cli    = { color = "#79c0ff", label = "CLI" },
}

local function detect_group(path)
  local first = path:match("^([^/]+)")
  if first and LAYER_META[first] then return first end
  return "other"
end

local function strip_src(p)
  return p:match("^src/(.+)") or p
end

--- Build combined JSON data for the view from eval result + edges.
function M.build_data(eval_result, ast_edges, bindings)
  -- 1. Aggregate entries by file
  local file_map = {}
  for _, entry in ipairs(eval_result.entries) do
    local n = entry.node
    local file = strip_src(n.file)
    if not file_map[file] then
      file_map[file] = {
        id = file,
        group = detect_group(file),
        lines = 0,
        entries = 0,
        max_cyclo = 0,
        hue_sum = 0,
        hue_count = 0,
      }
    end
    local f = file_map[file]
    f.entries = f.entries + 1
    f.lines = f.lines + (n.lines or 0)
    f.max_cyclo = math.max(f.max_cyclo, n.cyclomatic or 0)
    if entry.percept and entry.percept.hue then
      f.hue_sum = f.hue_sum + entry.percept.hue
      f.hue_count = f.hue_count + 1
    end
  end

  local nodes = {}
  for _, f in pairs(file_map) do
    f.avg_hue = f.hue_count > 0 and (f.hue_sum / f.hue_count) or 120
    f.hue_sum = nil
    f.hue_count = nil
    nodes[#nodes + 1] = f
  end

  -- 2. Deduplicate edges
  local edge_map = {}
  for _, e in ipairs(ast_edges) do
    local from = strip_src(e.from_file)
    local to   = strip_src(e.to_file)
    if to ~= "" and to ~= "crate" then
      local key = from .. "|" .. to
      if not edge_map[key] then
        edge_map[key] = { source = from, target = to, symbols = {} }
      end
      edge_map[key].symbols[#edge_map[key].symbols + 1] = e.symbol
    end
  end
  local edges = {}
  for _, e in pairs(edge_map) do
    e.label = #e.symbols <= 3
      and table.concat(e.symbols, ", ")
      or string.format("%d symbols", #e.symbols)
    edges[#edges + 1] = e
  end

  -- 3. Ensure all edge endpoints exist as nodes
  local node_ids = {}
  for _, n in ipairs(nodes) do node_ids[n.id] = true end
  for _, e in ipairs(edges) do
    for _, id in ipairs({e.source, e.target}) do
      if not node_ids[id] then
        node_ids[id] = true
        nodes[#nodes + 1] = {
          id = id, group = detect_group(id),
          lines = 0, entries = 0, max_cyclo = 0, avg_hue = 120,
        }
      end
    end
  end

  -- 4. Build groups
  local group_set = {}
  for _, n in ipairs(nodes) do group_set[n.group] = true end
  local groups = {}
  for g in pairs(group_set) do
    local meta = LAYER_META[g] or { color = "#8b949e", label = g }
    groups[#groups + 1] = { name = g, color = meta.color, label = meta.label }
  end

  -- 5. Bindings info
  local binding_info = {}
  for _, b in ipairs(bindings) do
    binding_info[#binding_info + 1] = { index = b.index.name, percept = b.percept.name }
  end

  return __rustlib.json.encode({
    nodes = nodes,
    edges = edges,
    groups = groups,
    bindings = binding_info,
    total = eval_result.total,
  })
end

--- Generate complete self-contained HTML with embedded data.
function M.generate_html(data_json)
  return HTML_BEFORE .. data_json .. HTML_AFTER
end

-- ================================================================
-- HTML template (split around data injection point)
-- ================================================================

HTML_BEFORE = [=[<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>codedash &mdash; module map</title>
<style>
:root{--bg:#0d1117;--bg2:#161b22;--bg3:#21262d;--fg:#c9d1d9;--fg2:#8b949e;--border:#30363d;--accent:#58a6ff}
*{margin:0;padding:0;box-sizing:border-box}
body{background:var(--bg);font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif;color:var(--fg);overflow:hidden}
#header{position:fixed;top:0;left:0;right:0;height:48px;background:var(--bg2);border-bottom:1px solid var(--border);display:flex;align-items:center;padding:0 20px;z-index:10;gap:16px}
#header h1{font-size:15px;font-weight:600;white-space:nowrap}
#search{background:var(--bg);border:1px solid var(--border);color:var(--fg);padding:6px 12px;border-radius:6px;width:200px;font-size:13px;outline:none}
#search:focus{border-color:var(--accent)}
.stats{margin-left:auto;font-size:13px;color:var(--fg2);display:flex;gap:16px;white-space:nowrap}
.stats .v{color:var(--fg);font-weight:600}
.chip{display:inline-flex;align-items:center;gap:4px;background:var(--bg3);border:1px solid var(--border);border-radius:12px;padding:2px 10px;font-size:11px;color:var(--fg2);margin-left:4px}
.chip .p{color:var(--accent);font-weight:600}
svg{position:fixed;top:48px;left:0;right:0;bottom:0;width:100%;height:calc(100vh - 48px)}
#legend{position:fixed;bottom:20px;left:20px;background:var(--bg2);border:1px solid var(--border);border-radius:8px;padding:12px 16px;font-size:12px;z-index:10;min-width:170px}
.lg-t{font-weight:600;margin-bottom:8px;color:var(--fg)}
.lg-i{display:flex;align-items:center;gap:8px;margin:4px 0;color:var(--fg2);padding:2px 4px;border-radius:4px;transition:background .15s,opacity .15s}
.lg-i:hover{background:var(--bg3)}
.lg-d{width:12px;height:12px;border-radius:50%;flex-shrink:0}
.lg-m{margin-top:10px;padding-top:8px;border-top:1px solid var(--border);color:var(--fg2);line-height:1.6}
#tip{display:none;position:fixed;background:var(--bg2);border:1px solid var(--border);border-radius:8px;padding:12px 16px;font-size:13px;z-index:100;box-shadow:0 4px 16px rgba(0,0,0,.5);max-width:360px;pointer-events:none}
#tip.show{display:block}
#tip h3{font-size:14px;margin:0 0 6px;color:var(--fg)}
.tr{display:flex;justify-content:space-between;gap:16px;line-height:1.7}
.tl{color:var(--fg2)}.tv{color:var(--fg);font-weight:500;font-variant-numeric:tabular-nums}
.ts{margin-top:6px;padding-top:6px;border-top:1px solid var(--border);color:var(--accent);font-size:12px;font-weight:600}
.edge{stroke:var(--border);stroke-width:1.2;fill:none;opacity:.45;transition:opacity .15s,stroke .15s,stroke-width .15s}
.edge.hl{stroke:var(--accent);opacity:.85;stroke-width:2}
.edge.dim{opacity:.06}
.node-g{cursor:grab}.node-g:active{cursor:grabbing}
.node-g .ring{transition:opacity .15s}
.node-g .body{transition:opacity .15s}
.node-g.dim .ring,.node-g.dim .body{opacity:.1}
.node-g.dim text{opacity:.1}
.node-g text{fill:var(--fg);pointer-events:none;text-anchor:middle;dominant-baseline:central;text-shadow:0 1px 3px rgba(0,0,0,.9)}
</style></head><body>
<div id="header">
<h1>codedash</h1>
<input id="search" type="text" placeholder="Search modules...">
<div id="bindings"></div>
<div class="stats">
<span><span class="v" id="s-mod">0</span> modules</span>
<span><span class="v" id="s-edge">0</span> edges</span>
<span><span class="v" id="s-total">0</span> code units</span>
</div>
</div>
<svg id="canvas">
<defs>
<marker id="arr" viewBox="0 0 10 6" refX="9" refY="3" markerWidth="8" markerHeight="5" orient="auto"><path d="M0,0L10,3L0,6z" fill="#30363d"/></marker>
<marker id="arr-hl" viewBox="0 0 10 6" refX="9" refY="3" markerWidth="8" markerHeight="5" orient="auto"><path d="M0,0L10,3L0,6z" fill="#58a6ff"/></marker>
</defs>
</svg>
<div id="legend"></div>
<div id="tip"></div>
<script>const VIEW_DATA=]=]

HTML_AFTER = [=[;
(function(){
'use strict';
const D=VIEW_DATA;
const GC={};for(const g of D.groups)GC[g.name]=g.color;

// ── Nodes ──
const W=window.innerWidth,H=window.innerHeight-48;
const nodes=D.nodes.map(n=>({
  ...n,
  x:W/2+(Math.random()-.5)*Math.min(W,600),
  y:H/2+(Math.random()-.5)*Math.min(H,400),
  vx:0,vy:0,
  r:Math.max(22,Math.min(72,12+Math.sqrt(n.lines)*1.8)),
  color:GC[n.group]||'#8b949e',
  fixed:false
}));
const NM={};for(const n of nodes)NM[n.id]=n;
const edges=D.edges.filter(e=>NM[e.source]&&NM[e.target]);

// ── Stats ──
document.getElementById('s-mod').textContent=nodes.length;
document.getElementById('s-edge').textContent=edges.length;
document.getElementById('s-total').textContent=D.total||0;

// ── Bindings chips ──
const bEl=document.getElementById('bindings');
for(const b of D.bindings||[]){
  const c=document.createElement('span');c.className='chip';
  c.innerHTML='<span class="p">'+b.percept+'</span>&larr;'+b.index;
  bEl.appendChild(c);
}

// ── Legend ──
const leg=document.getElementById('legend');
let lh='<div class="lg-t">Layers</div>';
D.groups.sort((a,b)=>a.label.localeCompare(b.label));
for(const g of D.groups)lh+='<div class="lg-i" data-layer="'+g.name+'" style="cursor:pointer"><div class="lg-d" style="background:'+g.color+'"></div>'+g.label+'</div>';
lh+='<div class="lg-m">Size = lines of code<br>Ring = complexity (hue)<br><span style="opacity:.6">Click layer to filter</span></div>';
leg.innerHTML=lh;

// Layer filter
let activeLayer=null,lockedLayer=null;
function applyLayerFilter(layer){
  activeLayer=layer;
  for(const li of leg.querySelectorAll('.lg-i'))li.style.opacity=(!layer||li.dataset.layer===layer)?1:0.35;
  for(const ne of nEls)ne.el.classList.toggle('dim',!!layer&&ne.d.group!==layer);
  for(const ee of eEls){
    const sn=NM[ee.d.source],tn=NM[ee.d.target];
    const hit=!layer||(sn&&sn.group===layer)||(tn&&tn.group===layer);
    ee.el.classList.toggle('dim',!hit);
  }
}
for(const li of leg.querySelectorAll('.lg-i')){
  li.addEventListener('mouseenter',function(){
    if(!lockedLayer)applyLayerFilter(this.dataset.layer);
  });
  li.addEventListener('mouseleave',function(){
    if(!lockedLayer)applyLayerFilter(null);
  });
  li.addEventListener('click',function(){
    const layer=this.dataset.layer;
    if(lockedLayer===layer){lockedLayer=null;applyLayerFilter(null);}
    else{lockedLayer=layer;applyLayerFilter(layer);}
    for(const li2 of leg.querySelectorAll('.lg-i'))li2.style.fontWeight=(lockedLayer===li2.dataset.layer)?'600':'';
  });
}

// ── SVG ──
const svg=document.getElementById('canvas');
const NS='http://www.w3.org/2000/svg';
let sc=1,tx=0,ty=0;
const gM=document.createElementNS(NS,'g');
svg.appendChild(gM);
function updTx(){gM.setAttribute('transform','translate('+tx+','+ty+') scale('+sc+')');}

// Edge elements
const eEls=edges.map(ed=>{
  const p=document.createElementNS(NS,'path');
  p.classList.add('edge');
  p.setAttribute('marker-end','url(#arr)');
  p.dataset.s=ed.source;p.dataset.t=ed.target;
  gM.appendChild(p);
  return{d:ed,el:p};
});

// Node elements
const nEls=nodes.map(n=>{
  const g=document.createElementNS(NS,'g');
  g.classList.add('node-g');g.dataset.id=n.id;

  // Complexity ring
  const rw=Math.max(2,Math.min(8,n.max_cyclo*0.5));
  const ring=document.createElementNS(NS,'circle');
  ring.classList.add('ring');
  ring.setAttribute('r',n.r+rw/2);
  const hu=n.avg_hue!=null?n.avg_hue:120;
  ring.setAttribute('stroke','hsl('+hu+',70%,50%)');
  ring.setAttribute('stroke-width',rw);
  ring.setAttribute('fill','none');
  g.appendChild(ring);

  // Body
  const circ=document.createElementNS(NS,'circle');
  circ.classList.add('body');
  circ.setAttribute('r',n.r);
  circ.setAttribute('fill',n.color);
  circ.setAttribute('opacity',0.85);
  g.appendChild(circ);

  // Module name
  const label=shortN(n.id);
  const fs=Math.max(10,Math.min(15,n.r*0.38));
  const parts=label.split('/');
  if(n.r<30){
    // Small circle: label outside (below)
    const ct=document.createElementNS(NS,'text');
    ct.textContent=label;
    ct.setAttribute('dy',n.r+14);
    ct.setAttribute('font-size',11);
    ct.setAttribute('fill','var(--fg)');ct.setAttribute('font-weight','500');
    g.appendChild(ct);
  }else if(parts.length>=2){
    // Two-line label inside circle
    const t1=document.createElementNS(NS,'text');
    t1.textContent=parts.slice(0,-1).join('/');
    t1.setAttribute('dy',-fs*0.4);t1.setAttribute('font-size',fs*0.8);
    t1.setAttribute('fill','#fff');t1.setAttribute('opacity',0.7);
    g.appendChild(t1);
    const t2=document.createElementNS(NS,'text');
    t2.textContent=parts[parts.length-1];
    t2.setAttribute('dy',fs*0.75);t2.setAttribute('font-size',fs);
    t2.setAttribute('fill','#fff');t2.setAttribute('font-weight','600');
    g.appendChild(t2);
  }else{
    const ct=document.createElementNS(NS,'text');
    ct.textContent=label;
    ct.setAttribute('font-size',fs);
    ct.setAttribute('fill','#fff');ct.setAttribute('font-weight','600');
    g.appendChild(ct);
  }

  gM.appendChild(g);
  return{d:n,el:g};
});

function shortN(id){
  // Strip /mod suffix (directory module root → parent name)
  let s=id.replace(/\/mod$/,'');
  const p=s.split('/');
  return p.length<=2?s:p.slice(-2).join('/');
}

// ── Force simulation ──
let alpha=1;
const AD=0.012,VD=0.55;

function tick(){
  const N=nodes.length;
  // Charge repulsion
  for(let i=0;i<N;i++){for(let j=i+1;j<N;j++){
    const a=nodes[i],b=nodes[j];
    let dx=b.x-a.x||.1,dy=b.y-a.y||.1;
    const d2=dx*dx+dy*dy,d=Math.sqrt(d2);
    const str=a.group===b.group?-800:-1400;
    const f=str/d2*alpha;
    const fx=dx/d*f,fy=dy/d*f;
    if(!a.fixed){a.vx-=fx;a.vy-=fy;}
    if(!b.fixed){b.vx+=fx;b.vy+=fy;}
  }}

  // Link attraction
  for(const ed of edges){
    const a=NM[ed.source],b=NM[ed.target];
    if(!a||!b)continue;
    let dx=b.x-a.x||.1,dy=b.y-a.y||.1;
    const d=Math.sqrt(dx*dx+dy*dy);
    const f=(d-200)*0.004*alpha;
    const fx=dx/d*f,fy=dy/d*f;
    if(!a.fixed){a.vx+=fx;a.vy+=fy;}
    if(!b.fixed){b.vx-=fx;b.vy-=fy;}
  }

  // Center gravity
  for(const n of nodes){
    if(n.fixed)continue;
    n.vx+=(W/2-n.x)*0.0008*alpha;
    n.vy+=(H/2-n.y)*0.0008*alpha;
  }

  // Group clustering
  const gc={},gn={};
  for(const n of nodes){
    if(!gc[n.group]){gc[n.group]={x:0,y:0};gn[n.group]=0;}
    gc[n.group].x+=n.x;gc[n.group].y+=n.y;gn[n.group]++;
  }
  for(const g in gc){gc[g].x/=gn[g];gc[g].y/=gn[g];}
  for(const n of nodes){
    if(n.fixed)continue;
    const c=gc[n.group];
    n.vx+=(c.x-n.x)*0.008*alpha;
    n.vy+=(c.y-n.y)*0.008*alpha;
  }

  // Collision
  for(let i=0;i<N;i++){for(let j=i+1;j<N;j++){
    const a=nodes[i],b=nodes[j];
    let dx=b.x-a.x||.1,dy=b.y-a.y||.1;
    const d=Math.sqrt(dx*dx+dy*dy);
    const mn=a.r+b.r+30;
    if(d<mn){
      const push=(mn-d)/d*0.5;
      if(!a.fixed){a.vx-=dx*push;a.vy-=dy*push;}
      if(!b.fixed){b.vx+=dx*push;b.vy+=dy*push;}
    }
  }}

  // Apply
  for(const n of nodes){
    if(n.fixed)continue;
    n.vx*=VD;n.vy*=VD;
    n.x+=n.vx;n.y+=n.vy;
  }
  alpha=Math.max(0.001,alpha*(1-AD));
}

// ── Render ──
function render(){
  for(const{d:ed,el}of eEls){
    const s=NM[ed.source],t=NM[ed.target];
    if(!s||!t)continue;
    let dx=t.x-s.x,dy=t.y-s.y;
    const d=Math.sqrt(dx*dx+dy*dy)||1;
    const sx=s.x+dx/d*s.r,sy=s.y+dy/d*s.r;
    const ex=t.x-dx/d*(t.r+6),ey=t.y-dy/d*(t.r+6);
    el.setAttribute('d','M'+sx+','+sy+' L'+ex+','+ey);
  }
  for(const{d:n,el}of nEls){
    el.setAttribute('transform','translate('+n.x+','+n.y+')');
  }
}

let running=true;
function anim(){
  if(!running)return;
  tick();render();
  if(alpha>0.005)requestAnimationFrame(anim);
  else{running=false;render();}
}
anim();

// ── Drag ──
let dragN=null,dOff={x:0,y:0};
for(const{d:n,el}of nEls){
  el.addEventListener('mousedown',function(ev){
    ev.stopPropagation();
    dragN=n;n.fixed=true;
    const r=svg.getBoundingClientRect();
    dOff.x=n.x-(ev.clientX-r.left-tx)/sc;
    dOff.y=n.y-(ev.clientY-r.top-ty)/sc;
    alpha=Math.max(alpha,0.3);running=true;anim();
  });
}
window.addEventListener('mousemove',function(ev){
  if(!dragN)return;
  const r=svg.getBoundingClientRect();
  dragN.x=(ev.clientX-r.left-tx)/sc+dOff.x;
  dragN.y=(ev.clientY-r.top-ty)/sc+dOff.y;
  render();
});
window.addEventListener('mouseup',function(){if(dragN){dragN.fixed=false;dragN=null;}});

// ── Zoom ──
svg.addEventListener('wheel',function(ev){
  ev.preventDefault();
  const d=ev.deltaY>0?0.92:1.08;
  const ns=Math.max(0.15,Math.min(4,sc*d));
  const r=svg.getBoundingClientRect();
  const mx=ev.clientX-r.left,my=ev.clientY-r.top;
  tx=mx-(mx-tx)*(ns/sc);
  ty=my-(my-ty)*(ns/sc);
  sc=ns;updTx();
},{passive:false});

// ── Pan ──
let panning=false,pS={x:0,y:0};
svg.addEventListener('mousedown',function(ev){
  if(!dragN&&(ev.target===svg||ev.target.closest('.edge'))){
    panning=true;pS={x:ev.clientX-tx,y:ev.clientY-ty};
    svg.style.cursor='grab';
  }
});
window.addEventListener('mousemove',function(ev){
  if(panning){tx=ev.clientX-pS.x;ty=ev.clientY-pS.y;updTx();}
});
window.addEventListener('mouseup',function(){panning=false;svg.style.cursor='';});

// ── Hover highlight ──
let hovId=null;
for(const{d:n,el}of nEls){
  el.addEventListener('mouseenter',function(ev){
    hovId=n.id;
    const conn=new Set([n.id]);
    for(const ed of edges){
      if(ed.source===n.id)conn.add(ed.target);
      if(ed.target===n.id)conn.add(ed.source);
    }
    for(const ne of nEls)ne.el.classList.toggle('dim',!conn.has(ne.d.id));
    for(const ee of eEls){
      const hit=ee.d.source===n.id||ee.d.target===n.id;
      ee.el.classList.toggle('hl',hit);
      ee.el.classList.toggle('dim',!hit);
      ee.el.setAttribute('marker-end',hit?'url(#arr-hl)':'url(#arr)');
    }
    showTip(ev,n);
  });
  el.addEventListener('mouseleave',function(){
    hovId=null;
    for(const ne of nEls)ne.el.classList.remove('dim');
    for(const ee of eEls){
      ee.el.classList.remove('hl','dim');
      ee.el.setAttribute('marker-end','url(#arr)');
    }
    hideTip();
    // Re-apply layer filter if locked
    if(lockedLayer)applyLayerFilter(lockedLayer);
  });
  el.addEventListener('mousemove',function(ev){if(hovId)moveTip(ev);});
}

// ── Tooltip ──
const tip=document.getElementById('tip');
function showTip(ev,n){
  let h='<h3>'+n.id+'</h3>';
  const rows=[
    ['Layer',(D.groups.find(function(g){return g.name===n.group})||{}).label||n.group],
    ['Code units',n.entries],
    ['Total lines',n.lines],
    ['Max cyclomatic',n.max_cyclo],
    ['Avg complexity hue',n.avg_hue!=null?n.avg_hue.toFixed(1):'—'],
  ];
  for(const r of rows)h+='<div class="tr"><span class="tl">'+r[0]+'</span><span class="tv">'+r[1]+'</span></div>';
  const deps=edges.filter(function(ed){return ed.source===n.id}).map(function(ed){return ed.target});
  const users=edges.filter(function(ed){return ed.target===n.id}).map(function(ed){return ed.source});
  if(deps.length)h+='<div class="ts">Depends on ('+deps.length+')</div><div class="tr"><span class="tl">'+deps.join(', ')+'</span></div>';
  if(users.length)h+='<div class="ts">Used by ('+users.length+')</div><div class="tr"><span class="tl">'+users.join(', ')+'</span></div>';
  // Edge symbols on hover
  const outEdges=edges.filter(function(ed){return ed.source===n.id});
  const inEdges=edges.filter(function(ed){return ed.target===n.id});
  if(outEdges.length){
    h+='<div class="ts">Imports</div>';
    for(const ed of outEdges)h+='<div class="tr"><span class="tl">'+ed.target+'</span><span class="tv">'+ed.label+'</span></div>';
  }
  tip.innerHTML=h;
  tip.classList.add('show');moveTip(ev);
}
function moveTip(ev){
  tip.style.left=Math.min(ev.clientX+24,window.innerWidth-400)+'px';
  tip.style.top=Math.min(ev.clientY-tip.offsetHeight-16,window.innerHeight-350)+'px';
  if(parseInt(tip.style.top)<52)tip.style.top=(ev.clientY+24)+'px';
}
function hideTip(){tip.classList.remove('show');}

// ── Search ──
document.getElementById('search').addEventListener('input',function(ev){
  const q=ev.target.value.toLowerCase();
  if(!q){
    for(const ne of nEls)ne.el.classList.remove('dim');
    for(const ee of eEls)ee.el.classList.remove('dim');
    if(lockedLayer)applyLayerFilter(lockedLayer);
    return;
  }
  for(const ne of nEls)ne.el.classList.toggle('dim',!ne.d.id.toLowerCase().includes(q));
  for(const ee of eEls){
    const sm=ee.d.source.toLowerCase().includes(q);
    const tm=ee.d.target.toLowerCase().includes(q);
    ee.el.classList.toggle('dim',!sm&&!tm);
  }
});

// ── Resize ──
window.addEventListener('resize',function(){render();});
})();
</script></body></html>]=]

return M
