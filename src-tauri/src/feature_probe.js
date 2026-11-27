(() => {
  if (typeof location === "object" && location.hostname && location.hostname !== "music.youtube.com") {
    return;
  }
  if (window.__ytmFeatures) return;

  const features = new Map();
  const requests = new Map();
  const queue = [];
  let requestId = 0;
  let sending = false;
  let config = window.__ytmFeatureConfig && typeof window.__ytmFeatureConfig === "object" ? { ...window.__ytmFeatureConfig } : {};

  function releaseTitle(item) {
    if (item.reassertTimer) {
      clearTimeout(item.reassertTimer);
      item.reassertTimer = 0;
    }
    if (document.title === item.title) {
      document.title = item.previousTitle;
    }
  }

  function enqueue(item) {
    if (item.message?.kind === "translate") {
      queue.push(item);
    } else {
      // Interactive requests (lyrics lookup, settings changes, actions) preempt background translation chunks
      const firstTranslateIdx = queue.findIndex((queued) => queued.message?.kind === "translate");
      if (firstTranslateIdx >= 0) {
        queue.splice(firstTranslateIdx, 0, item);
      } else {
        queue.push(item);
      }
    }
    flushQueue();
  }

  function flushQueue() {
    if (sending || !queue.length) return;
    sending = true;
    const item = queue.shift();
    item.timeout = setTimeout(() => {
      requests.delete(item.id);
      if (item.reassertTimer) {
        clearTimeout(item.reassertTimer);
        item.reassertTimer = 0;
      }
      if (!item.acked) {
        releaseTitle(item);
        sending = false;
        flushQueue();
      }
      item.reject(new Error("feature request timed out"));
    }, 5_000);
    requests.set(item.id, item);
    item.message.ts = Date.now();
    item.previousTitle = document.title.startsWith("YTMFEATURE:") ? "YouTube Music" : document.title;
    item.title = `YTMFEATURE:${JSON.stringify(item.message)}`;
    document.title = item.title;

    let reassertCount = 0;
    const checkTitlePreserved = () => {
      item.reassertTimer = 0;
      if (item.acked || !requests.has(item.id) || reassertCount >= 6) return;
      if (document.title !== item.title) {
        if (!document.title.startsWith("YTMFEATURE:") && !document.title.startsWith("YTMSPOTIFY:")) {
          item.previousTitle = document.title;
        }
        reassertCount++;
        document.title = item.title;
      }
      item.reassertTimer = setTimeout(checkTitlePreserved, 90);
    };
    item.reassertTimer = setTimeout(checkTitlePreserved, 90);
  }

  const api = {
    get config() {
      return config;
    },
    media() {
      return document.querySelector("video, audio");
    },
    app() {
      return document.querySelector("ytmusic-app");
    },
    register(name, feature) {
      features.set(name, { ...feature, active: false });
      apply(name);
    },
    configure(next) {
      config = next && typeof next === "object" ? { ...next } : {};
      for (const name of features.keys()) apply(name);
      try {
        window.dispatchEvent(new CustomEvent("ytm-settings-changed", { detail: { ...config } }));
      } catch {}
    },
    getUpdateStatus() {
      const id = ++requestId;
      return new Promise((resolve, reject) => {
        enqueue({ id, message: { id, kind: "get_update_status" }, resolve, reject });
      });
    },
    getAppStatus() {
      const id = ++requestId;
      return new Promise((resolve, reject) => {
        enqueue({ id, message: { id, kind: "get_app_status" }, resolve, reject });
      });
    },
    getSettings() {
      const id = ++requestId;
      const message = { id, kind: "get_settings" };
      return new Promise((resolve, reject) => {
        enqueue({ id, message, resolve, reject });
      });
    },
    setSetting(key, value) {
      const hadPrevious = Object.prototype.hasOwnProperty.call(config, key);
      const previousValue = config[key];
      config[key] = value;
      for (const name of features.keys()) apply(name);
      try {
        window.dispatchEvent(new CustomEvent("ytm-settings-changed", { detail: { ...config } }));
      } catch {}

      const id = ++requestId;
      const message = { id, kind: "set_setting", key, value };
      const pending = new Promise((resolve, reject) => {
        enqueue({ id, message, resolve, reject });
      });
      return pending.then(
        (response) => {
          if (response?.settings) api.configure(response.settings);
          return response;
        },
        (error) => {
          if (hadPrevious) config[key] = previousValue;
          else delete config[key];
          for (const name of features.keys()) apply(name);
          try {
            window.dispatchEvent(new CustomEvent("ytm-settings-changed", { detail: { ...config } }));
          } catch {}
          throw error;
        },
      );
    },
    triggerAction(action) {
      const id = ++requestId;
      const message = { id, kind: "action", action };
      return new Promise((resolve, reject) => {
        enqueue({ id, message, resolve, reject });
      });
    },
    request(url, init = {}) {
      const id = ++requestId;
      const message = {
        id,
        kind: "http",
        url,
        method: String(init.method || "GET").toUpperCase(),
        body: typeof init.body === "string" ? init.body : null,
        headers: init.headers && typeof init.headers === "object" ? init.headers : {},
      };

      return new Promise((resolve, reject) => {
        enqueue({ id, message, resolve, reject });
      });
    },
    lyrics(track) {
      const id = ++requestId;
      const message = { id, kind: "lyrics", value: track && typeof track === "object" ? track : {} };
      return new Promise((resolve, reject) => {
        enqueue({ id, message, resolve, reject });
      });
    },
    translate(target, lines) {
      const id = ++requestId;
      const message = {
        id,
        kind: "translate",
        value: {
          target: String(target || "ru"),
          lines: Array.isArray(lines) ? lines.map((line) => String(line || "")) : [],
        },
      };
      return new Promise((resolve, reject) => {
        enqueue({ id, message, resolve, reject });
      });
    },
    // frees title channel
    ack(id) {
      const pending = requests.get(id);
      if (!pending || pending.acked) return;
      pending.acked = true;
      if (pending.reassertTimer) {
        clearTimeout(pending.reassertTimer);
        pending.reassertTimer = 0;
      }
      clearTimeout(pending.timeout);
      pending.timeout = setTimeout(() => {
        requests.delete(id);
        pending.reject(new Error("feature request timed out"));
      }, 60_000);
      releaseTitle(pending);
      sending = false;
      if (queue.length) {
        setTimeout(flushQueue, 10);
      }
    },
    receive(id, response) {
      const pending = requests.get(id);
      if (!pending) return;
      clearTimeout(pending.timeout);
      requests.delete(id);
      if (response?.error) pending.reject(new Error(response.error));
      else pending.resolve(response);
      if (pending.acked) return;
      releaseTitle(pending);
      sending = false;
      if (queue.length) {
        setTimeout(flushQueue, 10);
      }
    },
  };

  function apply(name) {
    const feature = features.get(name);
    if (!feature) return;

    const enabled = Boolean(config[name]);
    if (enabled === feature.active) {
      try {
        feature.update?.(config, api);
      } catch {}
      return;
    }

    if (enabled) {
      if (!document.documentElement && document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", () => apply(name), { once: true });
        return;
      }
      try {
        feature.start?.(config, api);
        feature.active = true;
      } catch {}
    } else {
      try {
        feature.stop?.(api);
      } catch {}
      feature.active = false;
    }
  }

  document.addEventListener("DOMContentLoaded", () => {
    for (const name of features.keys()) apply(name);
  }, { once: true });

  const PLAYER_BAR_TEXT_SELECTORS = [
    "ytmusic-player-bar .content-info-wrapper",
    "[slot='player-bar'] .content-info-wrapper",
    "[slot='player-bar'] .ytMusicMiniPlayerTrackInfo",
    "[slot='player-bar'] .ytMusicSmallViewportMiniplayerTrackInfo",
    "ytmusic-player-bar .time-info",
    "[slot='player-bar'] .ytMusicMiniPlayerTimeInfo",
  ];

  const PLAYER_BAR_TEXT_CLOSEST = PLAYER_BAR_TEXT_SELECTORS.join(", ");

  const ALLOW_TEXT_CLOSEST = [
    "#synced-lyrics-container",
    ".synced-lyrics-wrapper",
    ".synced-lyrics-vlist",
    ".synced-line",
    ".text-lyrics",
    ".lyrics-line",
    "ytmusic-description-shelf-renderer",
    "ytmusic-tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS']",
    "#tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS']",
    ".description.ytmusic-description-shelf-renderer",
    "ytmusic-search-box",
    ...PLAYER_BAR_TEXT_SELECTORS,
  ].join(", ");

  const getEventElements = (event) => {
    const nodes = typeof event?.composedPath === "function" ? event.composedPath() : [event?.target];
    const elements = [];
    for (const raw of nodes) {
      if (!raw) continue;
      const el = raw.nodeType === 3 ? raw.parentElement : raw;
      if (el) elements.push(el);
    }
    return elements;
  };

  const hasClosest = (event, selector) =>
    getEventElements(event).some((el) => typeof el.closest === "function" && el.closest(selector));

  const isAllowedTarget = (event) =>
    getEventElements(event).some((el) => {
      const tag = el.tagName?.toUpperCase?.();
      return (
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        Boolean(el.isContentEditable) ||
        (typeof el.closest === "function" && Boolean(el.closest(ALLOW_TEXT_CLOSEST)))
      );
    });
  const blockContextMenu = (e) => {
    const hasSelection =
      typeof window?.getSelection === "function" && String(window.getSelection() || "").length > 0;
    if (hasSelection && hasClosest(e, PLAYER_BAR_TEXT_CLOSEST)) {
      // Keep browser native context menu ("Copy") instead of YouTube Music's custom track menu
      e.stopPropagation();
      e.stopImmediatePropagation?.();
      return;
    }
    if (isAllowedTarget(e)) return;
    e.preventDefault();
    e.stopPropagation();
    return false;
  };

  // block context menu except in editable fields, search box, song lyrics, and player bar track info
  if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
    window.addEventListener("contextmenu", blockContextMenu, true);
  }
  if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
    let playerBarDrag = null;
    let suppressNextClickUntil = 0;

    document.addEventListener("contextmenu", blockContextMenu, true);
    document.addEventListener("selectstart", (e) => {
      if (isAllowedTarget(e)) return;
      e.preventDefault();
      return false;
    }, true);
    document.addEventListener("dragstart", (e) => {
      if (hasClosest(e, PLAYER_BAR_TEXT_CLOSEST)) {
        e.preventDefault();
      }
    }, true);
    document.addEventListener("mousedown", (e) => {
      if (e?.button !== 0 || !hasClosest(e, PLAYER_BAR_TEXT_CLOSEST)) {
        playerBarDrag = null;
        return;
      }
      playerBarDrag = { x: Number(e.clientX) || 0, y: Number(e.clientY) || 0, moved: false };
    }, true);
    document.addEventListener("mousemove", (e) => {
      if (!playerBarDrag || playerBarDrag.moved) return;
      const dx = (Number(e.clientX) || 0) - playerBarDrag.x;
      const dy = (Number(e.clientY) || 0) - playerBarDrag.y;
      if (dx * dx + dy * dy >= 16) {
        playerBarDrag.moved = true;
      }
    }, true);
    document.addEventListener("mouseup", () => {
      if (playerBarDrag?.moved) {
        const selected = typeof window?.getSelection === "function" ? String(window.getSelection() || "") : "";
        if (selected.length > 0) {
          suppressNextClickUntil = Date.now() + 350;
        }
      }
      playerBarDrag = null;
    }, true);
    document.addEventListener("click", (e) => {
      if (Date.now() <= suppressNextClickUntil && hasClosest(e, PLAYER_BAR_TEXT_CLOSEST)) {
        suppressNextClickUntil = 0;
        e.preventDefault();
        e.stopPropagation();
        e.stopImmediatePropagation?.();
      }
    }, true);
  }

  if (typeof document !== "undefined" && typeof document.createElement === "function") {
    const guardStyle = document.createElement("style");
    guardStyle.id = "ytm-guard-style";
    guardStyle.textContent = `
      *, *::before, *::after {
        -webkit-user-select: none !important;
        -moz-user-select: none !important;
        -ms-user-select: none !important;
        user-select: none !important;
      }
      input, textarea, [contenteditable="true"], [contenteditable=""],
      ytmusic-search-box, ytmusic-search-box * {
        -webkit-user-select: text !important;
        -moz-user-select: text !important;
        -ms-user-select: text !important;
        user-select: text !important;
      }
      ytmusic-player-bar .content-info-wrapper,
      ytmusic-player-bar .content-info-wrapper *,
      [slot="player-bar"] .content-info-wrapper,
      [slot="player-bar"] .content-info-wrapper *,
      [slot="player-bar"] .ytMusicMiniPlayerTrackInfo,
      [slot="player-bar"] .ytMusicMiniPlayerTrackInfo *,
      [slot="player-bar"] .ytMusicSmallViewportMiniplayerTrackInfo,
      [slot="player-bar"] .ytMusicSmallViewportMiniplayerTrackInfo *,
      ytmusic-player-bar .time-info,
      [slot="player-bar"] .ytMusicMiniPlayerTimeInfo {
        -webkit-user-select: text !important;
        -moz-user-select: text !important;
        -ms-user-select: text !important;
        user-select: text !important;
        -webkit-user-drag: none !important;
        user-drag: none !important;
        cursor: default !important;
      }
      ytmusic-player-bar .content-info-wrapper a,
      ytmusic-player-bar .content-info-wrapper a *,
      [slot="player-bar"] .content-info-wrapper a,
      [slot="player-bar"] .content-info-wrapper a *,
      [slot="player-bar"] .ytMusicMiniPlayerTrackInfo a,
      [slot="player-bar"] .ytMusicMiniPlayerTrackInfo a *,
      [slot="player-bar"] .ytMusicSmallViewportMiniplayerTrackInfo a,
      [slot="player-bar"] .ytMusicSmallViewportMiniplayerTrackInfo a * {
        cursor: pointer !important;
      }
      #synced-lyrics-container,
      #synced-lyrics-container *,
      .synced-line,
      .synced-line *,
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
      }
      #synced-lyrics-container ::selection,
      ytmusic-description-shelf-renderer ::selection,
      ytmusic-player-bar ::selection,
      [slot="player-bar"] ::selection {
        background: rgba(255, 255, 255, 0.28) !important;
        color: #ffffff !important;
      }
    `;
    const injectGuard = () => {
      const target = document.head || document.documentElement || document.body;
      if (target && typeof target.appendChild === "function") {
        if (!document.getElementById?.("ytm-guard-style")) target.appendChild(guardStyle);
      } else {
        setTimeout(injectGuard, 50);
      }
    };
    injectGuard();
  }

  function isRendered(element) {
    if (typeof element.getClientRects === "function") {
      return element.getClientRects().length > 0;
    }
    return element.offsetParent !== null;
  }

  // unblock clicks
  function clearOrphanedInert() {
    if (typeof document?.querySelector !== "function") {
      return;
    }
    const layout = document.querySelector("ytmusic-app-layout[inert]");
    if (!layout) {
      return;
    }
    const dialog = document.querySelector(
      "tp-yt-paper-dialog:not([style*='display: none']), ytmusic-dialog:not([style*='display: none']), #ytm-settings-modal.open, #ytm-restart-dialog"
    );
    if (dialog && isRendered(dialog)) {
      return;
    }
    layout.removeAttribute("inert");
    if (typeof document.querySelectorAll === "function") {
      for (const backdrop of document.querySelectorAll("tp-yt-iron-overlay-backdrop.opened")) {
        backdrop.classList?.remove?.("opened");
        backdrop.removeAttribute?.("opened");
        if (backdrop.style) {
          backdrop.style.pointerEvents = "none";
        }
      }
    }
  }

  if (typeof window?.addEventListener === "function") {
    window.addEventListener("pointerdown", clearOrphanedInert, true);
    window.addEventListener("focus", clearOrphanedInert);
  }
  if (typeof document?.addEventListener === "function") {
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") clearOrphanedInert();
    });
  }
  if (typeof setInterval === "function") {
    setInterval(clearOrphanedInert, 2000);
  }

  window.__ytmFeatures = api;
  api.configure(window.__ytmFeatureConfig || {});
})();
