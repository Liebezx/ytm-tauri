(() => {
  if (typeof location === "object" && location.hostname && location.hostname !== "music.youtube.com") {
    return;
  }
  if (window.__liebeTitlebarInstalled) return;
  window.__liebeTitlebarInstalled = true;

  const TITLEBAR_HEIGHT = 34;

  function syncZoomCssVar(explicitZoom) {
    const raw = explicitZoom ?? window.__ytmFeatures?.config?.zoom ?? window.__ytmFeatureConfig?.zoom ?? 1;
    const parsed = Number(raw);
    const clamped = Number.isFinite(parsed) && parsed > 0 ? Math.min(2.0, Math.max(0.5, parsed)) : 1;
    document.documentElement?.style?.setProperty?.("--ytm-zoom", String(clamped));
  }
  syncZoomCssVar();
  window.addEventListener("ytm-settings-changed", (ev) => syncZoomCssVar(ev?.detail?.zoom));

  const css = `
    :root {
      --ytm-zoom: 1;
      --liebe-tb-height: calc(${TITLEBAR_HEIGHT}px / var(--ytm-zoom, 1));
      --theme-bg-dark: #09090b;
      --theme-tint: #0f0f12;
      --theme-tint-2: #0b0b0d;
      --theme-border: rgba(255, 255, 255, 0.08);
      --theme-glow-1: rgba(255, 255, 255, 0.05);
      --theme-glow-2: rgba(200, 200, 210, 0.03);
      --theme-glow-3: rgba(255, 255, 255, 0.02);
      --theme-ambient-dark: #08080a;
      --glow-color: rgba(255, 255, 255, 0.58);
      --theme-lyrics-tint: #ffffff;
      --theme-lyrics-glow-core: rgba(255, 255, 255, 0.72);
      --theme-lyrics-glow-outer: rgba(210, 215, 225, 0.40);
      --theme-lyrics-glow-ambient: rgba(170, 178, 195, 0.22);
      --theme-lyrics-backlight: rgba(255, 255, 255, 0.08);
      --ytmusic-player-page-horizontal-margin: 0px !important;
    }

    /* === Root Scroll: Keep native scrolling & layout intact, only hide root scrollbar visual so it never overlaps titlebar === */
    html,
    body {
      scrollbar-width: none !important;
      -ms-overflow-style: none !important;
    }

    html::-webkit-scrollbar,
    body::-webkit-scrollbar {
      display: none !important;
      width: 0px !important;
      height: 0px !important;
      background: transparent !important;
    }

    /* === Sleek Modern Scrollbars for Playlists, Queue, and Main Browsing === */
    ytmusic-player-queue,
    ytmusic-playlist-shelf-renderer,
    #contents.ytmusic-playlist-shelf-renderer,
    ytmusic-section-list-renderer,
    #main-panel {
      scrollbar-width: thin !important;
      scrollbar-color: rgba(255, 255, 255, 0.35) transparent !important;
    }

    ::-webkit-scrollbar {
      width: 8px !important;
      height: 8px !important;
      background: transparent !important;
    }

    ::-webkit-scrollbar-track {
      background: transparent !important;
    }

    ::-webkit-scrollbar-thumb {
      background: rgba(255, 255, 255, 0.3) !important;
      border-radius: 4px !important;
      border: 2px solid transparent !important;
      background-clip: padding-box !important;
    }

    ::-webkit-scrollbar-thumb:hover {
      background: rgba(255, 255, 255, 0.65) !important;
    }

    /* === Hide vertical scrollbars ONLY where unwanted (Sidebar, outer Player Page, Chips) === */
    #guide-wrapper,
    #guide #guide-wrapper,
    tp-yt-app-drawer#guide,
    #mini-guide,
    ytmusic-mini-guide-renderer,
    ytmusic-player-page,
    #player-page,
    ytmusic-chip-cloud-renderer #chips {
      scrollbar-width: none !important;
      -ms-overflow-style: none !important;
    }

    #guide-wrapper::-webkit-scrollbar,
    #guide #guide-wrapper::-webkit-scrollbar,
    tp-yt-app-drawer#guide::-webkit-scrollbar,
    #mini-guide::-webkit-scrollbar,
    ytmusic-mini-guide-renderer::-webkit-scrollbar,
    ytmusic-player-page::-webkit-scrollbar,
    #player-page::-webkit-scrollbar,
    ytmusic-chip-cloud-renderer #chips::-webkit-scrollbar {
      display: none !important;
      width: 0px !important;
      height: 0px !important;
      background: transparent !important;
    }

    /* === Carousel horizontal scrollbar: transparent by default with fixed 6px height so hover NEVER shifts layout === */
    ytmusic-carousel-shelf-renderer #items,
    ytmusic-shelf-renderer #items,
    ytmusic-carousel #items,
    #items.ytmusic-carousel,
    #items.ytmusic-carousel-shelf-renderer,
    #carousel-items,
    ytmusic-carousel {
      scrollbar-width: thin !important;
      scrollbar-color: transparent transparent !important;
    }

    ytmusic-carousel-shelf-renderer #items::-webkit-scrollbar,
    ytmusic-shelf-renderer #items::-webkit-scrollbar,
    ytmusic-carousel #items::-webkit-scrollbar,
    #items.ytmusic-carousel::-webkit-scrollbar,
    #items.ytmusic-carousel-shelf-renderer::-webkit-scrollbar,
    #carousel-items::-webkit-scrollbar,
    ytmusic-carousel::-webkit-scrollbar {
      display: block !important;
      height: 6px !important;
      width: 6px !important;
      background: transparent !important;
      border-radius: 6px !important;
    }

    ytmusic-carousel-shelf-renderer #items::-webkit-scrollbar-track,
    ytmusic-shelf-renderer #items::-webkit-scrollbar-track,
    ytmusic-carousel #items::-webkit-scrollbar-track,
    #items.ytmusic-carousel::-webkit-scrollbar-track,
    #items.ytmusic-carousel-shelf-renderer::-webkit-scrollbar-track,
    #carousel-items::-webkit-scrollbar-track,
    ytmusic-carousel::-webkit-scrollbar-track {
      display: block !important;
      background: transparent !important;
      border-radius: 6px !important;
    }

    ytmusic-carousel-shelf-renderer #items::-webkit-scrollbar-thumb,
    ytmusic-shelf-renderer #items::-webkit-scrollbar-thumb,
    ytmusic-carousel #items::-webkit-scrollbar-thumb,
    #items.ytmusic-carousel::-webkit-scrollbar-thumb,
    #items.ytmusic-carousel-shelf-renderer::-webkit-scrollbar-thumb,
    #carousel-items::-webkit-scrollbar-thumb,
    ytmusic-carousel::-webkit-scrollbar-thumb {
      display: block !important;
      background: transparent !important;
      border: none !important;
      border-radius: 6px !important;
    }

    /* Appear ONLY on hover over the shelf or carousel — zero height change */
    ytmusic-carousel-shelf-renderer:hover #items,
    ytmusic-shelf-renderer:hover #items,
    ytmusic-carousel:hover #items,
    #items.ytmusic-carousel:hover,
    #items.ytmusic-carousel-shelf-renderer:hover,
    #carousel-items:hover,
    ytmusic-carousel:hover,
    #items:hover {
      scrollbar-color: rgba(255, 255, 255, 0.45) rgba(255, 255, 255, 0.08) !important;
    }

    ytmusic-carousel-shelf-renderer:hover #items::-webkit-scrollbar,
    ytmusic-shelf-renderer:hover #items::-webkit-scrollbar,
    ytmusic-carousel:hover #items::-webkit-scrollbar,
    #items.ytmusic-carousel:hover::-webkit-scrollbar,
    #items.ytmusic-carousel-shelf-renderer:hover::-webkit-scrollbar,
    #carousel-items:hover::-webkit-scrollbar,
    ytmusic-carousel:hover::-webkit-scrollbar,
    #items:hover::-webkit-scrollbar {
      background: rgba(255, 255, 255, 0.08) !important;
    }

    ytmusic-carousel-shelf-renderer:hover #items::-webkit-scrollbar-track,
    ytmusic-shelf-renderer:hover #items::-webkit-scrollbar-track,
    ytmusic-carousel:hover #items::-webkit-scrollbar-track,
    #items.ytmusic-carousel:hover::-webkit-scrollbar-track,
    #items.ytmusic-carousel-shelf-renderer:hover::-webkit-scrollbar-track,
    #carousel-items:hover::-webkit-scrollbar-track,
    ytmusic-carousel:hover::-webkit-scrollbar-track,
    #items:hover::-webkit-scrollbar-track {
      background: rgba(255, 255, 255, 0.08) !important;
    }

    ytmusic-carousel-shelf-renderer:hover #items::-webkit-scrollbar-thumb,
    ytmusic-shelf-renderer:hover #items::-webkit-scrollbar-thumb,
    ytmusic-carousel:hover #items::-webkit-scrollbar-thumb,
    #items.ytmusic-carousel:hover::-webkit-scrollbar-thumb,
    #items.ytmusic-carousel-shelf-renderer:hover::-webkit-scrollbar-thumb,
    #carousel-items:hover::-webkit-scrollbar-thumb,
    ytmusic-carousel:hover::-webkit-scrollbar-thumb,
    #items:hover::-webkit-scrollbar-thumb {
      background: rgba(255, 255, 255, 0.45) !important;
      cursor: grab !important;
    }

    ytmusic-carousel-shelf-renderer #items::-webkit-scrollbar-thumb:hover,
    ytmusic-shelf-renderer #items::-webkit-scrollbar-thumb:hover,
    ytmusic-carousel #items::-webkit-scrollbar-thumb:hover,
    #items.ytmusic-carousel::-webkit-scrollbar-thumb:hover,
    #items.ytmusic-carousel-shelf-renderer::-webkit-scrollbar-thumb:hover,
    #carousel-items::-webkit-scrollbar-thumb:hover,
    ytmusic-carousel::-webkit-scrollbar-thumb:hover,
    #items::-webkit-scrollbar-thumb:hover {
      background: rgba(255, 255, 255, 0.85) !important;
    }

    ytmusic-carousel-shelf-renderer #items::-webkit-scrollbar-thumb:active,
    ytmusic-shelf-renderer #items::-webkit-scrollbar-thumb:active,
    ytmusic-carousel #items::-webkit-scrollbar-thumb:active,
    #items.ytmusic-carousel::-webkit-scrollbar-thumb:active,
    #items.ytmusic-carousel-shelf-renderer::-webkit-scrollbar-thumb:active,
    #carousel-items::-webkit-scrollbar-thumb:active,
    ytmusic-carousel::-webkit-scrollbar-thumb:active,
    #items::-webkit-scrollbar-thumb:active {
      background: #ffffff !important;
      cursor: grabbing !important;
    }

    /* === Custom Crafted Modern Glass Titlebar (scales inverse to WebView zoom so window chrome stays crisp) === */
    #liebe-titlebar {
      position: fixed !important;
      top: 0 !important;
      left: 0 !important;
      right: 0 !important;
      height: var(--liebe-tb-height) !important;
      z-index: 2147483647 !important;
      display: flex !important;
      align-items: center !important;
      justify-content: space-between !important;
      user-select: none !important;
      box-sizing: border-box !important;
      padding: 0 calc(8px / var(--ytm-zoom, 1)) 0 calc(14px / var(--ytm-zoom, 1)) !important;
      background: linear-gradient(90deg, var(--theme-bg-dark, #09090b) 0%, var(--theme-tint, #0f0f12) 45%, var(--theme-tint-2, #0b0b0d) 100%) !important;
      border-bottom: 1px solid rgba(255, 255, 255, 0.08) !important;
      opacity: 1 !important;
      box-shadow: 0 2px 14px rgba(0, 0, 0, 0.45) !important;
      transition: background-color 0.5s ease !important;
    }

    .titlebar-drag {
      display: flex !important;
      align-items: center !important;
      gap: calc(10px / var(--ytm-zoom, 1)) !important;
      flex: 1 !important;
      min-width: 0 !important;
      height: 100% !important;
      cursor: default !important;
      overflow: hidden !important;
      -webkit-app-region: drag !important;
    }

    .titlebar-logo-wrap {
      display: flex !important;
      align-items: center !important;
      justify-content: center !important;
      flex-shrink: 0 !important;
      pointer-events: none !important;
    }

    .titlebar-logo {
      width: calc(17px / var(--ytm-zoom, 1)) !important;
      height: calc(17px / var(--ytm-zoom, 1)) !important;
      filter: drop-shadow(0 0 6px rgba(255, 0, 51, 0.45)) !important;
      pointer-events: none !important;
    }

    .titlebar-title {
      font-size: calc(12.5px / var(--ytm-zoom, 1)) !important;
      font-weight: 600 !important;
      color: #e2e8f0 !important;
      letter-spacing: 0.2px !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
      max-width: 100% !important;
      font-family: "YouTube Sans", "Roboto", system-ui, -apple-system, sans-serif !important;
      pointer-events: none !important;
      text-shadow: 0 1px 4px rgba(0, 0, 0, 0.7) !important;
    }

    /* === Custom Smoothed Glass Floating Buttons === */
    .titlebar-controls {
      display: flex !important;
      align-items: center !important;
      gap: calc(6px / var(--ytm-zoom, 1)) !important;
      height: 100% !important;
      pointer-events: auto !important;
      flex-shrink: 0 !important;
      -webkit-app-region: no-drag !important;
      padding-right: calc(6px / var(--ytm-zoom, 1)) !important;
    }

    .titlebar-btn {
      background: rgba(255, 255, 255, 0.05) !important;
      border: 1px solid rgba(255, 255, 255, 0.09) !important;
      outline: none !important;
      color: #cbd5e1 !important;
      width: calc(36px / var(--ytm-zoom, 1)) !important;
      height: calc(26px / var(--ytm-zoom, 1)) !important;
      border-radius: calc(7px / var(--ytm-zoom, 1)) !important;
      display: flex !important;
      align-items: center !important;
      justify-content: center !important;
      cursor: pointer !important;
      pointer-events: auto !important;
      padding: 0 !important;
      margin: 0 !important;
      transition: transform 0.16s cubic-bezier(0.2, 0.8, 0.2, 1), background-color 0.16s ease, border-color 0.16s ease !important;
      -webkit-app-region: no-drag !important;
      box-sizing: border-box !important;
    }

    .titlebar-btn .tb-svg {
      display: block !important;
      width: calc(14px / var(--ytm-zoom, 1)) !important;
      height: calc(14px / var(--ytm-zoom, 1)) !important;
      pointer-events: none !important;
      shape-rendering: geometricPrecision !important;
      vector-effect: non-scaling-stroke !important;
      transition: transform 0.16s ease !important;
    }

    .titlebar-btn:hover {
      background: rgba(255, 255, 255, 0.12) !important;
      border-color: rgba(255, 255, 255, 0.22) !important;
      color: #ffffff !important;
      box-shadow: 0 2px 10px rgba(0, 0, 0, 0.35), 0 0 12px rgba(255, 255, 255, 0.08) !important;
      transform: translateY(-1px) !important;
    }

    .titlebar-btn:active {
      transform: scale(0.93) !important;
      background: rgba(255, 255, 255, 0.18) !important;
    }

    .titlebar-btn.titlebar-close:hover {
      background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%) !important;
      border-color: rgba(255, 120, 120, 0.5) !important;
      color: #ffffff !important;
      box-shadow: 0 2px 14px rgba(239, 68, 68, 0.5) !important;
      transform: translateY(-1px) !important;
    }

    .titlebar-btn.titlebar-close:active {
      transform: scale(0.93) !important;
      background: #b91c1c !important;
    }

    @keyframes smoothSpin {
      0% { transform: rotate(0deg); }
      100% { transform: rotate(360deg); }
    }

    .tb-reload-icon.spinning {
      animation: smoothSpin 0.5s cubic-bezier(0.25, 1, 0.5, 1) !important;
    }

    /* Seamless dynamic connection with nav-bar below */
    ytmusic-nav-bar,
    #nav-bar-background,
    #layout #nav-bar-background {
      top: var(--liebe-tb-height) !important;
      background: linear-gradient(90deg, var(--theme-bg-dark, #09090b) 0%, var(--theme-tint, #0f0f12) 45%, var(--theme-tint-2, #0b0b0d) 100%) !important;
      border-bottom: 1px solid rgba(255, 255, 255, 0.08) !important;
      opacity: 1 !important;
      z-index: 2000 !important;
    }

    /* Fix sidebar & mini-guide offset below navbar and eliminate empty gap */
    tp-yt-app-drawer#guide,
    #guide,
    #mini-guide,
    ytmusic-mini-guide-renderer,
    #mini-guide-background {
      top: calc(var(--ytmusic-nav-bar-height, 64px) + var(--liebe-tb-height)) !important;
      height: calc(100vh - var(--ytmusic-nav-bar-height, 64px) - var(--liebe-tb-height) - var(--ytmusic-player-bar-height, 72px)) !important;
    }

    #guide-wrapper,
    #guide #guide-wrapper,
    tp-yt-app-drawer#guide #contentContainer,
    #guide #contentContainer,
    ytmusic-guide-renderer,
    #guide ytmusic-guide-renderer {
      padding-top: 0px !important;
      margin-top: 0px !important;
    }

    /* Pull only the very first section slightly up; do not touch #items to keep + Новый and playlists separated */
    #guide ytmusic-guide-section-renderer:first-of-type {
      margin-top: -26px !important;
      padding-top: 0px !important;
    }

    /* Sidebar styling: Single unified ambient wash without inner overlapping gradients/seams */
    tp-yt-app-drawer#guide,
    #guide,
    #guide-wrapper,
    #guide #guide-wrapper,
    #mini-guide,
    #mini-guide-background,
    ytmusic-mini-guide-renderer {
      background: linear-gradient(180deg, var(--theme-tint, #0f0f12) 0%, var(--theme-tint-2, #0b0b0d) 38%, var(--theme-ambient-dark, #08080a) 75%, rgba(6, 6, 8, 0.98) 100%) !important;
      background-color: var(--theme-ambient-dark, #08080a) !important;
    }

    #guide-wrapper,
    #guide #guide-wrapper {
      border-right: none !important;
    }

    tp-yt-app-drawer#guide #contentContainer,
    #guide #contentContainer,
    ytmusic-guide-renderer,
    #guide ytmusic-guide-renderer,
    ytmusic-guide-renderer #guide-spacer {
      background: transparent !important;
      background-color: transparent !important;
    }

    #guide-wrapper {
      scrollbar-width: none !important;
    }
    #guide-wrapper::-webkit-scrollbar {
      display: none !important;
      width: 0px !important;
    }

    /* Video player: Clean, properly contained sleek video card without bleed-through */
    ytmusic-player[video-mode_] #song-image,
    ytmusic-player[video-mode] #song-image,
    [video-mode] #song-image,
    .video-mode #song-image,
    ytmusic-player[video-mode_] .html5-blur-image,
    ytmusic-player[video-mode] .html5-blur-image {
      display: none !important;
      opacity: 0 !important;
      visibility: hidden !important;
    }

    #song-video,
    .html5-video-player {
      background: #060608 !important;
      border-radius: 12px !important;
      overflow: hidden !important;
      box-shadow: 0 10px 36px rgba(0, 0, 0, 0.6) !important;
    }

    .html5-video-player .ytp-gradient-top,
    .html5-video-player .ytp-chrome-top {
      background: transparent !important;
      background-image: linear-gradient(180deg, rgba(0, 0, 0, 0.65) 0%, transparent 100%) !important;
    }

    .html5-video-player .ytp-gradient-bottom,
    .html5-video-player .ytp-chrome-bottom {
      background: transparent !important;
      background-image: linear-gradient(0deg, rgba(0, 0, 0, 0.65) 0%, transparent 100%) !important;
    }

    ytmusic-player-page #main-panel,
    ytmusic-player,
    #player.ytmusic-player-page,
    #player {
      background: transparent !important;
      background-color: transparent !important;
    }

    #layout,
    ytmusic-app-layout {
      padding-top: var(--liebe-tb-height) !important;
    }

    /* === Dynamic Shimmering Ambient Glow Background for Player Page === */
    #layout[player-page-open] ytmusic-player-page,
    ytmusic-app-layout[player-page-open] ytmusic-player-page,
    #layout[player-page-open] #player-page,
    ytmusic-app-layout[player-page-open] #player-page {
      top: calc(var(--ytmusic-nav-bar-height, 64px) + var(--liebe-tb-height)) !important;
      height: calc(100vh - var(--ytmusic-nav-bar-height, 64px) - var(--liebe-tb-height) - var(--ytmusic-player-bar-height, 72px)) !important;
      max-height: calc(100vh - var(--ytmusic-nav-bar-height, 64px) - var(--liebe-tb-height) - var(--ytmusic-player-bar-height, 72px)) !important;
      box-sizing: border-box !important;
      scrollbar-width: thin !important;
      scrollbar-color: rgba(255, 255, 255, 0.24) transparent !important;
    }

    @media (max-width: 935px) {
      #layout[player-page-open] ytmusic-player-page {
        overflow-y: auto !important;
        overflow-x: hidden !important;
      }
      #layout[player-page-open] ytmusic-player-page #main-panel {
        max-height: clamp(140px, 34vh, 280px) !important;
        flex: 0 0 auto !important;
      }
      #layout[player-page-open] ytmusic-player-page #song-image,
      #layout[player-page-open] ytmusic-player-page ytmusic-player {
        max-height: clamp(130px, 32vh, 260px) !important;
        max-width: min(320px, 78vw) !important;
        margin: 0 auto !important;
      }
      #layout[player-page-open] ytmusic-player-page #side-panel {
        min-height: clamp(180px, 44vh, 520px) !important;
      }
    }

    ytmusic-player-page,
    #player-page {
      background:
        radial-gradient(ellipse 70% 65% at 35% 42%, var(--theme-glow-1) 0%, transparent 75%),
        radial-gradient(ellipse 55% 55% at 80% 68%, var(--theme-glow-2) 0%, transparent 70%),
        radial-gradient(ellipse 75% 55% at 50% 10%, var(--theme-glow-3) 0%, transparent 75%),
        linear-gradient(150deg, var(--theme-ambient-dark) 0%, #09090b 50%, #050506 100%) !important;
    }

    /* === Preserved Natural Layout: Tabs & Queue remain fully visible, no focus box/ripple or 12px gutter notch === */
    ytmusic-app,
    ytmusic-app-layout,
    #layout {
      --ytmusic-scrollbar-width: 0px !important;
    }

    ytmusic-app-layout > [slot="nav-bar"],
    ytmusic-app-layout > [slot="player-bar"] {
      width: 100% !important;
    }

    ytmusic-player-page {
      --ytmusic-player-page-horizontal-margin: 0px !important;
    }

    ytmusic-player-page #side-panel,
    #side-panel.side-panel.ytmusic-player-page {
      z-index: 1 !important;
      position: relative !important;
      background: transparent !important;
    }

    #tabsContent tp-yt-paper-tab,
    #tabsContent tp-yt-paper-tab:focus,
    #tabsContent tp-yt-paper-tab:focus-visible,
    #tabsContent tp-yt-paper-tab[focused] {
      background: transparent !important;
      background-color: transparent !important;
      outline: none !important;
      box-shadow: none !important;
      --paper-tab-ink: transparent !important;
    }

    #tabsContent tp-yt-paper-tab tp-yt-paper-ripple,
    #tabsContent tp-yt-paper-tab tp-yt-paper-ripple #background,
    #tabsContent tp-yt-paper-tab tp-yt-paper-ripple #waves {
      display: none !important;
      opacity: 0 !important;
    }

    /* === Solid Opaque Bottom Player Bar (Supports classic ytmusic-player-bar & Wiz ytmusic-miniplayer) === */
    /* Keep the background slab strictly behind [slot="player-bar"] so it never occludes controls in A/B layouts */
    #player-bar-background,
    #layout #player-bar-background {
      background: var(--theme-ambient-dark, #08080a) !important;
      background-color: var(--theme-ambient-dark, #08080a) !important;
      backdrop-filter: none !important;
      -webkit-backdrop-filter: none !important;
      border-top: 1px solid rgba(255, 255, 255, 0.06) !important;
      z-index: 990 !important;
      pointer-events: none !important;
    }

    ytmusic-app-layout > [slot="player-bar"],
    #layout > [slot="player-bar"],
    ytmusic-player-bar,
    ytmusic-miniplayer {
      background: var(--theme-ambient-dark, #08080a) !important;
      background-color: var(--theme-ambient-dark, #08080a) !important;
      opacity: 1 !important;
      backdrop-filter: none !important;
      -webkit-backdrop-filter: none !important;
      border-top: 1px solid rgba(255, 255, 255, 0.06) !important;
      z-index: 1000 !important;
    }

    /* === 3-Dots Context Menu & Dropdowns: Ultra-Clean, Compact, Viewport-Safe Alignment === */
    ytmusic-popup-container {
      z-index: 100000 !important;
    }

    tp-yt-iron-dropdown {
      z-index: 100000 !important;
      max-width: min(360px, calc(100vw - 16px)) !important;
      max-height: calc(100vh - var(--liebe-tb-height) - 16px) !important;
      width: auto !important;
    }

    ytmusic-menu-popup-renderer {
      --paper-item-min-height: 34px !important;
      min-width: min(290px, calc(100vw - 24px)) !important;
      max-width: min(340px, calc(100vw - 24px)) !important;
      width: max-content !important;
      border-radius: 10px !important;
      border: 1px solid rgba(255, 255, 255, 0.12) !important;
      background: #202022 !important;
      box-shadow: 0 10px 28px rgba(0, 0, 0, 0.65) !important;
      padding: 6px 0 !important;
      overflow-x: hidden !important;
    }

    tp-yt-paper-listbox.ytmusic-menu-popup-renderer,
    #items.ytmusic-menu-popup-renderer {
      padding: 0 !important;
      margin: 0 !important;
      background: transparent !important;
      width: 100% !important;
    }

    ytmusic-menu-popup-renderer #items > *,
    tp-yt-paper-listbox.ytmusic-menu-popup-renderer > *,
    ytmusic-menu-service-item-renderer,
    ytmusic-menu-navigation-item-renderer,
    ytmusic-toggle-menu-service-item-renderer,
    ytmusic-menu-service-item-download-renderer {
      min-height: 34px !important;
      height: 34px !important;
      max-height: 34px !important;
      display: flex !important;
      align-items: center !important;
      box-sizing: border-box !important;
      padding: 0 14px !important;
      margin: 1px 6px !important;
      border-radius: 6px !important;
      width: calc(100% - 12px) !important;
      cursor: pointer !important;
      transition: background 0.1s ease !important;
    }

    ytmusic-menu-popup-renderer #items > *:hover,
    tp-yt-paper-listbox.ytmusic-menu-popup-renderer > *:hover {
      background: rgba(255, 255, 255, 0.1) !important;
    }

    ytmusic-menu-popup-renderer a.yt-simple-endpoint:not([hidden]),
    ytmusic-menu-popup-renderer tp-yt-paper-item:not([hidden]) {
      min-height: 34px !important;
      height: 34px !important;
      max-height: 34px !important;
      padding: 0 !important;
      margin: 0 !important;
      display: flex !important;
      align-items: center !important;
      width: 100% !important;
      box-sizing: border-box !important;
      background: transparent !important;
      cursor: pointer !important;
    }

    ytmusic-menu-popup-renderer yt-icon.icon {
      width: 18px !important;
      height: 18px !important;
      min-width: 18px !important;
      margin: 0 14px 0 0 !important;
      padding: 0 !important;
      display: inline-flex !important;
      align-items: center !important;
      justify-content: center !important;
      flex-shrink: 0 !important;
      color: rgba(255, 255, 255, 0.78) !important;
    }

    ytmusic-menu-popup-renderer yt-formatted-string {
      font-size: 13px !important;
      line-height: 20px !important;
      font-weight: 400 !important;
      color: #f1f1f1 !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
      flex: 1 1 auto !important;
      margin: 0 !important;
      padding: 0 !important;
    }

    ytmusic-menu-popup-renderer [hidden] {
      display: none !important;
    }

    ytmusic-popup-container *,
    ytmusic-menu-popup-renderer,
    ytmusic-menu-popup-renderer *,
    tp-yt-iron-dropdown,
    tp-yt-iron-dropdown *,
    tp-yt-paper-listbox,
    tp-yt-paper-listbox * {
      scrollbar-width: none !important;
      -ms-overflow-style: none !important;
    }

    ytmusic-popup-container *::-webkit-scrollbar,
    ytmusic-menu-popup-renderer::-webkit-scrollbar,
    ytmusic-menu-popup-renderer *::-webkit-scrollbar,
    tp-yt-iron-dropdown::-webkit-scrollbar,
    tp-yt-iron-dropdown *::-webkit-scrollbar,
    tp-yt-paper-listbox::-webkit-scrollbar,
    tp-yt-paper-listbox *::-webkit-scrollbar,
    .ytmusic-menu-popup-renderer::-webkit-scrollbar {
      display: none !important;
      width: 0px !important;
      height: 0px !important;
    }

    /* === Allow Selecting Text in Song Lyrics === */
    .synced-lyrics-wrapper,
    .synced-lyrics-wrapper *,
    .text-lyrics,
    .text-lyrics *,
    .lyrics-line,
    .lyrics-line *,
    .synced-lyrics-vlist,
    .synced-lyrics-vlist *,
    ytmusic-description-shelf-renderer,
    ytmusic-description-shelf-renderer *,
    ytmusic-tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'],
    ytmusic-tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'] *,
    #tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'],
    #tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'] *,
    .description.ytmusic-description-shelf-renderer,
    .description.ytmusic-description-shelf-renderer * {
      -webkit-user-select: text !important;
      -moz-user-select: text !important;
      -ms-user-select: text !important;
      user-select: text !important;
      cursor: text !important;
    }


    ytmusic-player-bar #right-controls,
    [slot="player-bar"] #right-controls {
      margin-right: 0px !important;
      padding-right: 8px !important;
    }

    ytmusic-nav-bar #right-content {
      flex-shrink: 0 !important;
      min-width: 0 !important;
      display: flex !important;
      align-items: center !important;
    }

    /* === Search Box: Fluid Width So Zooming Never Pushes Settings/Profile Off-Screen === */
    ytmusic-search-box:not([is-mobile-view]) {
      background: transparent !important;
      background-color: transparent !important;
      border: none !important;
      box-shadow: none !important;
      outline: none !important;
      border-radius: 8px !important;
      min-width: 0 !important;
      max-width: 660px !important;
      width: clamp(180px, 42vw, 660px) !important;
      flex: 0 1 660px !important;
    }

    ytmusic-search-box[is-mobile-view]:not([opened_]) {
      min-width: 0 !important;
      max-width: none !important;
      width: auto !important;
      flex: 0 0 auto !important;
    }

    @media (max-width: 1120px) {
      ytmusic-search-box:not([is-mobile-view]) {
        width: auto !important;
        flex: 1 1 180px !important;
        max-width: 480px !important;
      }
    }

    @media (max-width: 960px) {
      .ytm-spot-btn {
        padding: 5px 10px !important;
        font-size: 12px !important;
        margin-right: 4px !important;
      }
      #ytm-header-settings-btn {
        width: 34px !important;
        height: 34px !important;
        margin-right: 2px !important;
      }
    }

    ytmusic-search-box:not([opened_]) {
      background: transparent !important;
      background-color: transparent !important;
      border: none !important;
      box-shadow: none !important;
      outline: none !important;
      border-radius: 8px !important;
      overflow: visible !important;
    }

    ytmusic-search-box:not([is-mobile-view]):not([opened_]) #placeholder {
      border-radius: 8px !important;
      border: 1px solid rgba(255, 255, 255, 0.1) !important;
      box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25) !important;
      outline: none !important;
      background: rgba(255, 255, 255, 0.08) !important;
      padding-right: 20px !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
    }

    ytmusic-search-box:not([is-mobile-view]):not([opened_]) input,
    ytmusic-search-box:not([is-mobile-view]):not([opened_]) #input {
      border-radius: 8px !important;
      border: none !important;
      box-shadow: none !important;
      outline: none !important;
      background: transparent !important;
      width: 100% !important;
      white-space: nowrap !important;
      overflow: hidden !important;
    }

    /* High quality rendering for top-right user avatar & icons */
    ytmusic-settings-button img,
    ytmusic-settings-button yt-img-shadow img {
      image-rendering: -webkit-optimize-contrast !important;
      border-radius: 50% !important;
    }
  `;

  function installTitlebar() {
    if (document.getElementById("liebe-titlebar")) return;
    if (!document.body) {
      document.addEventListener("DOMContentLoaded", installTitlebar, { once: true });
      return;
    }

    // 1. Injected CSS
    let style = document.getElementById("liebe-titlebar-style");
    if (!style) {
      style = document.createElement("style");
      style.id = "liebe-titlebar-style";
      style.textContent = css;
      (document.head || document.documentElement || document.body).appendChild(style);
    }

    // 2. High-quality Custom Smoothed Vector Titlebar
    const header = document.createElement("header");
    header.id = "liebe-titlebar";
    header.setAttribute("data-tauri-drag-region", "true");
    header.innerHTML = `
      <div class="titlebar-drag" data-tauri-drag-region="true">
        <div class="titlebar-logo-wrap">
          <svg viewBox="0 0 24 24" class="titlebar-logo">
            <circle cx="12" cy="12" r="10" fill="#ff0000"/>
            <polygon points="10,8 16,12 10,16" fill="#ffffff"/>
          </svg>
        </div>
        <span id="liebe-titlebar-title" class="titlebar-title">YouTube Music</span>
      </div>
      <div class="titlebar-controls" onmousedown="event.stopPropagation()">
        <button type="button" class="titlebar-btn" id="liebe-btn-reload" title="Перезагрузить" aria-label="Reload">
          <svg viewBox="0 0 14 14" width="14" height="14" class="tb-svg tb-reload-icon" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" shape-rendering="geometricPrecision">
            <path d="M11.8 7 A4.8 4.8 0 1 1 10.3 3.6"/>
            <polyline points="10.5 1.5 10.5 3.8 8.2 3.8"/>
          </svg>
        </button>
        <button type="button" class="titlebar-btn" id="liebe-btn-min" title="Свернуть" aria-label="Minimize">
          <svg viewBox="0 0 14 14" width="14" height="14" class="tb-svg" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" shape-rendering="geometricPrecision">
            <line x1="3" y1="7" x2="11" y2="7"/>
          </svg>
        </button>
        <button type="button" class="titlebar-btn" id="liebe-btn-max" title="Развернуть" aria-label="Maximize">
          <svg viewBox="0 0 14 14" width="14" height="14" class="tb-svg" id="liebe-max-icon" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" shape-rendering="geometricPrecision">
            <rect x="2.5" y="2.5" width="9" height="9" rx="2"/>
          </svg>
        </button>
        <button type="button" class="titlebar-btn titlebar-close" id="liebe-btn-close" title="Закрыть" aria-label="Close">
          <svg viewBox="0 0 14 14" width="14" height="14" class="tb-svg" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" shape-rendering="geometricPrecision">
            <line x1="3.2" y1="3.2" x2="10.8" y2="10.8"/>
            <line x1="10.8" y1="3.2" x2="3.2" y2="10.8"/>
          </svg>
        </button>
      </div>
    `;
    document.body.insertBefore(header, document.body.firstChild);

    const sendCmd = (cmd) => {
      const prev = document.title;
      document.title = "YTMCMD:" + cmd;
      setTimeout(() => {
        if (document.title === "YTMCMD:" + cmd) {
          document.title = prev;
        }
      }, 80);
    };

    const dragEl = header.querySelector(".titlebar-drag");
    if (dragEl) {
      dragEl.addEventListener("mousedown", (e) => {
        if (e.button === 0 && !e.target.closest("button, input, a, .titlebar-btn")) {
          sendCmd("drag");
        }
      });
      dragEl.addEventListener("dblclick", (e) => {
        if (!e.target.closest("button, input, a, .titlebar-btn")) {
          sendCmd("maximize");
        }
      });
    }

    const btnReload = header.querySelector("#liebe-btn-reload");
    if (btnReload) {
      btnReload.addEventListener("click", (e) => {
        e.stopPropagation();
        const icon = btnReload.querySelector(".tb-reload-icon");
        if (icon) icon.classList.add("spinning");
        setTimeout(() => {
          window.location.reload();
        }, 220);
      });
    }

    const btnMin = header.querySelector("#liebe-btn-min");
    if (btnMin) btnMin.addEventListener("click", (e) => { e.stopPropagation(); sendCmd("minimize"); });

    const btnMax = header.querySelector("#liebe-btn-max");
    if (btnMax) btnMax.addEventListener("click", (e) => { e.stopPropagation(); sendCmd("maximize"); });

    const btnClose = header.querySelector("#liebe-btn-close");
    if (btnClose) btnClose.addEventListener("click", (e) => { e.stopPropagation(); sendCmd("close"); });

    const updateMaxIcon = () => {
      const isMax = window.innerWidth >= (screen.availWidth - 10) && window.innerHeight >= (screen.availHeight - 10);
      const icon = header.querySelector("#liebe-max-icon");
      if (!icon) return;
      if (isMax) {
        icon.innerHTML = `<path d="M4.5 3 V2 C4.5 1.4 5 1 5.6 1 H11.4 C12 1 12.5 1.4 12.5 2 V7.8 C12.5 8.4 12 9 11.4 9 H10.5"/><rect x="1.5" y="4.5" width="8" height="8" rx="1.8"/>`;
        header.querySelector("#liebe-btn-max")?.setAttribute("title", "Восстановить");
      } else {
        icon.innerHTML = `<rect x="2.5" y="2.5" width="9" height="9" rx="2"/>`;
        header.querySelector("#liebe-btn-max")?.setAttribute("title", "Развернуть");
      }
    };
    window.addEventListener("resize", updateMaxIcon);
    updateMaxIcon();
  }

  // === Dynamic Theme Color Extraction (CORS-safe with Blob fallback) ===
  let lastCoverSrc = "";
  const MAX_COLOR_CACHE = 12;
  const colorCache = new Map();
  function rememberColor(url, colors) {
    if (colorCache.size >= MAX_COLOR_CACHE) {
      const oldest = colorCache.keys().next().value;
      if (oldest) colorCache.delete(oldest);
    }
    colorCache.set(url, colors);
  }

  function processImageData(data) {
    const buckets = new Map();
    let totalR = 0, totalG = 0, totalB = 0, totalCount = 0;

    for (let i = 0; i < data.length; i += 4) {
      const r = data[i], g = data[i + 1], b = data[i + 2], a = data[i + 3];
      if (a < 128) continue;
      const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
      if (lum < 0.04 || lum > 0.96) continue;

      totalR += r; totalG += g; totalB += b; totalCount++;

      const qr = Math.floor(r / 24);
      const qg = Math.floor(g / 24);
      const qb = Math.floor(b / 24);
      const key = (qr << 10) | (qg << 5) | qb;
      let entry = buckets.get(key);
      if (!entry) {
        entry = { r: 0, g: 0, b: 0, count: 0 };
        buckets.set(key, entry);
      }
      entry.r += r;
      entry.g += g;
      entry.b += b;
      entry.count++;
    }

    const chromatic = [];
    for (const entry of buckets.values()) {
      const r = Math.round(entry.r / entry.count);
      const g = Math.round(entry.g / entry.count);
      const b = Math.round(entry.b / entry.count);
      const maxV = Math.max(r, g, b);
      const minV = Math.min(r, g, b);
      const sat = maxV > 0 ? (maxV - minV) / maxV : 0;
      const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
      if (sat >= 0.15 && entry.count >= 3 && lum > 0.07 && lum < 0.93) {
        const lumBoost = 1.15 - Math.abs(lum - 0.48) * 0.7;
        const score = entry.count * (0.4 + sat * 3.2) * Math.max(0.3, lumBoost);
        chromatic.push({ r, g, b, sat, lum, count: entry.count, score });
      }
    }

    chromatic.sort((a, b) => b.score - a.score);

    if (chromatic.length === 0) {
      const avg = totalCount > 0 ? Math.round((totalR + totalG + totalB) / (3 * totalCount)) : 120;
      const base = Math.max(105, Math.min(165, avg));
      return {
        primary: { r: base, g: base + 2, b: base + 8 },
        secondary: { r: Math.max(85, base - 20), g: Math.max(88, base - 16), b: Math.max(95, base - 10) },
        isMono: true,
      };
    }

    const p = chromatic[0];
    let s = null;
    for (let i = 1; i < chromatic.length; i++) {
      const c = chromatic[i];
      const dist = Math.hypot(c.r - p.r, c.g - p.g, c.b - p.b);
      if (dist > 70 && c.score > p.score * 0.12) {
        s = c;
        break;
      }
    }
    if (!s) {
      s = {
        r: Math.max(20, Math.round(p.r * 0.72)),
        g: Math.max(20, Math.round(p.g * 0.72)),
        b: Math.max(20, Math.round(p.b * 0.72)),
      };
    }

    return {
      primary: { r: p.r, g: p.g, b: p.b },
      secondary: { r: s.r, g: s.g, b: s.b },
      isMono: false,
    };
  }

  function liftColor(rgb, minPeak, maxPeak) {
    const peak = Math.max(rgb.r, rgb.g, rgb.b, 1);
    let scale = 1;
    if (peak < minPeak) scale = minPeak / peak;
    else if (peak > maxPeak) scale = maxPeak / peak;
    return {
      r: Math.min(255, Math.round(rgb.r * scale)),
      g: Math.min(255, Math.round(rgb.g * scale)),
      b: Math.min(255, Math.round(rgb.b * scale)),
    };
  }

  function ensureGlowRgb(rgb, targetLum, whiteMixCap = 0.6) {
    const lifted = liftColor(rgb, 210, 255);
    const lum = (0.299 * lifted.r + 0.587 * lifted.g + 0.114 * lifted.b) / 255;
    let w = 0;
    if (lum < targetLum && lum < 0.99) {
      w = Math.min(whiteMixCap, (targetLum - lum) / (1 - lum));
    }
    return {
      r: Math.min(255, Math.round(lifted.r + (255 - lifted.r) * w)),
      g: Math.min(255, Math.round(lifted.g + (255 - lifted.g) * w)),
      b: Math.min(255, Math.round(lifted.b + (255 - lifted.b) * w)),
    };
  }

  function applyColors(palette) {
    const root = document.documentElement;
    if (!palette) {
      root.style.setProperty("--theme-bg-dark", "#09090b");
      root.style.setProperty("--theme-tint", "#0f0f12");
      root.style.setProperty("--theme-tint-2", "#0b0b0d");
      root.style.setProperty("--theme-border", "rgba(255, 255, 255, 0.08)");
      root.style.setProperty("--theme-glow-1", "rgba(255, 255, 255, 0.05)");
      root.style.setProperty("--theme-glow-2", "rgba(200, 200, 210, 0.03)");
      root.style.setProperty("--theme-glow-3", "rgba(255, 255, 255, 0.02)");
      root.style.setProperty("--theme-ambient-dark", "#08080a");
      root.style.setProperty("--glow-color", "rgba(255, 255, 255, 0.50)");
      root.style.setProperty("--theme-lyrics-tint", "#ffffff");
      root.style.setProperty("--theme-lyrics-glow-core", "rgba(255, 255, 255, 0.42)");
      root.style.setProperty("--theme-lyrics-glow-outer", "rgba(210, 215, 225, 0.30)");
      root.style.setProperty("--theme-lyrics-glow-ambient", "rgba(170, 178, 195, 0.18)");
      root.style.setProperty("--theme-lyrics-backlight", "rgba(255, 255, 255, 0.0)");
      return;
    }

    if (palette.isMono) {
      const m = palette.primary;
      const bgDark = `rgb(${Math.round(m.r * 0.14)}, ${Math.round(m.g * 0.14)}, ${Math.round(m.b * 0.15)})`;
      const colorTint = `rgb(${Math.round(m.r * 0.24)}, ${Math.round(m.g * 0.24)}, ${Math.round(m.b * 0.26)})`;
      const colorTint2 = `rgb(${Math.round(m.r * 0.17)}, ${Math.round(m.g * 0.17)}, ${Math.round(m.b * 0.19)})`;
      const ambientDark = `rgb(${Math.round(m.r * 0.12)}, ${Math.round(m.g * 0.12)}, ${Math.round(m.b * 0.13)})`;

      root.style.setProperty("--theme-bg-dark", bgDark);
      root.style.setProperty("--theme-tint", colorTint);
      root.style.setProperty("--theme-tint-2", colorTint2);
      root.style.setProperty("--theme-border", `rgba(${m.r}, ${m.g}, ${m.b}, 0.26)`);
      root.style.setProperty("--theme-glow-1", `rgba(${m.r + 45}, ${m.g + 45}, ${m.b + 48}, 0.30)`);
      root.style.setProperty("--theme-glow-2", `rgba(${m.r + 20}, ${m.g + 22}, ${m.b + 26}, 0.22)`);
      root.style.setProperty("--theme-glow-3", `rgba(${m.r + 35}, ${m.g + 35}, ${m.b + 40}, 0.16)`);
      root.style.setProperty("--theme-ambient-dark", ambientDark);
      root.style.setProperty("--glow-color", "rgba(235, 240, 248, 0.56)");
      root.style.setProperty("--theme-lyrics-tint", "#ffffff");
      root.style.setProperty("--theme-lyrics-glow-core", "rgba(255, 255, 255, 0.46)");
      root.style.setProperty("--theme-lyrics-glow-outer", "rgba(215, 224, 238, 0.38)");
      root.style.setProperty("--theme-lyrics-glow-ambient", "rgba(180, 192, 212, 0.22)");
      root.style.setProperty("--theme-lyrics-backlight", "rgba(225, 232, 245, 0.0)");
      return;
    }

    const p = liftColor(palette.primary, 130, 235);
    const s = liftColor(palette.secondary || palette.primary, 115, 220);

    const bgDark = `rgb(${Math.max(12, Math.round(p.r * 0.14))}, ${Math.max(12, Math.round(p.g * 0.14))}, ${Math.max(12, Math.round(p.b * 0.14))})`;
    const colorTint = `rgb(${Math.max(20, Math.round(p.r * 0.28))}, ${Math.max(18, Math.round(p.g * 0.28))}, ${Math.max(20, Math.round(p.b * 0.28))})`;
    const colorTint2 = `rgb(${Math.max(15, Math.round(s.r * 0.22))}, ${Math.max(14, Math.round(s.g * 0.22))}, ${Math.max(16, Math.round(s.b * 0.22))})`;
    const ambientDark = `rgb(${Math.max(10, Math.round((p.r * 0.5 + s.r * 0.5) * 0.14))}, ${Math.max(10, Math.round((p.g * 0.5 + s.g * 0.5) * 0.14))}, ${Math.max(12, Math.round((p.b * 0.5 + s.b * 0.5) * 0.14))})`;

    const border = `rgba(${p.r}, ${p.g}, ${p.b}, 0.36)`;
    const glow1 = `rgba(${p.r}, ${p.g}, ${p.b}, 0.52)`;
    const glow2 = `rgba(${s.r}, ${s.g}, ${s.b}, 0.42)`;
    const glow3 = `rgba(${Math.round((p.r + s.r) / 2)}, ${Math.round((p.g + s.g) / 2)}, ${Math.round((p.b + s.b) / 2)}, 0.26)`;

    const coreRgb = ensureGlowRgb(p, 0.62, 0.42);
    const outerRgb = ensureGlowRgb(p, 0.36, 0.16);
    const ambRgb = ensureGlowRgb(s, 0.34, 0.14);
    const textTint = {
      r: Math.min(255, Math.round(255 * 0.92 + coreRgb.r * 0.08)),
      g: Math.min(255, Math.round(255 * 0.92 + coreRgb.g * 0.08)),
      b: Math.min(255, Math.round(255 * 0.92 + coreRgb.b * 0.08)),
    };

    root.style.setProperty("--theme-bg-dark", bgDark);
    root.style.setProperty("--theme-tint", colorTint);
    root.style.setProperty("--theme-tint-2", colorTint2);
    root.style.setProperty("--theme-border", border);
    root.style.setProperty("--theme-glow-1", glow1);
    root.style.setProperty("--theme-glow-2", glow2);
    root.style.setProperty("--theme-glow-3", glow3);
    root.style.setProperty("--theme-ambient-dark", ambientDark);
    root.style.setProperty("--glow-color", `rgba(${coreRgb.r}, ${coreRgb.g}, ${coreRgb.b}, 0.60)`);
    root.style.setProperty("--theme-lyrics-tint", `rgb(${textTint.r}, ${textTint.g}, ${textTint.b})`);
    root.style.setProperty("--theme-lyrics-glow-core", `rgba(${coreRgb.r}, ${coreRgb.g}, ${coreRgb.b}, 0.52)`);
    root.style.setProperty("--theme-lyrics-glow-outer", `rgba(${outerRgb.r}, ${outerRgb.g}, ${outerRgb.b}, 0.58)`);
    root.style.setProperty("--theme-lyrics-glow-ambient", `rgba(${ambRgb.r}, ${ambRgb.g}, ${ambRgb.b}, 0.34)`);
    root.style.setProperty("--theme-lyrics-backlight", `rgba(${outerRgb.r}, ${outerRgb.g}, ${outerRgb.b}, 0.0)`);
  }

  let colorCanvas = null;
  let colorCtx = null;
  function sampleImageData32(image) {
    if (!colorCanvas) {
      colorCanvas = document.createElement("canvas");
      colorCanvas.width = 32;
      colorCanvas.height = 32;
      colorCtx = colorCanvas.getContext("2d", { willReadFrequently: true });
    }
    if (!colorCtx) return null;
    colorCtx.clearRect(0, 0, 32, 32);
    colorCtx.drawImage(image, 0, 0, 32, 32);
    return colorCtx.getImageData(0, 0, 32, 32).data;
  }

  function extractAndApplyColor(url) {
    if (!url) {
      applyColors(null);
      return;
    }
    if (colorCache.has(url)) {
      if (url === lastCoverSrc) applyColors(colorCache.get(url));
      return;
    }

    const img = new Image();
    img.crossOrigin = "anonymous";
    let done = false;
    const clearHandlers = () => {
      img.onload = null;
      img.onerror = null;
    };
    img.onload = () => {
      if (done) return;
      done = true;
      try {
        const data = sampleImageData32(img);
        clearHandlers();
        if (!data) throw new Error("no ctx");
        const colors = processImageData(data);
        rememberColor(url, colors);
        if (url === lastCoverSrc) applyColors(colors);
      } catch {
        clearHandlers();
        fetchColorFromBlob(url);
      }
    };
    img.onerror = () => {
      if (done) return;
      done = true;
      clearHandlers();
      fetchColorFromBlob(url);
    };
    img.src = url;
  }

  function fetchColorFromBlob(url) {
    fetch(url, { mode: "cors" })
      .then((r) => r.blob())
      .then((blob) => {
        const blobUrl = URL.createObjectURL(blob);
        const bImg = new Image();
        const cleanup = () => {
          URL.revokeObjectURL(blobUrl);
          bImg.onload = null;
          bImg.onerror = null;
        };
        bImg.onload = () => {
          try {
            const data = sampleImageData32(bImg);
            cleanup();
            if (!data) throw new Error("no ctx");
            const colors = processImageData(data);
            rememberColor(url, colors);
            if (url === lastCoverSrc) applyColors(colors);
          } catch {
            cleanup();
            if (url === lastCoverSrc) applyColors(null);
          }
        };
        bImg.onerror = () => {
          cleanup();
          if (url === lastCoverSrc) applyColors(null);
        };
        bImg.src = blobUrl;
      })
      .catch(() => {
        if (url === lastCoverSrc) applyColors(null);
      });
  }

  function syncState() {
    installTitlebar();
    if (typeof document !== "undefined" && document.hidden) return;

    // 1. Sync Title for Titlebar and Windows Taskbar
    let trackTitle = "";
    if (navigator.mediaSession && navigator.mediaSession.metadata) {
      const meta = navigator.mediaSession.metadata;
      if (meta.title) {
        trackTitle = meta.artist ? `${meta.title} • ${meta.artist}` : meta.title;
      }
    }
    if (!trackTitle && document.title && !document.title.startsWith("YTM") && document.title !== "YouTube Music") {
      trackTitle = document.title;
    }
    const nextHeaderTitle = trackTitle ? `${trackTitle} — YouTube Music` : "YouTube Music";
    const titleEl = document.getElementById("liebe-titlebar-title");
    if (titleEl && titleEl.textContent !== nextHeaderTitle) {
      titleEl.textContent = nextHeaderTitle;
    }
    const desiredWindowTitle = `${trackTitle} - YouTube Music`;
    if (
      trackTitle &&
      !document.title.startsWith("YTM") &&
      document.title !== desiredWindowTitle
    ) {
      document.title = desiredWindowTitle;
    }

    // 2. Sync Color (prefer currently rendered cover images, fall back to mediaSession)
    let coverUrl = "";
    const songImg = document.querySelector("#song-image yt-img-shadow > img[src], #song-image img[src]");
    const barImg = document.querySelector("ytmusic-player-bar .image.ytmusic-player-bar img[src], ytmusic-player-bar img[src], [slot='player-bar'] img[src], ytmusic-miniplayer img[src]");
    if (songImg && songImg.src && !songImg.src.startsWith("data:")) {
      coverUrl = songImg.src;
    } else if (barImg && barImg.src && !barImg.src.startsWith("data:")) {
      coverUrl = barImg.src;
    } else if (navigator.mediaSession && navigator.mediaSession.metadata && navigator.mediaSession.metadata.artwork) {
      const art = navigator.mediaSession.metadata.artwork;
      if (Array.isArray(art) && art.length > 0) {
        coverUrl = art[art.length - 1].src || art[0].src;
      }
    }

    if (coverUrl && coverUrl !== lastCoverSrc) {
      lastCoverSrc = coverUrl;
      extractAndApplyColor(coverUrl);
    } else if (!coverUrl && lastCoverSrc) {
      lastCoverSrc = "";
      applyColors(null);
    }
  }

  setInterval(syncState, 1500);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) syncState();
  });
  window.addEventListener("ytm-track-change", syncState);
  window.addEventListener("load", syncState);
  document.addEventListener("DOMContentLoaded", syncState);
  if (document.readyState !== "loading") syncState();
})();
