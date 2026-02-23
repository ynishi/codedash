--[[
  view.lua — Force-directed dependency graph HTML generator

  Combines analyze (metrics/percepts) and parse (edges) data into
  a self-contained HTML file with an interactive module-level graph.

  Visual encoding:
    circle size   = total lines of code in module
    circle color  = domain (project area / crate)
    inner ring    = complexity hue (green=low → red=high)
    inner ring w  = max cyclomatic complexity
    outer ring    = git churn (amber, width ∝ change frequency)
    badge         = coverage status (shield=covered, warn=uncovered)
    edges         = import dependencies

  Data model:
    domain = vertical slice (project area, crate, feature zone)
    layer  = horizontal slice (architectural role: SDK, Runtime, etc.)
]]

local M = {}

-- Palette for auto-assigned domains (distinct, dark-theme friendly)
local PALETTE = {
  "#58a6ff", "#7ee787", "#f0883e", "#d2a8ff", "#79c0ff",
  "#f778ba", "#ffa657", "#56d4dd", "#d4976c", "#a5d6ff",
  "#bbeaa6", "#e2c08d", "#b694d8", "#ff9b8e", "#8bd5ca",
}

local function strip_src(p)
  return p:match("^src/(.+)") or p
end

--- Build combined JSON data for the view from eval result + edges.
--- @param eval_result table  Lua eval result with entries/groups
--- @param ast_edges table    Raw edges from AST
--- @param bindings table     Resolved bindings
--- @param domain_map table   Optional: node_name → domain_name mapping
--- @param layers_config table Optional: layers from config ({ name, domains })
function M.build_data(eval_result, ast_edges, bindings, domain_map, layers_config)
  domain_map = domain_map or {}
  layers_config = layers_config or {}

  -- Build file → domain lookup (domain_map keys are node.name = "file_name::symbol")
  local file_domain = {}
  for node_name, dom in pairs(domain_map) do
    if dom ~= "_excluded" then
      local file_part = node_name:match("^(.+)::") or node_name
      if not file_domain[file_part] then
        file_domain[file_part] = dom
      end
    end
  end

  -- Build stripped path → domain lookup (for edge endpoint resolution)
  local stripped_domain = {}
  for file, dom in pairs(file_domain) do
    local stripped = strip_src(file)
    if not stripped_domain[stripped] then
      stripped_domain[stripped] = dom
    end
  end

  -- 1. Aggregate entries by file
  local file_map = {}
  for _, entry in ipairs(eval_result.entries) do
    local n = entry.node
    local file = strip_src(n.file)
    local domain = file_domain[n.file] or "unknown"
    if not file_map[file] then
      file_map[file] = {
        id = file,
        domain = domain,
        lines = 0,
        entries = 0,
        max_cyclo = 0,
        hue_sum = 0,
        hue_count = 0,
        churn = 0,
        coverage_sum = 0,
        coverage_count = 0,
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
    f.churn = math.max(f.churn, n.git_churn_30d or 0)
    if n.coverage then
      f.coverage_sum = f.coverage_sum + n.coverage
      f.coverage_count = f.coverage_count + 1
    end
  end

  local nodes = {}
  for _, f in pairs(file_map) do
    f.avg_hue = f.hue_count > 0 and (f.hue_sum / f.hue_count) or 120
    f.hue_sum = nil
    f.hue_count = nil
    f.avg_coverage = f.coverage_count > 0 and (f.coverage_sum / f.coverage_count) or nil
    f.coverage_sum = nil
    f.coverage_count = nil
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
          id = id, domain = stripped_domain[id] or "unknown",
          lines = 0, entries = 0, max_cyclo = 0, avg_hue = 120,
          churn = 0, avg_coverage = nil,
        }
      end
    end
  end

  -- 4. Build domain list with auto-assigned colors (sorted for determinism)
  local domain_set = {}
  for _, n in ipairs(nodes) do domain_set[n.domain] = true end
  local sorted_doms = {}
  for d in pairs(domain_set) do sorted_doms[#sorted_doms + 1] = d end
  table.sort(sorted_doms)
  local domains = {}
  for i, d in ipairs(sorted_doms) do
    local c = PALETTE[((i - 1) % #PALETTE) + 1]
    local label = d:sub(1,1):upper() .. d:sub(2)
    domains[#domains + 1] = { name = d, color = c, label = label }
  end

  -- 5. Layers (pass through from config)
  local layers = {}
  for _, l in ipairs(layers_config) do
    layers[#layers + 1] = { name = l.name, domains = l.domains }
  end

  -- 6. Bindings info
  local binding_info = {}
  for _, b in ipairs(bindings) do
    binding_info[#binding_info + 1] = { index = b.index.name, percept = b.percept.name }
  end

  return __rustlib.json.encode({
    nodes = nodes,
    edges = edges,
    domains = domains,
    layers = layers,
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
#legend{position:fixed;bottom:20px;left:20px;background:var(--bg2);border:1px solid var(--border);border-radius:8px;padding:12px 16px;font-size:12px;z-index:10;min-width:170px;max-height:calc(100vh - 120px);overflow-y:auto}
.lg-t{font-weight:600;margin-bottom:8px;color:var(--fg)}
.lg-i{display:flex;align-items:center;gap:8px;margin:4px 0;color:var(--fg2);padding:2px 4px;border-radius:4px;transition:background .15s,opacity .15s}
.lg-i:hover{background:var(--bg3)}
.lg-d{width:12px;height:12px;border-radius:50%;flex-shrink:0}
.lg-d.sq{border-radius:2px}
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
.node-g .churn-ring{transition:opacity .15s}
.node-g .body{transition:opacity .15s}
.node-g .cov-badge{transition:opacity .15s}
.node-g.dim .ring,.node-g.dim .churn-ring,.node-g.dim .body,.node-g.dim .cov-badge{opacity:.1}
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

// ── Lookup maps ──
const DC={};for(const d of D.domains)DC[d.name]=d.color;
const DL={};for(const l of D.layers||[])for(const dom of l.domains||[])DL[dom]=l.name;
const layerDomains={};for(const l of D.layers||[])layerDomains[l.name]=new Set(l.domains||[]);

// ── Nodes (initial positions seeded by domain sector) ──
const W=window.innerWidth,H=window.innerHeight-48;
const domNames=D.domains.map(d=>d.name);
const domIdx={};for(let i=0;i<domNames.length;i++)domIdx[domNames[i]]=i;
const domCount=Math.max(1,domNames.length);
const sectorR=Math.min(W,H)*0.3;
const nodes=D.nodes.map(n=>{
  const di=domIdx[n.domain]!=null?domIdx[n.domain]:0;
  const angle=(2*Math.PI*di/domCount)-Math.PI/2;
  const jitter=sectorR*0.35;
  return{
    ...n,
    x:W/2+Math.cos(angle)*sectorR+(Math.random()-.5)*jitter,
    y:H/2+Math.sin(angle)*sectorR+(Math.random()-.5)*jitter,
    vx:0,vy:0,
    r:Math.max(22,Math.min(72,12+Math.sqrt(n.lines)*1.8)),
    color:DC[n.domain]||'#8b949e',
    fixed:false
  };
});
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
let lh='<div class="lg-t">Domains</div>';
D.domains.sort((a,b)=>a.label.localeCompare(b.label));
for(const d of D.domains)lh+='<div class="lg-i" data-ft="domain" data-fk="'+d.name+'" style="cursor:pointer"><div class="lg-d" style="background:'+d.color+'"></div>'+d.label+'</div>';
if(D.layers&&D.layers.length>0){
  lh+='<div class="lg-t" style="margin-top:12px">Layers</div>';
  for(const l of D.layers)lh+='<div class="lg-i" data-ft="layer" data-fk="'+l.name+'" style="cursor:pointer"><div class="lg-d sq" style="background:transparent;border:2px solid var(--fg2)"></div>'+l.name+' <span style="opacity:.5;font-size:10px">('+((l.domains||[]).length)+')</span></div>';
}
lh+='<div class="lg-m">Size = lines of code<br>Inner ring = complexity (hue)<br>Outer ring = <span style="color:#f0883e">git churn</span><br>Color = domain';
if(hasCoverage)lh+='<br>Badge = coverage';
lh+='<br><span style="opacity:.6">Click to filter</span></div>';
leg.innerHTML=lh;

// ── Filter (unified for domain/layer) ──
let activeFilter=null,lockedFilter=null;
function matchesFilter(domain,filter){
  if(!filter)return true;
  if(filter.type==='domain')return domain===filter.key;
  if(filter.type==='layer'){const ld=layerDomains[filter.key];return ld&&ld.has(domain);}
  return true;
}
function applyFilter(filter){
  activeFilter=filter;
  for(const li of leg.querySelectorAll('.lg-i'))li.style.opacity=(!filter||(li.dataset.ft===filter.type&&li.dataset.fk===filter.key))?1:0.35;
  for(const ne of nEls)ne.el.classList.toggle('dim',!matchesFilter(ne.d.domain,filter));
  for(const ee of eEls){
    const sn=NM[ee.d.source],tn=NM[ee.d.target];
    const hit=!filter||(sn&&matchesFilter(sn.domain,filter))||(tn&&matchesFilter(tn.domain,filter));
    ee.el.classList.toggle('dim',!hit);
  }
}
for(const li of leg.querySelectorAll('.lg-i')){
  li.addEventListener('mouseenter',function(){
    if(!lockedFilter)applyFilter({type:this.dataset.ft,key:this.dataset.fk});
  });
  li.addEventListener('mouseleave',function(){
    if(!lockedFilter)applyFilter(null);
  });
  li.addEventListener('click',function(){
    const f={type:this.dataset.ft,key:this.dataset.fk};
    if(lockedFilter&&lockedFilter.type===f.type&&lockedFilter.key===f.key){lockedFilter=null;applyFilter(null);}
    else{lockedFilter=f;applyFilter(f);}
    for(const li2 of leg.querySelectorAll('.lg-i'))li2.style.fontWeight=(lockedFilter&&lockedFilter.type===li2.dataset.ft&&lockedFilter.key===li2.dataset.fk)?'600':'';
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

// Coverage data availability (hide badges if all null)
const hasCoverage=nodes.some(n=>n.avg_coverage!=null);

// Churn max (for normalizing ring width)
const maxChurn=Math.max(1,...nodes.map(n=>n.churn||0));

// Node elements
const nEls=nodes.map(n=>{
  const g=document.createElementNS(NS,'g');
  g.classList.add('node-g');g.dataset.id=n.id;

  // Churn outer ring (amber, width proportional to churn)
  const churn=n.churn||0;
  if(churn>0){
    const cw=Math.max(2,Math.min(10,2+churn/maxChurn*8));
    const rw_inner=Math.max(2,Math.min(8,n.max_cyclo*0.5));
    const churnRing=document.createElementNS(NS,'circle');
    churnRing.classList.add('churn-ring');
    churnRing.setAttribute('r',n.r+rw_inner+cw/2+1);
    churnRing.setAttribute('stroke','#f0883e');
    churnRing.setAttribute('stroke-width',cw);
    churnRing.setAttribute('stroke-opacity',0.35+churn/maxChurn*0.45);
    churnRing.setAttribute('fill','none');
    g.appendChild(churnRing);
  }

  // Complexity inner ring
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

  // Coverage badge (only if coverage data exists somewhere)
  if(hasCoverage){
    const bx=n.r*0.6,by=-n.r*0.6;
    const badge=document.createElementNS(NS,'g');
    badge.classList.add('cov-badge');
    badge.setAttribute('transform','translate('+bx+','+by+')');
    if(n.avg_coverage!=null){
      // Shield: green=high, yellow=mid, red=low
      const cov=n.avg_coverage;
      const col=cov>=0.7?'#3fb950':cov>=0.4?'#d29922':'#f85149';
      const sh=document.createElementNS(NS,'path');
      sh.setAttribute('d','M0-6C-4-6-6-4-6-1C-6 3 0 6 0 6S6 3 6-1C6-4 4-6 0-6Z');
      sh.setAttribute('fill',col);sh.setAttribute('stroke','#0d1117');sh.setAttribute('stroke-width',0.8);
      badge.appendChild(sh);
      const pct=document.createElementNS(NS,'text');
      pct.textContent=Math.round(cov*100);
      pct.setAttribute('text-anchor','middle');pct.setAttribute('dy','1');
      pct.setAttribute('font-size','6');pct.setAttribute('fill','#fff');pct.setAttribute('font-weight','700');
      badge.appendChild(pct);
    }else{
      // Warning triangle: no coverage data
      const warn=document.createElementNS(NS,'path');
      warn.setAttribute('d','M0-5L5 4H-5Z');
      warn.setAttribute('fill','#6e7681');warn.setAttribute('stroke','#0d1117');warn.setAttribute('stroke-width',0.8);
      badge.appendChild(warn);
      const ex=document.createElementNS(NS,'text');
      ex.textContent='?';
      ex.setAttribute('text-anchor','middle');ex.setAttribute('dy','2');
      ex.setAttribute('font-size','6');ex.setAttribute('fill','#fff');ex.setAttribute('font-weight','700');
      badge.appendChild(ex);
    }
    g.appendChild(badge);
  }

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
  // Charge repulsion (same domain = less repulsion)
  for(let i=0;i<N;i++){for(let j=i+1;j<N;j++){
    const a=nodes[i],b=nodes[j];
    let dx=b.x-a.x||.1,dy=b.y-a.y||.1;
    const d2=dx*dx+dy*dy,d=Math.sqrt(d2);
    const str=a.domain===b.domain?-800:-1400;
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

  // Domain clustering
  const gc={},gn={};
  for(const n of nodes){
    if(!gc[n.domain]){gc[n.domain]={x:0,y:0};gn[n.domain]=0;}
    gc[n.domain].x+=n.x;gc[n.domain].y+=n.y;gn[n.domain]++;
  }
  for(const g in gc){gc[g].x/=gn[g];gc[g].y/=gn[g];}
  for(const n of nodes){
    if(n.fixed)continue;
    const c=gc[n.domain];
    n.vx+=(c.x-n.x)*0.02*alpha;
    n.vy+=(c.y-n.y)*0.02*alpha;
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
    // Re-apply filter if locked
    if(lockedFilter)applyFilter(lockedFilter);
  });
  el.addEventListener('mousemove',function(ev){if(hovId)moveTip(ev);});
}

// ── Tooltip ──
const tip=document.getElementById('tip');
function showTip(ev,n){
  let h='<h3>'+n.id+'</h3>';
  const domInfo=D.domains.find(function(d){return d.name===n.domain})||{};
  const layer=DL[n.domain];
  const rows=[
    ['Domain','<span style="color:'+(domInfo.color||'var(--fg)')+'">'+( domInfo.label||n.domain)+'</span>'],
  ];
  if(layer)rows.push(['Layer',layer]);
  rows.push(
    ['Code units',n.entries],
    ['Total lines',n.lines],
    ['Max cyclomatic',n.max_cyclo],
    ['Avg complexity hue',n.avg_hue!=null?n.avg_hue.toFixed(1):'—'],
    ['Git churn (30d)',(n.churn||0)>0?'<span style="color:#f0883e">'+n.churn+'</span>':'0'],
    ['Coverage',n.avg_coverage!=null?'<span style="color:'+(n.avg_coverage>=0.7?'#3fb950':n.avg_coverage>=0.4?'#d29922':'#f85149')+'">'+Math.round(n.avg_coverage*100)+'%</span>':'<span style="color:var(--fg2)">N/A</span>']
  );
  for(const r of rows)h+='<div class="tr"><span class="tl">'+r[0]+'</span><span class="tv">'+r[1]+'</span></div>';
  const deps=edges.filter(function(ed){return ed.source===n.id}).map(function(ed){return ed.target});
  const users=edges.filter(function(ed){return ed.target===n.id}).map(function(ed){return ed.source});
  if(deps.length)h+='<div class="ts">Depends on ('+deps.length+')</div><div class="tr"><span class="tl">'+deps.join(', ')+'</span></div>';
  if(users.length)h+='<div class="ts">Used by ('+users.length+')</div><div class="tr"><span class="tl">'+users.join(', ')+'</span></div>';
  const outEdges=edges.filter(function(ed){return ed.source===n.id});
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
    if(lockedFilter)applyFilter(lockedFilter);
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
