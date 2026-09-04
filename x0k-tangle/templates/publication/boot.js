// boot.js — the atlas boot shell. A stripped-down canvas host that boots
// render-vello over the FULL bundled lineage atlas — no daemon, no Solid, no MCP.
// The taxonomy is keyed by IDEA: a project that advances several ideas is ONE
// canonical node (atlas.json `nodes`, keyed by URI) TRANSCLUDED into every lane
// it belongs to (atlas.json `placements`, one (uri,lane) row). The renderer
// paints one glyph per placement — so Urbit shows up in the authority-identity,
// networking, AND durable-execution lanes while remaining one node with one mass
// and one doc. Two convergence bands (synthesis / frontier) sit at the present
// right edge.
//   - positions + lanes come from atlas.placements (x = year, y = lane row)
//   - canonical identity (mass, title, year, doc) comes from atlas.nodes
//   - glyph size = node mass (in-region citation count)
//   - influence arcs = atlas edges (drawn between primary placements); a faint
//     transclusion tie connects a node's copies across its lanes
//   - zoom-band LoD: atlas overview → node preview → deep-doc in place
// A vanilla-JS port of the live app's canvas host and renderer binding, so a
// published atlas runs the same code path as the app.

// ── bindRenderer (ported from the live app's renderer binding) ──────────────
const UNBOUND = new Set(['default', 'init', 'initSync', 'init_renderer', 'start', '__wbindgen_start', 'grove_world_spec_json']);
function bindRenderer(mod, handle) {
  const bound = Object.create(mod);
  for (const key of Object.keys(mod)) {
    const value = mod[key];
    if (typeof value !== 'function' || UNBOUND.has(key)) continue;
    Object.defineProperty(bound, key, {
      value: (...args) => value(handle, ...args),
      writable: true, configurable: true, enumerable: true,
    });
  }
  return bound;
}

// ── Placement-id helpers ────────────────────────────────────────────────────
// A placement (transcluded glyph) has a composite render id `<uri>#<lane>` so
// render-vello sees one entity per (node, lane). The canonical node URI carries
// no '#', so the base URI is everything before the first '#'.
function pidOf(uri, thread) { return `${uri}#${thread}`; }
function baseUri(pid) { const h = pid.indexOf('#'); return h < 0 ? pid : pid.slice(0, h); }

// ── #cam=zoom,cx,cy hash (ported from the live app's camera store) ──────────
function cameraFromHash() {
  const m = /[#&]cam=([^&]+)/.exec(location.hash);
  if (!m) return null;
  const [zoom, cx, cy] = decodeURIComponent(m[1]).split(',').map(Number);
  if ([zoom, cx, cy].some((n) => !Number.isFinite(n))) return null;
  return { zoom, cx, cy };
}
let camHashTimer = null;
function setCameraHash(zoom, cx, cy) {
  if (camHashTimer) clearTimeout(camHashTimer);
  camHashTimer = setTimeout(() => {
    const v = `${zoom.toFixed(4)},${cx.toFixed(2)},${cy.toFixed(2)}`;
    history.replaceState(null, '', `#cam=${encodeURIComponent(v)}`);
  }, 250);
}

// Human-readable lane titles for the eight idea-lanes + three bands.
const THREAD_TITLE = {
  'founding-vision': 'Founding vision — augment the intellect',
  'malleability': 'Malleability — shape your own tools',
  'data-ownership': 'Data ownership — own your data (local-first / CRDT)',
  'networking': 'Networking — route your own bytes (P2P)',
  'authority-identity': 'Authority & identity — hold your own keys',
  'privacy': 'Privacy — be unobservable (surveillance-resistance)',
  'durable-execution': 'Durable execution — computations that survive',
  'sovereign-ai': 'Sovereign AI — own your intelligence (open weights / local inference)',
  'synthesis': 'Synthesis — sovereign systems that fuse the threads',
  'capstone': 'Capstone — the contemporary umbrella-statement',
  'frontier': 'Frontier — open problems & current research',
};

// ── Idea-lane / band color system ────────────────────────────────────────────
// Each idea-lane gets a hue (the through-line shared by map glyphs, influence
// arcs, lane labels, narrative captions, and the mini-map); era is a VALUE ramp
// within the hue (older work deeper, newer brighter). The eight lane hues form a
// wheel: gold → terracotta → teal → steel-blue → indigo → green → rose (privacy
// gets a distinct deep-green "unobservable" hue between authority-identity's
// indigo and durable-execution's rose). The three bands get distinct off-wheel
// hues so the right-edge zone reads as "where the threads fuse / culminate",
// not as another lane.
const THREAD_COLORS = {
  'founding-vision':    '#c89537', // gold — augment the intellect
  'malleability':       '#c0562f', // terracotta — shape your tools
  'data-ownership':     '#2c8c78', // teal — own your data
  'networking':         '#3d6ea8', // steel-blue — route your bytes
  'authority-identity': '#5b4b9e', // indigo — hold your authority
  'privacy':            '#3f7d57', // deep-green — be unobservable
  'durable-execution':  '#a8446f', // rose — computations that survive
  'sovereign-ai':       '#8e3d9e', // magenta-violet — own your intelligence
  'synthesis':          '#5d5470', // slate-violet — the fusion zone
  'capstone':           '#b03a52', // crimson — the umbrella-statement
  'frontier':           '#c98a2e', // amber — the leading edge
};
const THREAD_FALLBACK = '#8a7a5a';
function threadColor(thread) { return THREAD_COLORS[thread] || THREAD_FALLBACK; }

function hexToRgb(hex) {
  const h = hex.replace('#', '');
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}
function rgbToHex(r, g, b) {
  const c = (n) => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0');
  return `#${c(r)}${c(g)}${c(b)}`;
}
// Mix a color toward `target` (white = lighten, black = deepen) by t∈[0,1].
function mix(hex, target, t) {
  const [r, g, b] = hexToRgb(hex);
  const [tr, tg, tb] = target;
  return rgbToHex(r + (tr - r) * t, g + (tg - g) * t, b + (tb - b) * t);
}
// Era value ramp: map a node's year into a lightness offset on its lane hue.
function eraShade(threadHex, year, minYear, maxYear) {
  const span = Math.max(1, maxYear - minYear);
  const t = Math.max(0, Math.min(1, (year - minYear) / span));
  if (t < 0.5) return mix(threadHex, [40, 32, 24], (0.5 - t) * 0.5);   // older → deeper
  return mix(threadHex, [250, 244, 232], (t - 0.5) * 0.7);              // newer → brighter
}

// Presentation-time vertical lane spread (see loadDataset): atlas lanes are
// 200 world-units apart; multiply so the lanes read as distinct bands at the
// whole-atlas fit zoom. atlas.json is never modified.
const Y_SCALE = 2.4;
const PORTAL_OPEN_LO = 1.5;  // matches doc-open.ts OPEN_LO
const PORTAL_OPEN_HI = 4.0;  // matches doc-open.ts OPEN_HI
const PORTAL_SHOW_EZ = 2.2;  // effective_zoom past which the DOM doc surface shows

// ── Build the renderer input set from the bundled atlas + member bodies ─────
async function loadDataset() {
  const atlas = await (await fetch('./atlas.json')).json();
  const members = (await (await fetch('./members.json')).json()).members;
  const nodeByUri = new Map(atlas.nodes.map((n) => [n.uri, n]));
  // Back-compat: if an atlas predates placements, synthesize one per node lane.
  const placements = atlas.placements && atlas.placements.length
    ? atlas.placements
    : atlas.nodes.flatMap((n) => (n.threads || [n.thread]).map((t) => ({ uri: n.uri, thread: t, x: n.x, y: n.y })));
  const placementsByUri = new Map();
  for (const p of placements) {
    if (!placementsByUri.has(p.uri)) placementsByUri.set(p.uri, []);
    placementsByUri.get(p.uri).push(p);
  }
  // Representative (primary) placement id of a node = its first lane.
  const repPid = (uri) => {
    const ps = placementsByUri.get(uri);
    return ps && ps.length ? pidOf(uri, ps[0].thread) : pidOf(uri, 'founding-vision');
  };

  // node degree (citation in/out), keyed by URI — drives connection glow.
  const degree = new Map();
  for (const e of atlas.edges) {
    degree.set(e.from, (degree.get(e.from) || 0) + 1);
    degree.set(e.to, (degree.get(e.to) || 0) + 1);
  }

  // ── Presentation-time intra-lane dodge ──────────────────────────────────
  // Same-year placements in one lane land on the IDENTICAL (x,y) point and
  // stack (e.g. loro + sync-engine, or the stacked band members). Resolve at
  // presentation time WITHOUT touching atlas.json — fan colliding placements
  // out vertically inside the lane band, heaviest-mass node on the centerline.
  const DODGE = 46;
  const renderY = new Map(); // pid -> world y
  const groups = new Map();  // key "lane|x" -> placements
  for (const p of placements) {
    const k = `${p.thread}|${Math.round(p.x)}`;
    if (!groups.has(k)) groups.set(k, []);
    groups.get(k).push(p);
  }
  const massOf = (uri) => (nodeByUri.get(uri) || {}).mass || 0;
  for (const [, g] of groups) {
    if (g.length === 1) { renderY.set(pidOf(g[0].uri, g[0].thread), g[0].y * Y_SCALE); continue; }
    g.sort((a, b) => massOf(b.uri) - massOf(a.uri));
    g.forEach((p, i) => {
      const rank = Math.ceil(i / 2) * (i % 2 === 1 ? -1 : 1);
      renderY.set(pidOf(p.uri, p.thread), p.y * Y_SCALE + rank * DODGE);
    });
  }

  // Year range for the era value ramp.
  const years = atlas.nodes.map((n) => n.year).filter((y) => Number.isFinite(y));
  const minYear = years.length ? Math.min(...years) : 1945;
  const maxYear = years.length ? Math.max(...years) : 2025;

  // One render entity PER PLACEMENT — the transclusion. id = `<uri>#<lane>`,
  // canonical metadata (mass, title) read from the node, tint = lane hue × era.
  const entities = placements.map((p) => {
    const n = nodeByUri.get(p.uri) || {};
    const pid = pidOf(p.uri, p.thread);
    return {
      id: pid,
      content_type: 'doc',
      position: [p.x, renderY.get(pid)],
      scale: 1.0 + Math.min(1.6, ((n.mass || 0) - 1) * 0.11),
      label: n.title || p.uri,
      connections: degree.get(p.uri) || 0,
      intents: 0,
      nested: false,
      container: false,
      tint: eraShade(threadColor(p.thread), n.year || minYear, minYear, maxYear),
    };
  });

  // Influence edges connect the PRIMARY placements of source/target so the
  // de-hairball overview stays one arc per citation. Per-edge tint = the source
  // node's primary-lane hue.
  const edges = atlas.edges.map((e) => {
    const ps = placementsByUri.get(e.from);
    const srcThread = ps && ps.length ? ps[0].thread : 'founding-vision';
    return { source: repPid(e.from), target: repPid(e.to), predicate: 'cites', weight: 1.0, tint: threadColor(srcThread) };
  });
  // Faint transclusion ties: connect a multi-lane node's copies so the eye
  // reads "same project, several ideas". Cheap (one tie per extra lane).
  for (const [uri, ps] of placementsByUri) {
    if (ps.length < 2) continue;
    for (let i = 1; i < ps.length; i++) {
      edges.push({
        source: pidOf(uri, ps[i - 1].thread),
        target: pidOf(uri, ps[i].thread),
        predicate: 'transclusion', weight: 0.4,
        tint: threadColor(ps[0].thread),
      });
    }
  }

  // Per-lane left anchor (min-x placement of each lane) for the lane labels.
  const laneAnchor = new Map();
  for (const p of placements) {
    const cur = laneAnchor.get(p.thread);
    if (!cur || p.x < cur.x) laneAnchor.set(p.thread, p);
  }
  return { atlas, members, entities, edges, laneAnchor, nodeByUri, placements, placementsByUri, repPid, minYear, maxYear, renderY };
}

async function main() {
  const canvas = document.getElementById('vello-canvas');
  const portal = document.getElementById('doc-portal');
  const laneLayer = document.getElementById('lane-labels');
  const dpr = window.devicePixelRatio || 1;
  const sizeCanvas = () => {
    const r = canvas.getBoundingClientRect();
    canvas.width = Math.max(1, Math.round(r.width * dpr));
    canvas.height = Math.max(1, Math.round(r.height * dpr));
  };
  sizeCanvas();

  const ds = await loadDataset();
  const { atlas, members, entities, edges, laneAnchor, nodeByUri, placements, placementsByUri, repPid } = ds;
  const posByPid = new Map(entities.map((e) => [e.id, e.position]));
  // Expand a set of node URIs to their placement ids (for emphasis / framing).
  const expandUris = (uris) => uris.flatMap((u) => (placementsByUri.get(u) || []).map((p) => pidOf(u, p.thread)));

  // Boot the wasm renderer over bundled data.
  const raw = await import('./wasm/x0k_ui_render_vello.js');
  await raw.default(); // resolves ./wasm/x0k_ui_render_vello_bg.wasm sibling
  const handle = await raw.init_renderer('vello-canvas');
  const mod = bindRenderer(raw, handle);
  mod.resize_canvas(canvas.width, canvas.height);

  // Push the bundled graph + real per-doc preview text (one per placement, so
  // each transcluded glyph opens the same canonical doc in place).
  mod.set_graph(JSON.stringify(entities), JSON.stringify(edges));
  for (const p of placements) {
    const n = nodeByUri.get(p.uri) || {};
    const m = members[p.uri] || {};
    mod.set_doc_preview_text(JSON.stringify({
      doc_id: pidOf(p.uri, p.thread), title: n.title || p.uri, first_heading: m.summary || '', badge_label: 'Wiki',
    }));
  }

  // Expose the renderer + a minimal store stub so the canvas-capture harness
  // recognizes a connected, bound page.
  window.__c0k_render = mod;
  window.__c0k_atlas = atlas;
  window.__c0k_entities = entities;
  window.__c0k_edges = edges;
  window.__c0k_threadColors = THREAD_COLORS;
  // Transclusion helpers for the validation harness (lane membership lookups).
  window.__c0k_placementsByUri = Object.fromEntries(
    [...placementsByUri].map(([u, ps]) => [u, ps.map((p) => p.thread)]),
  );
  window.__c0k_pidOf = pidOf;
  window.__c0k_store = {
    connectionStatus: () => 'connected',
    filteredDocs: () => atlas.nodes,
  };

  // Initial framing: restore #cam if present, else frame the whole atlas
  // (both axes — the region is now a 2D field of lanes + a convergence zone).
  const xs = entities.map((e) => e.position[0]);
  const ys = entities.map((e) => e.position[1]);
  const minX = Math.min(...xs), maxX = Math.max(...xs);
  const minY = Math.min(...ys), maxY = Math.max(...ys);
  const cam = cameraFromHash();
  if (cam) {
    mod.set_camera(cam.zoom, cam.cx, cam.cy);
  } else {
    const spanX = Math.max(1, maxX - minX);
    const spanY = Math.max(1, maxY - minY);
    const fitZoom = Math.min(canvas.width / (spanX * 1.25), canvas.height / (spanY * 1.6));
    mod.set_camera(fitZoom, (minX + maxX) / 2, (minY + maxY) / 2);
  }

  // ── Lane labels: world-anchored captions at each lane's / band's left edge ──
  const laneEls = new Map();
  for (const [thread, anchor] of laneAnchor) {
    const el = document.createElement('div');
    el.className = 'lane-label';
    el.textContent = THREAD_TITLE[thread] || thread;
    const hue = threadColor(thread);
    el.style.borderLeft = `3px solid ${hue}`;
    el.style.color = mix(hue, [40, 32, 24], 0.25);
    laneLayer.appendChild(el);
    laneEls.set(thread, { el, anchor });
  }

  // ── Node labels: name the high-mass key nodes per lane (persistent), and the
  // hovered placement (transient). Labels are keyed by placement id so the same
  // node can carry a label in each lane it appears in.
  const labelLayer = document.getElementById('node-labels');
  const keyPids = new Set();
  {
    const byThread = new Map();
    for (const p of placements) {
      if (!byThread.has(p.thread)) byThread.set(p.thread, []);
      byThread.get(p.thread).push(p);
    }
    const massOf = (uri) => (nodeByUri.get(uri) || {}).mass || 0;
    for (const [, g] of byThread) {
      g.sort((a, b) => massOf(b.uri) - massOf(a.uri));
      for (const p of g.slice(0, 2)) keyPids.add(pidOf(p.uri, p.thread));
    }
  }
  const labelEls = new Map(); // pid -> el
  function ensureLabel(pid) {
    if (!pid) return null;
    if (labelEls.has(pid)) return labelEls.get(pid);
    const n = nodeByUri.get(baseUri(pid));
    if (!n) return null;
    const el = document.createElement('div');
    el.className = 'node-label';
    el.textContent = n.title;
    el.style.setProperty('--lbl', threadColor(pid.slice(baseUri(pid).length + 1)));
    labelLayer.appendChild(el);
    labelEls.set(pid, el);
    return el;
  }
  for (const pid of keyPids) ensureLabel(pid);
  let hoverPid = null;
  function syncNodeLabels() {
    const show = new Set(keyPids);
    if (hoverPid) show.add(hoverPid);
    for (const [pid, el] of labelEls) {
      if (!show.has(pid)) { el.style.display = 'none'; continue; }
      const wp = posByPid.get(pid);
      if (!wp) { el.style.display = 'none'; continue; }
      const sp = mod.world_to_screen(wp[0], wp[1]);
      if (!sp) { el.style.display = 'none'; continue; }
      const sx = sp[0] / dpr, sy = sp[1] / dpr;
      if (sx < -80 || sx > window.innerWidth + 80 || sy < -40 || sy > window.innerHeight + 40) {
        el.style.display = 'none'; continue;
      }
      el.style.display = 'block';
      el.classList.toggle('hover', pid === hoverPid);
      el.style.left = (sx + 10) + 'px';
      el.style.top = (sy - 8) + 'px';
    }
  }
  canvas.addEventListener('pointermove', (e) => {
    const r = canvas.getBoundingClientRect();
    let pid = null;
    try { pid = mod.hit_test((e.clientX - r.left) * dpr, (e.clientY - r.top) * dpr); } catch {}
    if (pid !== hoverPid) { hoverPid = pid; ensureLabel(hoverPid); syncNodeLabels(); }
  });

  // ── Persistent mini-map: the whole atlas at a glance with a "you are here"
  // viewport rectangle. Dots are lane-colored (one per placement); the rect
  // tracks the camera. Click to recenter.
  const miniSvg = document.getElementById('minimap');
  const MINI_W = 188, MINI_H = 132, MINI_PAD = 8;
  const wbx = { minX: Infinity, maxX: -Infinity, minY: Infinity, maxY: -Infinity };
  for (const e of entities) {
    wbx.minX = Math.min(wbx.minX, e.position[0]); wbx.maxX = Math.max(wbx.maxX, e.position[0]);
    wbx.minY = Math.min(wbx.minY, e.position[1]); wbx.maxY = Math.max(wbx.maxY, e.position[1]);
  }
  const wSpanX = Math.max(1, wbx.maxX - wbx.minX), wSpanY = Math.max(1, wbx.maxY - wbx.minY);
  const miniScale = Math.min((MINI_W - 2 * MINI_PAD) / wSpanX, (MINI_H - 2 * MINI_PAD) / wSpanY);
  const worldToMini = (wx, wy) => [
    MINI_PAD + (wx - wbx.minX) * miniScale,
    MINI_PAD + (wy - wbx.minY) * miniScale,
  ];
  miniSvg.setAttribute('viewBox', `0 0 ${MINI_W} ${MINI_H}`);
  {
    const dots = [];
    for (const e of entities) {
      const [mx, my] = worldToMini(e.position[0], e.position[1]);
      const n = nodeByUri.get(baseUri(e.id)) || {};
      const rr = 1.4 + Math.min(2.2, ((n.mass || 0) - 1) * 0.18);
      dots.push(`<circle cx="${mx.toFixed(1)}" cy="${my.toFixed(1)}" r="${rr.toFixed(1)}" fill="${e.tint}" />`);
    }
    miniSvg.innerHTML =
      `<rect x="0.5" y="0.5" width="${MINI_W - 1}" height="${MINI_H - 1}" rx="6" class="mini-bg"/>` +
      dots.join('') +
      `<rect id="mini-view" class="mini-view"/>`;
  }
  const miniView = miniSvg.querySelector('#mini-view');
  function syncMinimap() {
    const c = mod.get_camera(); if (!c || c.length !== 3) return;
    const [zoom, cx, cy] = c;
    const halfW = (canvas.width / 2) / zoom, halfH = (canvas.height / 2) / zoom;
    const [vx0, vy0] = worldToMini(cx - halfW, cy - halfH);
    const [vx1, vy1] = worldToMini(cx + halfW, cy + halfH);
    const x = Math.max(0, Math.min(MINI_W, vx0)), y = Math.max(0, Math.min(MINI_H, vy0));
    const w = Math.max(3, Math.min(MINI_W, vx1) - x), h = Math.max(3, Math.min(MINI_H, vy1) - y);
    miniView.setAttribute('x', x.toFixed(1)); miniView.setAttribute('y', y.toFixed(1));
    miniView.setAttribute('width', w.toFixed(1)); miniView.setAttribute('height', h.toFixed(1));
  }
  miniSvg.addEventListener('click', (e) => {
    const r = miniSvg.getBoundingClientRect();
    const mx = (e.clientX - r.left) / r.width * MINI_W, my = (e.clientY - r.top) / r.height * MINI_H;
    const wx = wbx.minX + (mx - MINI_PAD) / miniScale, wy = wbx.minY + (my - MINI_PAD) / miniScale;
    const c = mod.get_camera();
    mod.set_camera(c ? c[0] : 0.2, wx, wy); scheduleRender();
  });
  function syncLaneLabels() {
    for (const { el, anchor } of laneEls.values()) {
      const sp = mod.world_to_screen(anchor.x - 200, anchor.y * Y_SCALE);
      if (!sp) { el.style.display = 'none'; continue; }
      const sy = sp[1] / dpr;
      el.style.display = (sy < -40 || sy > window.innerHeight + 40) ? 'none' : 'block';
      el.style.left = '12px';
      el.style.top = Math.max(2, Math.min(window.innerHeight - 18, sy - 9)) + 'px';
    }
  }

  // ── Deep-doc DOM portal: render the focused member's document in place ────
  function syncDocPortal() {
    let boxes;
    try { boxes = JSON.parse(mod.get_doc_preview_boxes()); } catch { boxes = []; }
    let best = null;
    for (const b of boxes) if (!best || b.effective_zoom > best.effective_zoom) best = b;
    if (best && best.effective_zoom >= PORTAL_SHOW_EZ) {
      const open = Math.max(0, Math.min(1, (best.effective_zoom - PORTAL_OPEN_LO) / (PORTAL_OPEN_HI - PORTAL_OPEN_LO)));
      try { mod.set_doc_open(best.doc_id, open); } catch {}
      const uri = baseUri(best.doc_id);
      const thread = best.doc_id.slice(uri.length + 1);
      const n = nodeByUri.get(uri);
      const m = members[uri] || {};
      if (n) {
        const bx = best.screen_x / dpr, by = best.screen_y / dpr;
        const w = Math.min(window.innerWidth - 24, Math.max(best.screen_width / dpr, 460));
        const h = Math.min(window.innerHeight - 24, Math.max(best.screen_height / dpr, 340));
        const left = Math.max(12, Math.min(window.innerWidth - w - 12, bx));
        const top = Math.max(12, Math.min(window.innerHeight - h - 12, by));
        portal.style.display = 'block';
        portal.style.left = left + 'px';
        portal.style.top = top + 'px';
        portal.style.width = w + 'px';
        portal.style.height = h + 'px';
        portal.dataset.docId = uri;
        const hue = threadColor(thread) || 'var(--color-accent)';
        portal.style.borderColor = hue;
        const lanes = (n.threads || [thread]).join(' · ');
        portal.innerHTML =
          `<span class="badge" style="color:${hue};border-color:${hue}">${best.badge_label || 'Wiki'} · ${n.year} · ${lanes}</span>` +
          `<h1>${n.title}</h1>` +
          `<p class="lede">${m.summary || ''}</p>` +
          `<div class="doc-body"><p>${m.body || ''}</p></div>`;
      }
    } else {
      try { mod.set_doc_open('', 0); } catch {}
      portal.style.display = 'none';
      portal.dataset.docId = '';
    }
  }

  let lastCam = [NaN, NaN, NaN];
  let rafScheduled = false;
  function scheduleRender() {
    if (rafScheduled) return;
    rafScheduled = true;
    requestAnimationFrame(() => {
      rafScheduled = false;
      try {
        mod.render_frame();
        syncDocPortal();
        syncLaneLabels();
        syncNodeLabels();
        syncMinimap();
        const c = mod.get_camera();
        if (c && c.length === 3 &&
            (Math.abs(c[0] - lastCam[0]) > 1e-3 || Math.abs(c[1] - lastCam[1]) > 0.5 || Math.abs(c[2] - lastCam[2]) > 0.5)) {
          lastCam = [c[0], c[1], c[2]];
          setCameraHash(c[0], c[1], c[2]);
        }
        if (mod.camera_settling && mod.camera_settling()) scheduleRender();
      } catch (err) { console.error('[atlas] render failed', err); }
    });
  }

  // Expose a render hook so the validation harness can repaint after set_camera.
  window.__mockup_render = scheduleRender;
  window.__atlas_render = scheduleRender;

  // ── Input: wheel = zoom (shift/ctrl = pan), pointer drag = pan ───────────
  document.addEventListener('wheel', (e) => {
    if (e.target.closest && e.target.closest('#doc-portal')) {
      const p = e.target.closest('#doc-portal');
      if (p.scrollHeight > p.clientHeight + 1) return;
    }
    e.preventDefault();
    const r = canvas.getBoundingClientRect();
    const mx = (e.clientX - r.left) * dpr;
    const my = (e.clientY - r.top) * dpr;
    const wantsPan = e.shiftKey || e.ctrlKey;
    if (mod.handle_wheel(e.deltaX, e.deltaY, mx, my, !wantsPan)) scheduleRender();
  }, { passive: false });

  let dragging = false;
  const off = (e) => {
    const r = canvas.getBoundingClientRect();
    return { x: (e.clientX - r.left) * dpr, y: (e.clientY - r.top) * dpr };
  };
  canvas.addEventListener('pointerdown', (e) => {
    canvas.setPointerCapture(e.pointerId); dragging = true;
    const { x, y } = off(e); if (mod.handle_pointer_down(x, y, e.button)) scheduleRender();
  });
  canvas.addEventListener('pointermove', (e) => {
    const { x, y } = off(e); if (mod.handle_pointer_move(x, y)) scheduleRender();
  });
  canvas.addEventListener('pointerup', (e) => {
    try { canvas.releasePointerCapture(e.pointerId); } catch {}
    dragging = false; const { x, y } = off(e); if (mod.handle_pointer_up(x, y, e.button)) scheduleRender();
  });

  window.addEventListener('resize', () => { sizeCanvas(); mod.resize_canvas(canvas.width, canvas.height); scheduleRender(); syncEdges(); });

  // ── Narrative: a scripted camera trail over the atlas (Phase 4) ───────────
  const narrative = await (await fetch('./narrative.json')).json();
  const edgeSvg = document.getElementById('edge-overlay');
  const panel = document.getElementById('narrative');
  const launch = document.getElementById('narrative-launch');
  let currentStation = -1;        // -1 = overview (no station active)
  let activeEdges = [];           // [from,to] uri pairs drawn for the current beat
  let easeToken = 0;              // cancels an in-flight ease when a new step starts

  // The beat's edges = authored key_edges ∪ atlas edges fully inside the set.
  function computeActiveEdges(st) {
    const set = new Set(st.highlight);
    const seen = new Set(); const out = [];
    for (const [a, b] of (st.key_edges || [])) { out.push([a, b]); seen.add(a + '>' + b); }
    for (const e of atlas.edges) {
      if (set.has(e.from) && set.has(e.to)) {
        const k = e.from + '>' + e.to;
        if (!seen.has(k)) { out.push([e.from, e.to]); seen.add(k); }
      }
    }
    return out;
  }

  // Camera target for a station: fit the highlight set's RENDERED placement
  // positions (every lane copy of every highlighted node).
  function stationTarget(uris) {
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity, n = 0;
    for (const pid of expandUris(uris)) {
      const wp = mod.get_entity_world_position(pid);
      if (!wp) continue;
      minX = Math.min(minX, wp[0]); maxX = Math.max(maxX, wp[0]);
      minY = Math.min(minY, wp[1]); maxY = Math.max(maxY, wp[1]); n++;
    }
    if (!n) return null;
    const cx = (minX + maxX) / 2, cy = (minY + maxY) / 2;
    const spanX = Math.max(260, maxX - minX), spanY = Math.max(260, maxY - minY);
    const MARGIN = 1.6;
    let z = Math.min(canvas.width / (spanX * MARGIN), canvas.height / (spanY * MARGIN));
    z = Math.max(0.13, Math.min(z, 1.2));
    return { zoom: z, cx, cy };
  }

  function easeCamera(target, ms = 950) {
    return new Promise((resolve) => {
      const start = mod.get_camera();
      if (!start || start.length !== 3) { mod.set_camera(target.zoom, target.cx, target.cy); scheduleRender(); return resolve(); }
      const [sZoom, sX, sY] = start;
      const token = ++easeToken;
      const t0 = performance.now();
      const tick = (now) => {
        if (token !== easeToken) return resolve(); // superseded
        const t = Math.min(1, (now - t0) / ms);
        const e = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2; // easeInOutQuad
        const z = Math.exp(Math.log(sZoom) + (Math.log(target.zoom) - Math.log(sZoom)) * e);
        mod.set_camera(z, sX + (target.cx - sX) * e, sY + (target.cy - sY) * e);
        mod.render_frame(); syncEdges(); syncLaneLabels(); syncNodeLabels(); syncMinimap();
        if (t < 1) requestAnimationFrame(tick);
        else { scheduleRender(); resolve(); }
      };
      requestAnimationFrame(tick);
    });
  }

  // Draw the current beat's key influence arcs in screen space (between the
  // primary placements of each endpoint), refreshed every frame.
  function syncEdges() {
    if (!activeEdges.length) { edgeSvg.innerHTML = ''; return; }
    let cx = 0, cy = 0; const parts = [], pts = [];
    const screen = (uri) => {
      const wp = mod.get_entity_world_position(repPid(uri)); if (!wp) return null;
      const sp = mod.world_to_screen(wp[0], wp[1]); if (!sp) return null;
      return [sp[0] / dpr, sp[1] / dpr];
    };
    for (const [a, b] of activeEdges) {
      const pa = screen(a), pb = screen(b); if (!pa || !pb) continue;
      cx += pa[0] + pb[0]; cy += pa[1] + pb[1]; pts.push([pa, pb]);
    }
    if (!pts.length) { edgeSvg.innerHTML = ''; return; }
    cx /= pts.length * 2; cy /= pts.length * 2;
    pts.forEach(([pa, pb], i) => {
      const src = activeEdges[i] ? activeEdges[i][0] : null;
      const ps = src ? placementsByUri.get(src) : null;
      const hue = threadColor(ps && ps.length ? ps[0].thread : null);
      const mx = (pa[0] + pb[0]) / 2, my = (pa[1] + pb[1]) / 2;
      const qx = mx + (mx - cx) * 0.18, qy = my + (my - cy) * 0.18; // gentle outward bow
      parts.push(`<path stroke="${hue}" d="M ${pa[0].toFixed(1)} ${pa[1].toFixed(1)} Q ${qx.toFixed(1)} ${qy.toFixed(1)} ${pb[0].toFixed(1)} ${pb[1].toFixed(1)}"/>`);
      parts.push(`<circle stroke="${hue}" cx="${pb[0].toFixed(1)}" cy="${pb[1].toFixed(1)}" r="6"/>`);
    });
    edgeSvg.innerHTML = parts.join('');
  }

  function renderCaption(st) {
    const total = narrative.stations.length;
    const hue = threadColor(st.thread);
    panel.style.setProperty('--beat', hue);
    panel.style.borderTop = `3px solid ${hue}`;
    const dots = narrative.stations.map((_, i) => `<i class="${i === st.ordinal - 1 ? 'on' : ''}"></i>`).join('');
    panel.innerHTML =
      `<div class="eyebrow"><span>Station ${st.ordinal} / ${total}</span><span>${st.thread.split('-')[0]}</span></div>` +
      `<h2>${st.title}</h2>` +
      `<p class="empower">${st.empower}</p>` +
      `<p class="shadow"><b>The extractive shadow</b>${st.shadow}</p>` +
      `<div class="controls">` +
        `<button id="nav-prev"${st.ordinal === 1 ? ' disabled' : ''}>← Prev</button>` +
        `<button id="nav-next"${st.ordinal === total ? ' disabled' : ''}>Next →</button>` +
        `<button id="nav-exit">Exit</button>` +
        `<span class="dots">${dots}</span>` +
      `</div>` +
      `<div class="hint">← / → or scroll here to walk · Esc to exit</div>`;
    panel.querySelector('#nav-prev').onclick = () => prev();
    panel.querySelector('#nav-next').onclick = () => next();
    panel.querySelector('#nav-exit').onclick = () => exitNarrative();
  }

  async function go(i) {
    if (i < 0 || i >= narrative.stations.length) return null;
    const st = narrative.stations[i];
    currentStation = i;
    launch.style.display = 'none';
    panel.classList.add('show');
    try { mod.set_emphasis_set(JSON.stringify(expandUris(st.highlight))); } catch {}
    activeEdges = computeActiveEdges(st);
    renderCaption(st);
    const target = stationTarget(st.highlight);
    if (target) await easeCamera(target);
    syncEdges();
    const c = mod.get_camera();
    return { station: i, id: st.id, camera: c ? Array.from(c) : null };
  }
  const next = () => go(Math.min(currentStation + 1, narrative.stations.length - 1));
  const prev = () => go(Math.max(currentStation - 1, 0));
  function exitNarrative() {
    easeToken++; currentStation = -1; activeEdges = []; edgeSvg.innerHTML = '';
    try { mod.set_emphasis_set('[]'); } catch {}
    panel.classList.remove('show'); launch.style.display = 'block';
    const xs = entities.map((e) => e.position[0]), ys = entities.map((e) => e.position[1]);
    const minX = Math.min(...xs), maxX = Math.max(...xs), minY = Math.min(...ys), maxY = Math.max(...ys);
    const spanX = Math.max(1, maxX - minX), spanY = Math.max(1, maxY - minY);
    const z = Math.min(canvas.width / (spanX * 1.25), canvas.height / (spanY * 1.6));
    easeCamera({ zoom: z, cx: (minX + maxX) / 2, cy: (minY + maxY) / 2 }, 700);
  }

  launch.onclick = () => go(0);
  window.addEventListener('keydown', (e) => {
    if (currentStation < 0) { if (e.key === 'Enter') go(0); return; }
    if (e.key === 'ArrowRight' || e.key === ' ') { e.preventDefault(); next(); }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); prev(); }
    else if (e.key === 'Escape') exitNarrative();
  });
  let scrollLock = 0;
  panel.addEventListener('wheel', (e) => {
    e.preventDefault(); e.stopPropagation();
    const now = performance.now(); if (now - scrollLock < 480) return; scrollLock = now;
    if (e.deltaY > 0) next(); else if (e.deltaY < 0) prev();
  }, { passive: false });

  window.__narrative = {
    stations: narrative.stations,
    thesis: narrative.thesis,
    go, next, prev,
    current: () => currentStation,
    target: (uris) => stationTarget(uris),
  };

  console.info(`[atlas] booted: ${atlas.nodes.length} nodes → ${entities.length} placements, ${edges.length} ties, ${laneEls.size} lanes/bands, ${narrative.stations.length} narrative stations, offline`);
  scheduleRender();
}

main().catch((e) => {
  console.error('[atlas] boot failed — falling back to the semantic reader', e);
  try { location.replace('pages/index.html'); } catch { location.href = 'pages/index.html'; }
});
