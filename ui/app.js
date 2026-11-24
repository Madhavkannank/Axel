(() => {
  'use strict';

  const ACCENTS = {
    amber: { core: '#e59a54', rim: '#f6c896', halo: 'rgba(229, 154, 84, 0.34)', text: '#fde6c4' },
    violet: { core: '#7b8cff', rim: '#b4c0ff', halo: 'rgba(123, 140, 255, 0.34)', text: '#dbe2ff' },
    cyan: { core: '#6ea8fe', rim: '#a8cbff', halo: 'rgba(110, 168, 254, 0.30)', text: '#d6e6ff' },
    emerald: { core: '#5ec2a6', rim: '#9be2cd', halo: 'rgba(94, 194, 166, 0.28)', text: '#d1fae5' },
    rose: { core: '#e07a8c', rim: '#f5b3bf', halo: 'rgba(224, 122, 140, 0.30)', text: '#ffe4e6' },
  };

  const state = {
    graph: { hubs: [], nodes: [], edges: [], activity: [] },
    stats: null,
    view: 'galaxy',
    activeCategory: null,
    selectedHash: null,
    hoveredId: null,
    searchQuery: '',
    searchMatches: new Map(),
    dockPage: 0,
    stagedFiles: [],
    camera: { x: 0, y: 0, zoom: 1 },
    layout: { hubs: new Map(), nodes: new Map() },
    ripples: [],
    drag: { mode: null, targetId: null, startX: 0, startY: 0, moved: false },
    reducedMotion: window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    lastFocusedEl: null,
  };

  const apiToken = (() => {
    const meta = document.querySelector('meta[name="axel-token"]');
    if (meta && meta.content) return meta.content;
    const urlParams = new URLSearchParams(window.location.search);
    return urlParams.get('token') || '';
  })();

  function apiFetch(url, options = {}) {
    const opts = { ...options };
    opts.headers = new Headers(opts.headers || {});
    if (apiToken && !opts.headers.has('Authorization')) {
      opts.headers.set('Authorization', `Bearer ${apiToken}`);
    }
    return fetch(url, opts);
  }

  const el = {
    shell: document.getElementById('nexusShell'),
    windowMeta: document.getElementById('windowMeta'),
    canvas: document.getElementById('galaxyCanvas'),
    tooltip: document.getElementById('canvasTooltip'),
    emptyGalaxy: document.getElementById('emptyGalaxy'),
    gridView: document.getElementById('gridView'),
    docGrid: document.getElementById('docGrid'),
    gridSubtitle: document.getElementById('gridSubtitle'),
    hudActiveFilter: document.getElementById('hudActiveFilter'),
    hudFilterText: document.getElementById('hudFilterText'),
    workspaceLabel: document.getElementById('workspaceLabel'),
    workspaceDropdown: document.getElementById('workspaceDropdown'),
    btnWorkspaceMenu: document.getElementById('btnWorkspaceMenu'),
    searchInput: document.getElementById('semanticSearchInput'),
    btnInputClear: document.getElementById('btnInputClear'),
    searchPopover: document.getElementById('searchResultsPopover'),
    searchSummary: document.getElementById('searchResultsSummary'),
    searchList: document.getElementById('searchResultsList'),
    visualChips: document.getElementById('visualChips'),
    activityScroll: document.getElementById('activityScroll'),
    activityDropdown: document.getElementById('activityDropdown'),
    dropDock: document.getElementById('dropDock'),
    dockCards: document.getElementById('dockCards'),
    dockDots: document.getElementById('dockDots'),
    dragOverlay: document.getElementById('dragOverlay'),
    fileInput: document.getElementById('hiddenFileInput'),
    inspectorDrawer: document.getElementById('inspectorDrawer'),
    modalDocCategory: document.getElementById('modalDocCategory'),
    modalDocTitle: document.getElementById('modalDocTitle'),
    modalDocPath: document.getElementById('modalDocPath'),
    modalMetaStrip: document.getElementById('modalMetaStrip'),
    modalDocKeywords: document.getElementById('modalDocKeywords'),
    modalDocPreview: document.getElementById('modalDocPreview'),
    btnDownloadRaw: document.getElementById('btnDownloadRaw'),
    btnFocusInGalaxy: document.getElementById('btnFocusInGalaxy'),
    btnDeleteDoc: document.getElementById('btnDeleteDoc'),
    deleteConfirmBar: document.getElementById('deleteConfirmBar'),
    btnConfirmDelete: document.getElementById('btnConfirmDelete'),
    btnCancelDelete: document.getElementById('btnCancelDelete'),
    settingsDrawer: document.getElementById('settingsDrawer'),
    telemetryGrid: document.getElementById('telemetryGrid'),
    toastStack: document.getElementById('toastStack'),
  };

  const ctx = el.canvas.getContext('2d');
  let width = 0;
  let height = 0;
  let dpr = Math.min(window.devicePixelRatio || 1, 2);
  let searchTimer = null;

  function escapeHtml(str) {
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function showToast(message) {
    const node = document.createElement('div');
    node.className = 'toast';
    node.textContent = message;
    el.toastStack.appendChild(node);
    setTimeout(() => node.remove(), 3400);
  }

  function formatBytes(bytes) {
    if (bytes == null) return 'Unknown size';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  }

  function formatRelativeTime(iso) {
    if (!iso) return 'Indexed';
    const ts = Date.parse(iso);
    if (Number.isNaN(ts)) return 'Indexed';
    const diffSec = Math.max(0, Math.floor((Date.now() - ts) / 1000));
    if (diffSec < 45) return 'Just now';
    if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
    if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
    return `${Math.floor(diffSec / 86400)}d ago`;
  }

  function fileTypeIconSvg(fileType, size = 16) {
    if (fileType === 'pdf') {
      return `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><path d="M9 15h6"></path><path d="M9 11h3"></path></svg>`;
    }
    if (fileType === 'image') {
      return `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`;
    }
    if (fileType === 'docx') {
      return `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="8" y1="13" x2="16" y2="13"></line><line x1="8" y1="17" x2="14" y2="17"></line></svg>`;
    }
    return `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path><polyline points="13 2 13 9 20 9"></polyline></svg>`;
  }

  function humanTitle(filename) {
    const base = String(filename || '').replace(/\.[^.]+$/, '').replace(/[_-]+/g, ' ').trim();
    if (!base) return filename;
    return base.replace(/\b\w/g, (c) => c.toUpperCase());
  }

  function resizeCanvas() {
    const rect = el.canvas.parentElement.getBoundingClientRect();
    width = Math.max(320, rect.width);
    height = Math.max(320, rect.height);
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    el.canvas.width = Math.floor(width * dpr);
    el.canvas.height = Math.floor(height * dpr);
    el.canvas.style.width = `${width}px`;
    el.canvas.style.height = `${height}px`;
    computeLayout(false);
  }

  function computeLayout(preserveExisting = true) {
    const hubs = state.graph.hubs || [];
    const nodes = state.graph.nodes || [];
    const compact = width < 560;
    const cx = width * (compact ? 0.5 : 0.49);
    const cy = height * (compact ? 0.38 : 0.38);
    const rx = compact ? Math.min(width * 0.27, 105) : Math.min(width * 0.30, 330);
    const ry = compact ? Math.min(height * 0.23, 165) : Math.min(height * 0.19, 150);

    const nextHubs = new Map();
    hubs.forEach((hub, idx) => {
      const existing = preserveExisting ? state.layout.hubs.get(hub.id) : null;
      if (existing) {
        nextHubs.set(hub.id, { ...existing, data: hub });
        return;
      }
      let x = cx;
      let y = cy;
      if (hubs.length > 1) {
        const angle = (idx / hubs.length) * Math.PI * 2 - Math.PI * 0.72;
        const ringScale = hubs.length >= 5 && idx === 0 ? (compact ? 0.12 : 0.24) : 1.0;
        x = cx + Math.cos(angle) * rx * ringScale;
        y = cy + Math.sin(angle) * ry * ringScale;
      }
      nextHubs.set(hub.id, {
        id: hub.id,
        x,
        y,
        baseX: x,
        baseY: y,
        radius: compact ? Math.min(44, 35 + hub.count * 2.2) : Math.min(54, 38 + hub.count * 3.2),
        phase: idx * 1.3,
        data: hub,
      });
    });
    state.layout.hubs = nextHubs;

    const grouped = new Map();
    nodes.forEach((n) => {
      const key = `hub:${n.category}`;
      if (!grouped.has(key)) grouped.set(key, []);
      grouped.get(key).push(n);
    });

    const nextNodes = new Map();
    grouped.forEach((groupNodes, hubId) => {
      const hubPos = nextHubs.get(hubId) || { x: cx, y: cy, radius: 42 };
      const dx = hubPos.x - cx;
      const dy = hubPos.y - cy;
      const outwardAngle = Math.hypot(dx, dy) > 12 ? Math.atan2(dy, dx) : -Math.PI * 0.5;
      groupNodes.forEach((node, idx) => {
        const existing = preserveExisting ? state.layout.nodes.get(node.hash) : null;
        if (existing) {
          nextNodes.set(node.hash, { ...existing, data: node });
          return;
        }
        const spread = (Math.PI * 2) / Math.max(3, groupNodes.length);
        const angle = outwardAngle + (idx - (groupNodes.length - 1) / 2) * spread;
        const orbit = compact
          ? hubPos.radius + 34 + (idx % 2) * 10
          : hubPos.radius + 80 + (idx % 2) * 20;
        const rawX = hubPos.x + Math.cos(angle) * orbit;
        const x = compact ? Math.max(34, Math.min(width - 34, rawX)) : rawX;
        const y = hubPos.y + Math.sin(angle) * orbit;
        nextNodes.set(node.hash, {
          id: node.hash,
          hubId,
          x,
          y,
          baseX: x,
          baseY: y,
          orbitAngle: angle,
          orbitRadius: orbit,
          radius: compact ? 15 : 18,
          phase: idx * 0.9,
          data: node,
        });
      });
    });
    state.layout.nodes = nextNodes;
  }

  function worldToScreen(wx, wy) {
    return {
      x: (wx - width / 2) * state.camera.zoom + width / 2 + state.camera.x,
      y: (wy - height / 2) * state.camera.zoom + height / 2 + state.camera.y,
    };
  }

  function screenToWorld(sx, sy) {
    return {
      x: (sx - width / 2 - state.camera.x) / state.camera.zoom + width / 2,
      y: (sy - height / 2 - state.camera.y) / state.camera.zoom + height / 2,
    };
  }

  function isNodeHighlighted(node) {
    if (state.searchMatches.size > 0) {
      return state.searchMatches.has(node.hash);
    }
    if (state.activeCategory) {
      return node.category === state.activeCategory;
    }
    return true;
  }

  function drawBraidedFilament(p1, p2, colorA, colorB, alpha, widthPx, strands, timeSec, animatePulse, seed) {
    const dx = p2.x - p1.x;
    const dy = p2.y - p1.y;
    const dist = Math.hypot(dx, dy) || 1;
    const nx = -dy / dist;
    const ny = dx / dist;

    ctx.save();
    ctx.globalCompositeOperation = 'lighter';
    const grad = ctx.createLinearGradient(p1.x, p1.y, p2.x, p2.y);
    grad.addColorStop(0, colorA);
    grad.addColorStop(1, colorB);

    for (let s = 0; s < strands; s++) {
      const spread = strands === 1 ? 0 : (s - (strands - 1) / 2) * 14;
      const curveOffset = Math.min(52, dist * 0.18) * (seed % 2 === 0 ? 1 : -1) + spread;
      const cx = (p1.x + p2.x) / 2 + nx * curveOffset;
      const cy = (p1.y + p2.y) / 2 + ny * curveOffset;

      ctx.globalAlpha = s === Math.floor(strands / 2) ? Math.min(0.85, alpha * 1.25) : alpha * 0.48;
      ctx.beginPath();
      ctx.moveTo(p1.x, p1.y);
      ctx.quadraticCurveTo(cx, cy, p2.x, p2.y);
      ctx.strokeStyle = grad;
      ctx.lineWidth = s === Math.floor(strands / 2) ? widthPx : Math.max(0.8, widthPx * 0.65);
      ctx.stroke();

      if (animatePulse && s === 0 && !state.reducedMotion) {
        const t = (timeSec * 0.24 + seed * 0.19) % 1;
        const inv = 1 - t;
        const px = inv * inv * p1.x + 2 * inv * t * cx + t * t * p2.x;
        const py = inv * inv * p1.y + 2 * inv * t * cy + t * t * p2.y;
        ctx.globalAlpha = 0.9;
        ctx.beginPath();
        ctx.arc(px, py, 2.4, 0, Math.PI * 2);
        ctx.fillStyle = '#f6c896';
        ctx.fill();
      }
    }
    ctx.restore();
  }

  function drawNodeGlyph(x, y, fileType, color) {
    ctx.save();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    if (fileType === 'image') {
      ctx.strokeRect(x - 6, y - 5, 12, 10);
      ctx.beginPath();
      ctx.arc(x - 2.5, y - 1.5, 1.2, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x - 5, y + 4);
      ctx.lineTo(x - 1, y);
      ctx.lineTo(x + 2, y + 2.5);
      ctx.lineTo(x + 5, y - 0.5);
      ctx.stroke();
    } else {
      ctx.beginPath();
      ctx.moveTo(x - 4.5, y - 6);
      ctx.lineTo(x + 1.5, y - 6);
      ctx.lineTo(x + 4.5, y - 3);
      ctx.lineTo(x + 4.5, y + 6);
      ctx.lineTo(x - 4.5, y + 6);
      ctx.closePath();
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x - 2.2, y);
      ctx.lineTo(x + 2.2, y);
      ctx.moveTo(x - 2.2, y + 3);
      ctx.lineTo(x + 1.5, y + 3);
      ctx.stroke();
    }
    ctx.restore();
  }

  function renderFrame(nowMs) {
    const timeSec = nowMs * 0.001;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);

    if (state.graph.nodes.length === 0) {
      requestAnimationFrame(renderFrame);
      return;
    }

    if (!state.reducedMotion) {
      state.layout.hubs.forEach((hub) => {
        if (state.drag.targetId !== hub.id) {
          hub.x = hub.baseX + Math.sin(timeSec * 0.48 + hub.phase) * 3.2;
          hub.y = hub.baseY + Math.cos(timeSec * 0.40 + hub.phase) * 2.8;
        }
      });
      state.layout.nodes.forEach((node) => {
        if (state.drag.targetId !== node.id) {
          node.x = node.baseX + Math.cos(timeSec * 0.62 + node.phase) * 2.4;
          node.y = node.baseY + Math.sin(timeSec * 0.58 + node.phase) * 2.4;
        }
      });
    }

    // Draw braided filaments & radial spokes
    state.graph.edges.forEach((edge, idx) => {
      const src = state.layout.hubs.get(edge.source) || state.layout.nodes.get(edge.source);
      const dst = state.layout.hubs.get(edge.target) || state.layout.nodes.get(edge.target);
      if (!src || !dst) return;

      const p1 = worldToScreen(src.x, src.y);
      const p2 = worldToScreen(dst.x, dst.y);

      if (edge.kind === 'backbone') {
        drawBraidedFilament(p1, p2, 'rgba(229, 154, 84, 0.65)', 'rgba(123, 140, 255, 0.65)', 0.52, 1.5, 3, timeSec, false, idx);
      } else if (edge.kind === 'pipeline') {
        drawBraidedFilament(p1, p2, '#e59a54', '#7b8cff', 0.78, 2.0, 3, timeSec, true, idx);
      } else if (edge.kind === 'semantic') {
        const active = state.hoveredId === edge.source || state.hoveredId === edge.target ||
          state.selectedHash === edge.source || state.selectedHash === edge.target;
        drawBraidedFilament(p1, p2, active ? '#f6c896' : '#e59a54', '#7b8cff', active ? 0.92 : 0.58, active ? 2.2 : 1.4, 2, timeSec, true, idx);
      } else {
        const nodeObj = state.layout.nodes.get(edge.target);
        const lit = nodeObj ? isNodeHighlighted(nodeObj.data) : true;
        const accent = ACCENTS[src.data?.accent || 'violet'] || ACCENTS.violet;
        drawBraidedFilament(p1, p2, accent.core, 'rgba(148, 163, 184, 0.45)', lit ? 0.48 : 0.12, 1.25, 1, timeSec, lit && state.hoveredId === edge.target, idx);
      }
    });

    // Expanding click ripples
    state.ripples = state.ripples.filter((r) => nowMs - r.start < 1100);
    for (const r of state.ripples) {
      const progress = (nowMs - r.start) / 1100;
      const pos = worldToScreen(r.x, r.y);
      const rad = (r.baseRadius + progress * 90) * state.camera.zoom;
      ctx.save();
      ctx.beginPath();
      ctx.arc(pos.x, pos.y, rad, 0, Math.PI * 2);
      ctx.strokeStyle = r.color;
      ctx.globalAlpha = (1 - progress) * 0.55;
      ctx.lineWidth = 1.8;
      ctx.stroke();
      ctx.restore();
    }

    // Volumetric Luminous Hubs
    state.layout.hubs.forEach((hub) => {
      const pos = worldToScreen(hub.x, hub.y);
      const r = hub.radius * state.camera.zoom;
      const accent = ACCENTS[hub.data.accent] || ACCENTS.violet;
      const isActive = state.activeCategory === hub.data.category || state.hoveredId === hub.id;
      const dimmed = state.activeCategory && state.activeCategory !== hub.data.category && state.searchMatches.size === 0;

      ctx.save();
      ctx.globalAlpha = dimmed ? 0.32 : 1;

      // Soft atmospheric halo
      const halo = ctx.createRadialGradient(pos.x, pos.y, r * 0.2, pos.x, pos.y, r * 2.1);
      halo.addColorStop(0, accent.halo);
      halo.addColorStop(0.55, 'rgba(17, 24, 42, 0.22)');
      halo.addColorStop(1, 'rgba(11, 16, 29, 0)');
      ctx.fillStyle = halo;
      ctx.beginPath();
      ctx.arc(pos.x, pos.y, r * 2.1, 0, Math.PI * 2);
      ctx.fill();

      // Concentric ripple rings
      const ringCount = isActive ? 3 : 2;
      for (let i = 1; i <= ringCount; i++) {
        const pulse = state.reducedMotion ? 0 : Math.sin(timeSec * 1.4 - i * 0.8) * 2;
        ctx.beginPath();
        ctx.arc(pos.x, pos.y, r + i * 14 * state.camera.zoom + pulse, 0, Math.PI * 2);
        ctx.strokeStyle = accent.rim;
        ctx.globalAlpha = (dimmed ? 0.1 : 0.24) / i;
        ctx.lineWidth = 1.1;
        ctx.stroke();
      }

      // Frosted glass sphere core
      ctx.globalAlpha = dimmed ? 0.35 : 1;
      const sphereGrad = ctx.createRadialGradient(pos.x - r * 0.32, pos.y - r * 0.32, r * 0.1, pos.x, pos.y, r);
      sphereGrad.addColorStop(0, 'rgba(255, 255, 255, 0.16)');
      sphereGrad.addColorStop(0.45, 'rgba(22, 31, 54, 0.88)');
      sphereGrad.addColorStop(1, 'rgba(11, 16, 29, 0.96)');
      ctx.beginPath();
      ctx.arc(pos.x, pos.y, r, 0, Math.PI * 2);
      ctx.fillStyle = sphereGrad;
      ctx.fill();

      ctx.lineWidth = isActive ? 2.2 : 1.5;
      ctx.strokeStyle = accent.core;
      ctx.stroke();

      // Directional top-left specular rim arc
      ctx.beginPath();
      ctx.arc(pos.x, pos.y, r - 1.5, Math.PI * 1.08, Math.PI * 1.68);
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.42)';
      ctx.lineWidth = 1.4;
      ctx.stroke();

      const hubFontPx = width < 560 ? Math.max(11, Math.round(11.5 * state.camera.zoom)) : Math.max(12, Math.round(13.5 * state.camera.zoom));
      ctx.fillStyle = '#eef2ff';
      ctx.font = `600 ${hubFontPx}px "IBM Plex Sans", sans-serif`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(hub.data.category, pos.x, pos.y - 6 * state.camera.zoom);

      ctx.fillStyle = accent.text;
      ctx.font = `500 ${Math.max(10, Math.round(11 * state.camera.zoom))}px "JetBrains Mono", monospace`;
      ctx.fillText(`${hub.data.count} ${hub.data.count === 1 ? 'doc' : 'docs'}`, pos.x, pos.y + 11 * state.camera.zoom);
      ctx.restore();
    });

    // Circular Glass Medallions + Frosted Preview Capsules
    state.layout.nodes.forEach((node) => {
      const pos = worldToScreen(node.x, node.y);
      const r = node.radius * state.camera.zoom;
      const lit = isNodeHighlighted(node.data);
      const isSelected = state.selectedHash === node.id || state.hoveredId === node.id;
      const hub = state.layout.hubs.get(node.hubId);
      const accent = ACCENTS[hub?.data?.accent || 'violet'] || ACCENTS.violet;
      const ringColor = isSelected ? '#e59a54' : accent.core;

      ctx.save();
      ctx.globalAlpha = lit ? 1 : 0.22;

      if (isSelected || state.searchMatches.has(node.id)) {
        ctx.beginPath();
        ctx.arc(pos.x, pos.y, r + 6, 0, Math.PI * 2);
        ctx.strokeStyle = 'rgba(229, 154, 84, 0.55)';
        ctx.lineWidth = 1.4;
        ctx.stroke();
      }

      const medGrad = ctx.createRadialGradient(pos.x - r * 0.3, pos.y - r * 0.3, 2, pos.x, pos.y, r);
      medGrad.addColorStop(0, isSelected ? 'rgba(229, 154, 84, 0.32)' : 'rgba(255, 255, 255, 0.12)');
      medGrad.addColorStop(1, 'rgba(15, 22, 40, 0.94)');
      ctx.beginPath();
      ctx.arc(pos.x, pos.y, r, 0, Math.PI * 2);
      ctx.fillStyle = medGrad;
      ctx.fill();
      ctx.lineWidth = isSelected ? 2 : 1.4;
      ctx.strokeStyle = ringColor;
      ctx.stroke();

      drawNodeGlyph(pos.x, pos.y, node.data.file_type, isSelected ? '#fde6c4' : '#e2e8f0');

      // Frosted preview capsule with human-readable title + subtitle
      if (lit && state.camera.zoom >= 0.72) {
        const compact = width < 560;
        const title = humanTitle(node.data.file);
        const maxLen = compact ? 14 : 20;
        const shortTitle = title.length > maxLen ? `${title.slice(0, maxLen - 2)}…` : title;
        const sub = `${node.data.category} · ${node.data.file_type.toUpperCase()}`;

        ctx.font = compact ? '600 10.5px "IBM Plex Sans", sans-serif' : '600 11.5px "IBM Plex Sans", sans-serif';
        const titleW = ctx.measureText(shortTitle).width;
        ctx.font = compact ? '500 9px "JetBrains Mono", monospace' : '500 10px "JetBrains Mono", monospace';
        const subW = ctx.measureText(sub).width;

        const pillW = Math.max(titleW, subW) + (compact ? 14 : 20);
        const pillH = compact ? 28 : 34;
        const nearBottomDock = pos.y + r + pillH + 14 > height - (compact ? 140 : 126);
        let px = pos.x - pillW / 2;
        let py;
        if (nearBottomDock) {
          px = pos.x + r + 8;
          py = pos.y - pillH / 2;
        } else {
          const placeAbove = hub && node.y < hub.y - 4;
          py = placeAbove ? pos.y - r - pillH - 6 : pos.y + r + 6;
        }
        px = Math.max(8, Math.min(width - pillW - 8, px));

        ctx.beginPath();
        ctx.roundRect(px, py, pillW, pillH, 10);
        ctx.fillStyle = isSelected ? 'rgba(229, 154, 84, 0.20)' : 'rgba(14, 20, 36, 0.88)';
        ctx.fill();
        ctx.lineWidth = 1;
        ctx.strokeStyle = isSelected ? 'rgba(229, 154, 84, 0.65)' : 'rgba(226, 232, 240, 0.14)';
        ctx.stroke();

        ctx.fillStyle = isSelected ? '#fde6c4' : '#eef2ff';
        ctx.font = compact ? '600 10.5px "IBM Plex Sans", sans-serif' : '600 11.5px "IBM Plex Sans", sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(shortTitle, px + pillW / 2, py + (compact ? 9.5 : 11.5));

        ctx.fillStyle = '#94a3b8';
        ctx.font = compact ? '500 9px "JetBrains Mono", monospace' : '500 10px "JetBrains Mono", monospace';
        ctx.fillText(sub, px + pillW / 2, py + (compact ? 20.5 : 24.5));
      }
      ctx.restore();
    });

    requestAnimationFrame(renderFrame);
  }

  function hitTest(sx, sy) {
    const world = screenToWorld(sx, sy);
    for (const node of state.layout.nodes.values()) {
      if (Math.hypot(world.x - node.x, world.y - node.y) <= node.radius + 6) {
        return { kind: 'node', item: node };
      }
    }
    for (const hub of state.layout.hubs.values()) {
      if (Math.hypot(world.x - hub.x, world.y - hub.y) <= hub.radius + 4) {
        return { kind: 'hub', item: hub };
      }
    }
    return null;
  }

  function updateTooltip(hit, sx, sy) {
    if (!hit) {
      el.tooltip.classList.add('hidden');
      return;
    }
    if (hit.kind === 'hub') {
      const h = hit.item.data;
      el.tooltip.innerHTML = `
        <div class="tooltip-title">${escapeHtml(h.category)} Cluster</div>
        <div class="tooltip-meta">${h.count} indexed ${h.count === 1 ? 'document' : 'documents'} &middot; Click to filter</div>
        <div class="tooltip-tags">${h.top_keywords.map((k) => `<span class="tooltip-tag">${escapeHtml(k)}</span>`).join('')}</div>
      `;
    } else {
      const n = hit.item.data;
      el.tooltip.innerHTML = `
        <div class="tooltip-title">${escapeHtml(n.file)}</div>
        <div class="tooltip-meta">${escapeHtml(n.category)} &middot; ${escapeHtml(n.file_type.toUpperCase())} &middot; ${formatBytes(n.size_bytes)}</div>
        <div class="tooltip-tags">${n.keywords.slice(0, 5).map((k) => `<span class="tooltip-tag">${escapeHtml(k)}</span>`).join('')}</div>
      `;
    }
    const left = Math.min(width - 280, Math.max(12, sx + 16));
    const top = Math.min(height - 120, Math.max(12, sy - 12));
    el.tooltip.style.left = `${left}px`;
    el.tooltip.style.top = `${top}px`;
    el.tooltip.classList.remove('hidden');
  }

  function renderVisualChips() {
    const hubs = state.graph.hubs || [];
    const chips = [
      { label: 'All Nodes', category: null },
      ...hubs.map((h) => ({ label: `${h.category} (${h.count})`, category: h.category })),
    ];
    el.visualChips.innerHTML = chips
      .map((c) => {
        const active = state.activeCategory === c.category;
        return `<button type="button" class="visual-chip ${active ? 'active' : ''}" data-cat="${c.category ? escapeHtml(c.category) : ''}">${escapeHtml(c.label)}</button>`;
      })
      .join('');

    el.visualChips.querySelectorAll('.visual-chip').forEach((btn) => {
      btn.addEventListener('click', () => {
        const cat = btn.dataset.cat || null;
        setCategoryFilter(state.activeCategory === cat ? null : cat);
      });
    });
  }

  function renderWorkspaceDropdown() {
    const hubs = state.graph.hubs || [];
    const total = state.graph.nodes.length;
    const rows = [
      `<button type="button" class="dropdown-item ${!state.activeCategory ? 'active' : ''}" data-cat=""><span>All Clusters</span><span class="dropdown-count">${total}</span></button>`,
      ...hubs.map((h) => `<button type="button" class="dropdown-item ${state.activeCategory === h.category ? 'active' : ''}" data-cat="${escapeHtml(h.category)}"><span>${escapeHtml(h.category)}</span><span class="dropdown-count">${h.count}</span></button>`),
    ];
    el.workspaceDropdown.innerHTML = rows.join('');
    el.workspaceDropdown.querySelectorAll('.dropdown-item').forEach((btn) => {
      btn.addEventListener('click', () => {
        setCategoryFilter(btn.dataset.cat || null);
        el.workspaceDropdown.classList.add('hidden');
        el.btnWorkspaceMenu.setAttribute('aria-expanded', 'false');
      });
    });
  }

  function renderActivityStream() {
    const items = state.graph.activity || [];
    if (items.length === 0) {
      el.activityScroll.innerHTML = '<p style="color:var(--text-muted);font-size:13px;padding:8px;">No indexed activity yet. Drop files to begin.</p>';
      return;
    }

    const now = Date.now();
    const today = [];
    const earlier = [];
    items.forEach((item) => {
      const ts = item.indexed_at ? Date.parse(item.indexed_at) : 0;
      if (ts && now - ts < 86400 * 1000) today.push(item);
      else earlier.push(item);
    });

    const renderGroup = (title, list) => {
      if (list.length === 0) return '';
      return `
        <div class="timeline-group">
          <div class="timeline-group-label"><span class="timeline-group-dot"></span><span>${title}</span></div>
          <div class="timeline-list">
            ${list.map((item) => `
              <button type="button" class="activity-card ${state.selectedHash === item.hash ? 'active' : ''}" data-hash="${escapeHtml(item.hash)}">
                <span class="activity-avatar">${fileTypeIconSvg(item.file_type, 15)}</span>
                <span class="activity-info">
                  <div class="activity-title">${escapeHtml(humanTitle(item.file))}</div>
                  <div class="activity-sub">${escapeHtml(item.category)} &middot; ${formatRelativeTime(item.indexed_at)}</div>
                </span>
              </button>
            `).join('')}
          </div>
        </div>
      `;
    };

    el.activityScroll.innerHTML = renderGroup('Today', today) + renderGroup('Past timelines', earlier);
    el.activityScroll.querySelectorAll('.activity-card').forEach((card) => {
      const hash = card.dataset.hash;
      card.addEventListener('mouseenter', () => { state.hoveredId = hash; });
      card.addEventListener('mouseleave', () => { if (state.hoveredId === hash) state.hoveredId = null; });
      card.addEventListener('click', () => openDocumentModal(hash));
    });
  }

  function renderDropDock() {
    const source = state.stagedFiles.length > 0
      ? state.stagedFiles.map((f) => ({ label: f.name, type: 'staged', hash: null }))
      : (state.graph.activity || []).map((a) => ({ label: humanTitle(a.file), type: a.file_type, hash: a.hash }));

    const pageSize = 3;
    const totalPages = Math.max(1, Math.ceil(source.length / pageSize));
    state.dockPage = ((state.dockPage % totalPages) + totalPages) % totalPages;
    const slice = source.slice(state.dockPage * pageSize, state.dockPage * pageSize + pageSize);
    const slots = ['left', 'center', 'right'];

    if (slice.length === 0) {
      el.dockCards.innerHTML = `
        <button type="button" class="dock-card left" data-upload="1" aria-label="Upload PDF">${fileTypeIconSvg('pdf', 22)}<span class="dock-card-tooltip">Upload PDF</span></button>
        <button type="button" class="dock-card center" data-upload="1" aria-label="Upload Document">${fileTypeIconSvg('docx', 24)}<span class="dock-card-tooltip">Drop Files</span></button>
        <button type="button" class="dock-card right" data-upload="1" aria-label="Upload Image">${fileTypeIconSvg('image', 22)}<span class="dock-card-tooltip">OCR Image</span></button>
      `;
    } else {
      el.dockCards.innerHTML = slice.map((item, idx) => {
        const slot = slice.length === 1 ? 'center' : slots[idx] || 'right';
        return `
          <button type="button" class="dock-card ${slot}" ${item.hash ? `data-hash="${escapeHtml(item.hash)}"` : 'data-upload="1"'} aria-label="${escapeHtml(item.label)}">
            ${fileTypeIconSvg(item.type, slot === 'center' ? 24 : 20)}
            <span class="dock-card-tooltip">${escapeHtml(item.label)}</span>
          </button>
        `;
      }).join('');
    }

    el.dockCards.querySelectorAll('.dock-card').forEach((card) => {
      card.addEventListener('click', () => {
        if (card.dataset.hash) openDocumentModal(card.dataset.hash);
        else el.fileInput.click();
      });
    });

    el.dockDots.innerHTML = Array.from({ length: totalPages }, (_, i) =>
      `<span class="dock-dot ${i === state.dockPage ? 'active' : ''}"></span>`
    ).join('');
  }

  function renderGridView() {
    const nodes = (state.graph.nodes || []).filter((n) => {
      if (state.searchMatches.size > 0 && !state.searchMatches.has(n.hash)) return false;
      if (state.activeCategory && n.category !== state.activeCategory) return false;
      return true;
    });

    el.gridSubtitle.textContent = state.activeCategory
      ? `Filtered by ${state.activeCategory} (${nodes.length} documents)`
      : `Showing ${nodes.length} indexed documents`;

    el.docGrid.innerHTML = nodes.map((n) => `
      <article class="doc-card" tabindex="0" data-hash="${escapeHtml(n.hash)}">
        <div class="doc-card-top">
          <span class="doc-cat-pill">${escapeHtml(n.category)}</span>
          <span class="doc-type-mono">${escapeHtml(n.file_type)} &middot; ${formatBytes(n.size_bytes)}</span>
        </div>
        <div class="doc-card-title">${escapeHtml(humanTitle(n.file))}</div>
        <div class="doc-card-tags">
          ${n.keywords.slice(0, 5).map((k) => `<span class="doc-tag">${escapeHtml(k)}</span>`).join('')}
        </div>
      </article>
    `).join('');

    el.docGrid.querySelectorAll('.doc-card').forEach((card) => {
      const open = () => openDocumentModal(card.dataset.hash);
      card.addEventListener('click', open);
      card.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); open(); } });
    });
  }

  function setCategoryFilter(category) {
    state.activeCategory = category;
    el.workspaceLabel.textContent = category ? `Cluster: ${category}` : 'High-Tech File Manager';
    if (category) {
      el.hudFilterText.textContent = `Cluster: ${category}`;
      el.hudActiveFilter.classList.remove('hidden');
    } else {
      el.hudActiveFilter.classList.add('hidden');
    }
    renderVisualChips();
    renderWorkspaceDropdown();
    if (state.view === 'grid') renderGridView();
  }

  function setView(view) {
    state.view = view;
    document.getElementById('navGalaxy').classList.toggle('active', view === 'galaxy');
    document.getElementById('navGrid').classList.toggle('active', view === 'grid');
    el.gridView.classList.toggle('hidden', view !== 'grid');
    if (view === 'grid') renderGridView();
  }

  async function fetchGraphAndStats() {
    try {
      const [graphRes, statsRes] = await Promise.all([apiFetch('/api/graph'), apiFetch('/api/stats')]);
      if (!graphRes.ok) throw new Error('Failed to load graph');
      state.graph = await graphRes.json();
      if (statsRes.ok) state.stats = await statsRes.json();

      if (el.windowMeta) {
        el.windowMeta.textContent = `${state.graph.nodes.length} docs · ${state.graph.hubs.length} clusters`;
      }
      el.emptyGalaxy.classList.toggle('hidden', state.graph.nodes.length > 0);

      computeLayout(true);
      renderVisualChips();
      renderWorkspaceDropdown();
      renderActivityStream();
      renderDropDock();
      if (state.view === 'grid') renderGridView();
    } catch (err) {
      showToast(`Sync error: ${err.message}`);
    }
  }

  async function runSemanticSearch(query) {
    const q = query.trim();
    state.searchQuery = q;
    el.btnInputClear.classList.toggle('hidden', q.length === 0);

    if (!q) {
      state.searchMatches.clear();
      el.searchPopover.classList.add('hidden');
      if (state.view === 'grid') renderGridView();
      return;
    }

    try {
      const res = await apiFetch(`/api/search?q=${encodeURIComponent(q)}&limit=12`);
      const data = await res.json();
      state.searchMatches = new Map(data.results.map((r) => [r.item.hash, r]));
      el.searchSummary.textContent = `${data.total} semantic ${data.total === 1 ? 'match' : 'matches'} for "${q}"`;

      if (data.results.length === 0) {
        el.searchList.innerHTML = '<div style="padding:10px;color:var(--text-muted);font-size:13px;">No matching documents found. Try another keyword or category.</div>';
      } else {
        el.searchList.innerHTML = data.results.map((r) => `
          <button type="button" class="search-result-row" data-hash="${escapeHtml(r.item.hash)}">
            <div style="min-width:0;">
              <div class="search-result-name">${escapeHtml(humanTitle(r.item.file))}</div>
              <div class="search-result-meta">${escapeHtml(r.item.category)} &middot; ${escapeHtml(r.match_type.toUpperCase())}</div>
            </div>
            <span class="search-score-pill">${Math.round(r.score * 100)}%</span>
          </button>
        `).join('');
        el.searchList.querySelectorAll('.search-result-row').forEach((row) => {
          row.addEventListener('click', () => openDocumentModal(row.dataset.hash));
        });
      }
      el.searchPopover.classList.remove('hidden');
      if (state.view === 'grid') renderGridView();
    } catch (err) {
      showToast(`Search failed: ${err.message}`);
    }
  }

  async function openDocumentModal(hash) {
    state.lastFocusedEl = document.activeElement;
    state.selectedHash = hash;
    renderActivityStream();

    try {
      const res = await apiFetch(`/api/documents/${encodeURIComponent(hash)}`);
      if (!res.ok) throw new Error('Document not found');
      const data = await res.json();
      const item = data.item;

      el.modalDocCategory.textContent = item.category;
      el.modalDocTitle.textContent = humanTitle(item.file);
      el.modalDocPath.textContent = item.path;
      el.modalMetaStrip.innerHTML = `
        <span class="meta-pill">Type: ${escapeHtml(data.file_type.toUpperCase())}</span>
        <span class="meta-pill">Size: ${formatBytes(data.size_bytes)}</span>
        <span class="meta-pill">SHA-256: ${escapeHtml(item.hash.slice(0, 12))}…</span>
        <span class="meta-pill">${data.file_exists ? 'File on disk' : 'Missing on disk'}</span>
      `;
      el.modalDocKeywords.innerHTML = item.keywords.map((k) =>
        `<button type="button" class="modal-kw-chip" data-kw="${escapeHtml(k)}">${escapeHtml(k)}</button>`
      ).join('');
      el.modalDocKeywords.querySelectorAll('.modal-kw-chip').forEach((chip) => {
        chip.addEventListener('click', () => {
          closeModals();
          el.searchInput.value = chip.dataset.kw;
          runSemanticSearch(chip.dataset.kw);
        });
      });

      el.modalDocPreview.textContent = data.text_preview || '(No extractable text content)';
      let rawUrl = `/api/documents/${encodeURIComponent(hash)}/raw`;
      if (apiToken) rawUrl += `?token=${encodeURIComponent(apiToken)}`;
      el.btnDownloadRaw.href = rawUrl;
      el.btnDownloadRaw.classList.toggle('hidden', !data.file_exists);
      el.btnDeleteDoc.dataset.hash = hash;
      el.btnFocusInGalaxy.dataset.hash = hash;
      el.btnDeleteDoc.classList.remove('hidden');
      el.deleteConfirmBar.classList.add('hidden');

      el.settingsDrawer.classList.add('hidden');
      el.inspectorDrawer.classList.remove('hidden');
      document.getElementById('btnCloseDocModal').focus();
    } catch (err) {
      showToast(err.message);
    }
  }

  function openSettingsModal() {
    state.lastFocusedEl = document.activeElement;
    const s = state.stats || {};
    el.telemetryGrid.innerHTML = `
      <div class="telemetry-card"><div class="telemetry-label">Indexed Documents</div><div class="telemetry-value">${s.total_documents ?? state.graph.nodes.length}</div></div>
      <div class="telemetry-card"><div class="telemetry-label">AI Acceleration</div><div class="telemetry-value">${s.groq_enabled ? 'Groq Cloud + Local Rules' : 'Local Rule Engine'}</div></div>
      <div class="telemetry-card"><div class="telemetry-label">Active Model</div><div class="telemetry-value">${escapeHtml(s.active_model || 'llama-3.3-70b-versatile')}</div></div>
      <div class="telemetry-card"><div class="telemetry-label">OCR Engine</div><div class="telemetry-value">${s.tesseract_ready ? 'Tesseract Ready' : 'Filename Fallback'}</div></div>
      <div class="telemetry-card" style="grid-column:1/-1;"><div class="telemetry-label">Watched Folder</div><div class="telemetry-value">${escapeHtml(s.input_folder || './documents')}</div></div>
    `;
    el.inspectorDrawer.classList.add('hidden');
    el.settingsDrawer.classList.remove('hidden');
    document.getElementById('btnCloseSettingsModal').focus();
  }

  function closeModals() {
    el.inspectorDrawer.classList.add('hidden');
    el.settingsDrawer.classList.add('hidden');
    el.btnDeleteDoc.classList.remove('hidden');
    el.deleteConfirmBar.classList.add('hidden');
    if (state.lastFocusedEl && typeof state.lastFocusedEl.focus === 'function') {
      state.lastFocusedEl.focus();
      state.lastFocusedEl = null;
    }
  }

  async function triggerScan() {
    try {
      const res = await apiFetch('/api/scan', { method: 'POST' });
      const data = await res.json();
      showToast(`Scan complete: ${data.indexed} indexed, ${data.skipped} unchanged`);
      await fetchGraphAndStats();
    } catch (err) {
      showToast(`Scan failed: ${err.message}`);
    }
  }

  async function uploadFiles(fileList) {
    const files = Array.from(fileList || []);
    if (files.length === 0) return;
    state.stagedFiles = files;
    renderDropDock();

    const form = new FormData();
    files.forEach((f) => form.append('file', f));

    try {
      const res = await apiFetch('/api/upload', { method: 'POST', body: form });
      if (!res.ok) throw new Error(await res.text());
      const data = await res.json();
      showToast(`Indexed ${data.count} ${data.count === 1 ? 'document' : 'documents'} into Galaxy`);
      state.stagedFiles = [];
      await fetchGraphAndStats();
      if (data.uploaded && data.uploaded[0]) {
        state.selectedHash = data.uploaded[0].hash;
        const node = state.layout.nodes.get(data.uploaded[0].hash);
        if (node) state.ripples.push({ x: node.x, y: node.y, baseRadius: 18, color: '#e59a54', start: performance.now() });
      }
    } catch (err) {
      state.stagedFiles = [];
      renderDropDock();
      showToast(`Upload failed: ${err.message}`);
    }
  }

  function connectSse() {
    let sseUrl = '/api/events';
    if (apiToken) sseUrl += `?token=${encodeURIComponent(apiToken)}`;
    const es = new EventSource(sseUrl);
    es.onmessage = (evt) => {
      try {
        const payload = JSON.parse(evt.data);
        if (payload.type === 'index_updated') {
          fetchGraphAndStats();
          if (payload.file) showToast(`Live Watcher indexed: ${payload.file}`);
        }
      } catch (_) {}
    };
  }

  function bindEvents() {
    window.addEventListener('resize', resizeCanvas);

    // Canvas pointer interactions
    el.canvas.addEventListener('pointerdown', (e) => {
      const rect = el.canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;
      const hit = hitTest(sx, sy);
      state.drag = {
        mode: hit ? hit.kind : 'pan',
        targetId: hit ? hit.item.id : null,
        startX: sx,
        startY: sy,
        lastX: sx,
        lastY: sy,
        moved: false,
      };
      el.canvas.classList.add('dragging');
    });

    window.addEventListener('pointermove', (e) => {
      const rect = el.canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;

      if (state.drag.mode) {
        const dx = sx - state.drag.lastX;
        const dy = sy - state.drag.lastY;
        if (Math.hypot(sx - state.drag.startX, sy - state.drag.startY) > 4) state.drag.moved = true;
        state.drag.lastX = sx;
        state.drag.lastY = sy;

        if (state.drag.mode === 'pan') {
          state.camera.x += dx;
          state.camera.y += dy;
        } else if (state.drag.mode === 'hub') {
          const hub = state.layout.hubs.get(state.drag.targetId);
          if (hub) {
            hub.baseX += dx / state.camera.zoom;
            hub.baseY += dy / state.camera.zoom;
            hub.x = hub.baseX;
            hub.y = hub.baseY;
          }
        } else if (state.drag.mode === 'node') {
          const node = state.layout.nodes.get(state.drag.targetId);
          if (node) {
            node.baseX += dx / state.camera.zoom;
            node.baseY += dy / state.camera.zoom;
            node.x = node.baseX;
            node.y = node.baseY;
          }
        }
        return;
      }

      if (e.target === el.canvas) {
        const hit = hitTest(sx, sy);
        state.hoveredId = hit ? hit.item.id : null;
        el.canvas.classList.toggle('pointer', Boolean(hit));
        updateTooltip(hit, sx, sy);
      } else {
        el.tooltip.classList.add('hidden');
      }
    });

    window.addEventListener('pointerup', (e) => {
      if (!state.drag.mode) return;
      const wasClick = !state.drag.moved;
      const rect = el.canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;
      state.drag.mode = null;
      state.drag.targetId = null;
      el.canvas.classList.remove('dragging');

      if (wasClick && e.target === el.canvas) {
        const hit = hitTest(sx, sy);
        if (hit?.kind === 'hub') {
          const hub = hit.item;
          const accent = ACCENTS[hub.data.accent] || ACCENTS.violet;
          state.ripples.push({ x: hub.x, y: hub.y, baseRadius: hub.radius, color: accent.core, start: performance.now() });
          setCategoryFilter(state.activeCategory === hub.data.category ? null : hub.data.category);
        } else if (hit?.kind === 'node') {
          openDocumentModal(hit.item.id);
        }
      }
    });

    el.canvas.addEventListener('wheel', (e) => {
      e.preventDefault();
      const factor = e.deltaY < 0 ? 1.08 : 0.92;
      state.camera.zoom = Math.min(2.2, Math.max(0.55, state.camera.zoom * factor));
    }, { passive: false });

    el.canvas.addEventListener('dblclick', () => {
      state.camera = { x: 0, y: 0, zoom: 1 };
      computeLayout(false);
    });

    // Navigation & Topbar
    document.getElementById('btnToggleRail').addEventListener('click', () => el.shell.classList.toggle('rail-expanded'));
    document.getElementById('navGalaxy').addEventListener('click', () => setView('galaxy'));
    document.getElementById('navGrid').addEventListener('click', () => setView('grid'));
    document.getElementById('btnBackToGalaxy').addEventListener('click', () => setView('galaxy'));
    document.getElementById('navSearch').addEventListener('click', () => el.searchInput.focus());
    document.getElementById('navScan').addEventListener('click', triggerScan);
    document.getElementById('btnEmptyScan').addEventListener('click', triggerScan);
    document.getElementById('btnSettingsScan').addEventListener('click', () => { closeModals(); triggerScan(); });
    document.getElementById('btnUploadTop').addEventListener('click', () => el.fileInput.click());
    document.getElementById('btnEmptyUpload').addEventListener('click', () => el.fileInput.click());
    document.getElementById('navActivity').addEventListener('click', () => el.shell.classList.toggle('activity-open'));
    document.getElementById('navSettings').addEventListener('click', openSettingsModal);
    document.getElementById('btnSettingsTop').addEventListener('click', openSettingsModal);
    document.getElementById('navResetView').addEventListener('click', () => {
      state.camera = { x: 0, y: 0, zoom: 1 };
      state.selectedHash = null;
      setCategoryFilter(null);
      el.searchInput.value = '';
      runSemanticSearch('');
      computeLayout(false);
    });
    document.getElementById('btnClearFilter').addEventListener('click', () => setCategoryFilter(null));

    // Dropdowns
    el.btnWorkspaceMenu.addEventListener('click', () => {
      const open = el.workspaceDropdown.classList.toggle('hidden');
      el.btnWorkspaceMenu.setAttribute('aria-expanded', String(!open));
    });
    document.getElementById('btnActivityMenu').addEventListener('click', () => el.activityDropdown.classList.toggle('hidden'));
    document.getElementById('btnMenuScan').addEventListener('click', () => { el.activityDropdown.classList.add('hidden'); triggerScan(); });
    document.getElementById('btnMenuUpload').addEventListener('click', () => { el.activityDropdown.classList.add('hidden'); el.fileInput.click(); });
    document.getElementById('btnMenuReset').addEventListener('click', () => {
      el.activityDropdown.classList.add('hidden');
      state.selectedHash = null;
      setCategoryFilter(null);
      renderActivityStream();
    });

    // Search
    el.searchInput.addEventListener('input', (e) => {
      clearTimeout(searchTimer);
      searchTimer = setTimeout(() => runSemanticSearch(e.target.value), 120);
    });
    document.getElementById('btnSemanticSearch').addEventListener('click', () => runSemanticSearch(el.searchInput.value));
    document.getElementById('btnClearSearch').addEventListener('click', () => { el.searchInput.value = ''; runSemanticSearch(''); });
    el.btnInputClear.addEventListener('click', () => { el.searchInput.value = ''; runSemanticSearch(''); el.searchInput.focus(); });

    // Drop-Dock & Drag-and-Drop
    document.getElementById('btnDockPrev').addEventListener('click', () => { state.dockPage -= 1; renderDropDock(); });
    document.getElementById('btnDockNext').addEventListener('click', () => { state.dockPage += 1; renderDropDock(); });
    el.fileInput.addEventListener('change', (e) => { uploadFiles(e.target.files); e.target.value = ''; });

    let dragDepth = 0;
    window.addEventListener('dragenter', (e) => {
      if (e.dataTransfer?.types?.includes('Files')) {
        dragDepth += 1;
        el.dragOverlay.classList.remove('hidden');
        el.dropDock.classList.add('drag-over');
      }
    });
    window.addEventListener('dragover', (e) => { e.preventDefault(); });
    window.addEventListener('dragleave', () => {
      dragDepth = Math.max(0, dragDepth - 1);
      if (dragDepth === 0) {
        el.dragOverlay.classList.add('hidden');
        el.dropDock.classList.remove('drag-over');
      }
    });
    window.addEventListener('drop', (e) => {
      e.preventDefault();
      dragDepth = 0;
      el.dragOverlay.classList.add('hidden');
      el.dropDock.classList.remove('drag-over');
      if (e.dataTransfer?.files?.length) uploadFiles(e.dataTransfer.files);
    });

    // Inline Drawers & Two-Step Delete
    document.getElementById('btnCloseDocModal').addEventListener('click', closeModals);
    document.getElementById('btnCloseSettingsModal').addEventListener('click', closeModals);

    el.btnFocusInGalaxy.addEventListener('click', () => {
      const hash = el.btnFocusInGalaxy.dataset.hash;
      closeModals();
      setView('galaxy');
      const node = state.layout.nodes.get(hash);
      if (node) {
        state.camera.x = width / 2 - node.x;
        state.camera.y = height / 2 - node.y;
        state.camera.zoom = 1.25;
        state.ripples.push({ x: node.x, y: node.y, baseRadius: 18, color: '#e59a54', start: performance.now() });
      }
    });

    el.btnDeleteDoc.addEventListener('click', () => {
      el.btnDeleteDoc.classList.add('hidden');
      el.deleteConfirmBar.classList.remove('hidden');
    });

    el.btnCancelDelete.addEventListener('click', () => {
      el.deleteConfirmBar.classList.add('hidden');
      el.btnDeleteDoc.classList.remove('hidden');
    });

    el.btnConfirmDelete.addEventListener('click', async () => {
      const hash = el.btnDeleteDoc.dataset.hash;
      if (!hash) return;
      try {
        const res = await apiFetch(`/api/documents/${encodeURIComponent(hash)}`, { method: 'DELETE' });
        if (!res.ok) throw new Error('Failed to remove document');
        closeModals();
        state.selectedHash = null;
        showToast('Document removed from index');
        await fetchGraphAndStats();
      } catch (err) {
        showToast(err.message);
      }
    });

    // Keyboard shortcuts
    window.addEventListener('keydown', (e) => {
      if (e.key === '/' && document.activeElement !== el.searchInput && el.inspectorDrawer.classList.contains('hidden')) {
        e.preventDefault();
        el.searchInput.focus();
      } else if (e.key === 'Escape') {
        if (!el.inspectorDrawer.classList.contains('hidden') || !el.settingsDrawer.classList.contains('hidden')) {
          closeModals();
        } else if (el.searchInput.value) {
          el.searchInput.value = '';
          runSemanticSearch('');
        }
        el.workspaceDropdown.classList.add('hidden');
        el.activityDropdown.classList.add('hidden');
      }
    });
  }

  resizeCanvas();
  bindEvents();
  fetchGraphAndStats();
  connectSse();
  requestAnimationFrame(renderFrame);
})();
