//! Reaction catalog generator.
//! Reads all material and rule RON files, validates them, and outputs
//! a self-contained HTML file with an interactive interaction matrix.
//!
//! Usage: cargo run -p alkahest-rules --example reaction_catalog > catalog.html

use alkahest_rules::defaults;
use alkahest_rules::loader::{load_all_materials, load_all_rules};

fn main() {
    let table = load_all_materials(&[
        include_str!("../../../data/materials/naturals.ron"),
        include_str!("../../../data/materials/organics.ron"),
        include_str!("../../../data/materials/energy.ron"),
        include_str!("../../../data/materials/explosives.ron"),
        include_str!("../../../data/materials/metals.ron"),
        include_str!("../../../data/materials/synthetics.ron"),
        include_str!("../../../data/materials/exotic.ron"),
    ])
    .expect("Failed to load materials");

    let rules = load_all_rules(&[
        include_str!("../../../data/rules/combustion.ron"),
        include_str!("../../../data/rules/structural.ron"),
        include_str!("../../../data/rules/phase_change.ron"),
        include_str!("../../../data/rules/dissolution.ron"),
        include_str!("../../../data/rules/displacement.ron"),
        include_str!("../../../data/rules/biological.ron"),
        include_str!("../../../data/rules/thermal.ron"),
        include_str!("../../../data/rules/synthesis.ron"),
    ])
    .expect("Failed to load rules");

    // Build JSON data for materials
    let mut materials_json = String::from("[");
    for (i, mat) in table.materials.iter().enumerate() {
        if i > 0 {
            materials_json.push(',');
        }
        let cat = defaults::get_category(mat.id);
        materials_json.push_str(&format!(
            r#"{{"id":{},"name":"{}","category":"{}","color":[{:.2},{:.2},{:.2}],"phase":"{}","density":{:.1}}}"#,
            mat.id,
            mat.name.replace('"', r#"\""#),
            cat,
            mat.color.0, mat.color.1, mat.color.2,
            format!("{:?}", mat.phase),
            mat.density,
        ));
    }
    materials_json.push(']');

    // Build JSON data for rules
    let mut rules_json = String::from("[");
    for (i, rule) in rules.rules.iter().enumerate() {
        if i > 0 {
            rules_json.push(',');
        }
        rules_json.push_str(&format!(
            r#"{{"name":"{}","ia":{},"ib":{},"oa":{},"ob":{},"prob":{:.2},"td":{},"mt":{},"xt":{},"pd":{}}}"#,
            rule.name.replace('"', r#"\""#),
            rule.input_a, rule.input_b,
            rule.output_a, rule.output_b,
            rule.probability,
            rule.temp_delta,
            rule.min_temp, rule.max_temp,
            rule.pressure_delta,
        ));
    }
    rules_json.push(']');

    // Stats
    let total_materials = table.len();
    let total_rules = rules.len();

    // Count per category
    let categories = [
        ("Legacy", 0u16, 15u16),
        ("Naturals", defaults::NATURALS_START, defaults::NATURALS_END),
        ("Metals", defaults::METALS_START, defaults::METALS_END),
        ("Organics", defaults::ORGANICS_START, defaults::ORGANICS_END),
        ("Energy", defaults::ENERGY_START, defaults::ENERGY_END),
        (
            "Synthetics",
            defaults::SYNTHETICS_START,
            defaults::SYNTHETICS_END,
        ),
        ("Exotic", defaults::EXOTIC_START, defaults::EXOTIC_END),
    ];

    let mut cat_stats = String::from("[");
    for (i, (name, start, end)) in categories.iter().enumerate() {
        if i > 0 {
            cat_stats.push(',');
        }
        let count = table
            .materials
            .iter()
            .filter(|m| m.id >= *start && m.id <= *end)
            .count();
        cat_stats.push_str(&format!(r#"{{"name":"{}","count":{}}}"#, name, count));
    }
    cat_stats.push(']');

    // Output self-contained HTML
    print!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Alkahest Reaction Catalog</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:system-ui,sans-serif;background:#1a1a2e;color:#e0e0e0;padding:20px}}
h1{{color:#eee;margin-bottom:10px}}
.stats{{display:flex;gap:20px;flex-wrap:wrap;margin-bottom:20px}}
.stat-card{{background:#16213e;padding:12px 20px;border-radius:8px;min-width:120px}}
.stat-card .value{{font-size:24px;font-weight:bold;color:#0f3460}}
.stat-card .label{{font-size:12px;color:#888}}
.cat-bar{{display:flex;gap:4px;margin-bottom:20px;flex-wrap:wrap}}
.cat-chip{{padding:6px 12px;border-radius:4px;font-size:12px;cursor:pointer;border:2px solid transparent}}
.cat-chip.active{{border-color:#fff}}
.cat-chip .count{{font-weight:bold;margin-left:4px}}
.controls{{margin-bottom:16px;display:flex;gap:12px;align-items:center}}
.controls input{{padding:8px 12px;border-radius:4px;border:1px solid #333;background:#0a0a23;color:#eee;width:300px}}
#matrix{{overflow:auto;max-height:70vh;position:relative}}
table{{border-collapse:collapse;font-size:11px}}
th{{position:sticky;background:#16213e;padding:2px 4px;white-space:nowrap;z-index:1}}
th.row-header{{left:0;z-index:2;text-align:right}}
th.col-header{{top:0;writing-mode:vertical-lr;text-orientation:mixed;transform:rotate(180deg)}}
td{{width:16px;height:16px;min-width:16px;border:1px solid #111;cursor:pointer;position:relative}}
td:hover{{outline:2px solid #fff;z-index:3}}
.tooltip{{position:fixed;background:#222;color:#eee;padding:8px 12px;border-radius:6px;font-size:12px;
  pointer-events:none;z-index:100;max-width:350px;box-shadow:0 4px 12px rgba(0,0,0,0.5)}}
.cat-Legacy{{background:#555}}.cat-Naturals{{background:#5d4e37}}.cat-Metals{{background:#708090}}
.cat-Organics{{background:#3a5f3a}}.cat-Energy{{background:#8b4513}}.cat-Synthetics{{background:#4a5568}}
.cat-Exotic{{background:#6b21a8}}
</style>
</head>
<body>
<h1>Alkahest Reaction Catalog</h1>
<div class="stats" id="stats"></div>
<div class="cat-bar" id="catbar"></div>
<div class="controls">
  <input type="text" id="search" placeholder="Search materials or rules...">
  <label><input type="checkbox" id="showEmpty" checked> Show empty cells</label>
</div>
<div id="matrix"></div>
<div class="tooltip" id="tip" style="display:none"></div>
<script>
const materials={materials};
const rules={rules};
const catStats={cats};
const TOTAL_MATERIALS={total_m};
const TOTAL_RULES={total_r};

// Build lookup maps
const matById={{}};
materials.forEach(m=>matById[m.id]=m);
const ruleMap={{}};
rules.forEach(r=>{{
  const key=Math.min(r.ia,r.ib)+','+Math.max(r.ia,r.ib);
  if(!ruleMap[key])ruleMap[key]=[];
  ruleMap[key].push(r);
}});

// Category colors
const catColors={{Legacy:'#555',Naturals:'#8B7355',Metals:'#A8B2BD',Organics:'#6B8E5A',
  Energy:'#D4762C',Synthetics:'#7B8A9E',Exotic:'#9B59B6',Air:'#333',Unknown:'#333'}};

// Stats
const statsEl=document.getElementById('stats');
statsEl.innerHTML=`<div class="stat-card"><div class="value">${{TOTAL_MATERIALS}}</div><div class="label">Materials</div></div>
<div class="stat-card"><div class="value">${{TOTAL_RULES}}</div><div class="label">Rules</div></div>
<div class="stat-card"><div class="value">${{Object.keys(ruleMap).length}}</div><div class="label">Unique Pairs</div></div>`;

// Category bar
const catBar=document.getElementById('catbar');
const activeCats=new Set(catStats.map(c=>c.name));
catStats.forEach(c=>{{
  const chip=document.createElement('span');
  chip.className=`cat-chip cat-${{c.name}} active`;
  chip.innerHTML=`${{c.name}}<span class="count">${{c.count}}</span>`;
  chip.style.background=catColors[c.name]||'#333';
  chip.onclick=()=>{{
    if(activeCats.has(c.name)){{activeCats.delete(c.name);chip.classList.remove('active')}}
    else{{activeCats.add(c.name);chip.classList.add('active')}}
    buildMatrix();
  }};
  catBar.appendChild(chip);
}});

// Tooltip
const tip=document.getElementById('tip');
document.addEventListener('mousemove',e=>{{tip.style.left=(e.clientX+12)+'px';tip.style.top=(e.clientY+12)+'px'}});

function buildMatrix(){{
  const search=document.getElementById('search').value.toLowerCase();
  const showEmpty=document.getElementById('showEmpty').checked;
  const filtered=materials.filter(m=>activeCats.has(m.category)&&(!search||m.name.toLowerCase().includes(search)));
  filtered.sort((a,b)=>a.id-b.id);
  if(filtered.length>200){{
    document.getElementById('matrix').innerHTML='<p>Too many materials to display. Use category filters or search.</p>';
    return;
  }}
  const ids=filtered.map(m=>m.id);
  const idSet=new Set(ids);
  let html='<table><tr><th class="row-header"></th>';
  filtered.forEach(m=>{{
    html+=`<th class="col-header" style="color:${{catColors[m.category]||'#eee'}}">${{m.name}}</th>`;
  }});
  html+='</tr>';
  filtered.forEach(row=>{{
    html+=`<tr><th class="row-header" style="color:${{catColors[row.category]||'#eee'}}">${{row.name}}</th>`;
    filtered.forEach(col=>{{
      const key=Math.min(row.id,col.id)+','+Math.max(row.id,col.id);
      const rs=ruleMap[key];
      if(rs){{
        const count=rs.length;
        const maxTd=Math.max(...rs.map(r=>Math.abs(r.td)));
        const hue=maxTd>200?0:maxTd>50?30:120;
        const light=30+Math.min(count*10,40);
        html+=`<td style="background:hsl(${{hue}},${{60}}%,${{light}}%)" data-key="${{key}}"`;
        html+=` onmouseenter="showTip(this,'${{key}}')" onmouseleave="hideTip()"></td>`;
      }}else if(showEmpty){{
        html+='<td></td>';
      }}else{{
        html+='<td></td>';
      }}
    }});
    html+='</tr>';
  }});
  html+='</table>';
  document.getElementById('matrix').innerHTML=html;
}}

function showTip(el,key){{
  const rs=ruleMap[key];
  if(!rs)return;
  const ids=key.split(',').map(Number);
  const a=matById[ids[0]],b=matById[ids[1]];
  let html=`<b>${{a?a.name:'?'}} + ${{b?b.name:'?'}}</b><br>${{rs.length}} rule(s):<br>`;
  rs.forEach(r=>{{
    const oa=matById[r.oa],ob=matById[r.ob];
    html+=`&rarr; ${{oa?oa.name:'?'}} + ${{ob?ob.name:'?'}}`;
    if(r.prob<1)html+=` (p=${{r.prob}})`;
    if(r.td)html+=` td=${{r.td}}`;
    if(r.pd)html+=` pd=${{r.pd}}`;
    if(r.mt||r.xt)html+=` [${{r.mt}}-${{r.xt||'inf'}}K]`;
    html+='<br>';
  }});
  tip.innerHTML=html;
  tip.style.display='block';
}}
function hideTip(){{tip.style.display='none'}}

document.getElementById('search').oninput=buildMatrix;
document.getElementById('showEmpty').onchange=buildMatrix;
buildMatrix();
</script>
</body>
</html>"##,
        materials = materials_json,
        rules = rules_json,
        cats = cat_stats,
        total_m = total_materials,
        total_r = total_rules,
    );
}
