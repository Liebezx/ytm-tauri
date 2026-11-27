(() => {
  if (typeof location === "object" && location.hostname && location.hostname !== "music.youtube.com") {
    return;
  }
  const runtime = window.__ytmFeatures;
  if (!runtime) return;

  const PROVIDERS = ["Unified"];
  const STYLE_ID = "ytm-tauri-synced-lyrics-style";
  const CONTAINER_ID = "synced-lyrics-container";
  const STAR_KEY = "ytmd-sl-starred-";
  const CACHE_LIMIT = 15;
  const CACHE = new Map();
  const TRANSLATION_CACHE = new Map();

  let headerObserver = null;
  let observedHeader = null;
  let updateInterval = 0;
  let trackPollInterval = 0;
  let activeTrack = null;
  let currentTrackEpoch = 0;
  let currentAbortController = null;
  let currentLineElements = [];
  let currentLineOffsets = null;
  let cachedContainerHeight = 0;
  let resizeObserver = null;
  let scrollRafId = 0;
  let currentProvider = PROVIDERS[0];
  let manuallySwitched = false;
  let renderVersion = 0;
  let lastCurrentIndex = -1;
  let isUserScrolling = false;
  let userScrollTimeout = 0;
  let running = false;
  let boundPlayer = null;
  let boundMedia = null;
  let isPointerSelecting = false;
  let pointerEndListener = null;

  const state = () => CACHE.get(activeTrack?.videoId)?.providers || null;
  const config = () => runtime.config || {};
  const clean = (value) => String(value || "").replace(/\s+/g, " ").trim();

  const styles = `
/* hide original tab contents ONLY while the Lyrics tab itself is active */
html[data-ytm-lyrics-tab-active="true"] [data-ytm-lyrics-host="true"] > *:not(#${CONTAINER_ID}),
html[data-ytm-lyrics-tab-active="true"] #tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'] > *:not(#${CONTAINER_ID}),
html[data-ytm-lyrics-tab-active="true"] ytmusic-tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'] > *:not(#${CONTAINER_ID}),
html[data-ytm-lyrics-tab-active="true"] ytmusic-player-page ytmusic-tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'] > *:not(#${CONTAINER_ID}) {
  display: none !important;
}

#${CONTAINER_ID} {
  display: none !important;
}

html[data-ytm-lyrics-tab-active="true"] [data-ytm-lyrics-host="true"] > #${CONTAINER_ID},
html[data-ytm-lyrics-tab-active="true"] #${CONTAINER_ID},
ytmusic-tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'] > #${CONTAINER_ID} {
  display: flex !important;
  flex-direction: column !important;
  height: 100% !important;
  min-height: 0 !important;
  width: 100% !important;
  box-sizing: border-box !important;
  position: relative !important;
  z-index: 10 !important;
  background: transparent !important;
  -webkit-user-select: text !important;
  user-select: text !important;
}

html[data-ytm-lyrics-tab-active="true"] [data-ytm-lyrics-host="true"],
ytmusic-tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'],
#tab-renderer[page-type='MUSIC_PAGE_TYPE_TRACK_LYRICS'] {
  scrollbar-width: none !important;
  display: flex !important;
  flex-direction: column !important;
  height: 100% !important;
  min-height: 0 !important;
}

#tabsContent > tp-yt-paper-tab,
#tabsContent > .tab-header {
  pointer-events: auto !important;
  cursor: pointer !important;
}

@property --lyrics-duration {
  syntax: '<time>';
  inherits: false;
  initial-value: 2s;
}

:root {
  /* layout */
  --global-margin: clamp(8px, 0.7vmax, 14px);
  --lyrics-padding: 0.22em;

  /* typography (explicit px so YouTube Music's 10px html root & zoom-out never shrink lyrics) */
  --lyrics-font-family: Satoshi, Avenir, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Open Sans", "Helvetica Neue", sans-serif;
  --lyrics-font-size: max(clamp(20px, 1.95vmax, 32px), calc(16px / var(--ytm-zoom, 1)));
  --lyrics-line-height: var(--ytmusic-body-line-height, 1.45);
  --lyrics-width: 100%;

  /* inactive lyrics */
  --lyrics-inactive-font-weight: 400;
  --lyrics-inactive-opacity: 0.33;
  --lyrics-inactive-scale: 1;
  --lyrics-inactive-offset: 0;

  /* active lyrics */
  --lyrics-active-font-weight: 700;
  --lyrics-active-opacity: 1;
  --lyrics-active-scale: 1;
  --lyrics-active-offset: 0;

  --lyrics-duration: 2s;

  /* animations */
  --lyrics-animations: lyrics-glow var(--lyrics-glow-duration, 2s) forwards, lyrics-wobble var(--lyrics-wobble-duration, 1s) forwards;
  --lyrics-scale-duration: 0.166s;
  --lyrics-opacity-transition: 0.33s;
  --lyrics-glow-duration: var(--lyrics-duration, 2s);
  --lyrics-wobble-duration: calc(var(--lyrics-duration, 2s) / 2);

  /* colors (inherited from dynamic theme variables) */
  --lyrics-active-color: var(--theme-lyrics-tint, #ffffff);
  --lyrics-active-shadow:
    0 0 1px rgba(255, 255, 255, 0.32),
    0 0 14px var(--theme-lyrics-glow-core, rgba(255, 255, 255, 0.45)),
    0 0 34px var(--theme-lyrics-glow-outer, rgba(210, 218, 232, 0.45)),
    0 0 56px var(--theme-lyrics-glow-ambient, rgba(175, 188, 210, 0.24));

  /* effects defaults */
  --lyrics-inactive-blur: 0.4px;
  --lyrics-active-blur: 0px;
  --lyrics-hover-blur: 0px;
  --lyrics-hover-opacity: 0.88;
  --lyrics-letter-spacing: -0.012em;
  --lyrics-vlist-mask: linear-gradient(to bottom, black 0%, black 84%, transparent 100%);
}

/* line effects */
html[data-lyrics-effect="cinematic"], :root[data-lyrics-effect="cinematic"] {
  --lyrics-font-size: max(clamp(20px, 2.0vmax, 32px), calc(16px / var(--ytm-zoom, 1)));
  --lyrics-line-height: 1.4;
  --lyrics-width: 100%;
  --lyrics-padding: 0.28em;
  --lyrics-inactive-font-weight: 600;
  --lyrics-inactive-opacity: 0.32;
  --lyrics-inactive-scale: 0.96;
  --lyrics-inactive-offset: 0;
  --lyrics-inactive-blur: 1.4px;
  --lyrics-active-font-weight: 700;
  --lyrics-active-opacity: 1;
  --lyrics-active-scale: 1.12;
  --lyrics-active-offset: 0;
  --lyrics-active-blur: 0px;
  --lyrics-hover-blur: 0.3px;
  --lyrics-hover-opacity: 0.82;
  --lyrics-animations: none;
  --lyrics-scale-duration: 0.44s;
  --lyrics-opacity-transition: 0.34s;
  --lyrics-vlist-mask: linear-gradient(to bottom, transparent 0%, black 7%, black 85%, transparent 100%);
  --lyrics-active-shadow:
    0 0 1px rgba(255, 255, 255, 0.30),
    0 0 14px var(--theme-lyrics-glow-core, rgba(255, 255, 255, 0.46)),
    0 0 32px var(--theme-lyrics-glow-outer, rgba(210, 218, 232, 0.46));
}

html[data-lyrics-effect="studio"], :root[data-lyrics-effect="studio"] {
  --lyrics-font-size: max(clamp(19px, 1.9vmax, 29px), calc(15.5px / var(--ytm-zoom, 1)));
  --lyrics-line-height: 1.42;
  --lyrics-width: 100%;
  --lyrics-padding: 0.24em;
  --lyrics-letter-spacing: -0.016em;
  --lyrics-inactive-font-weight: 600;
  --lyrics-inactive-opacity: 0.34;
  --lyrics-inactive-scale: 0.955;
  --lyrics-inactive-offset: 0;
  --lyrics-inactive-blur: 0px;
  --lyrics-active-font-weight: 700;
  --lyrics-active-opacity: 1;
  --lyrics-active-scale: 1.095;
  --lyrics-active-offset: 0;
  --lyrics-active-blur: 0px;
  --lyrics-hover-blur: 0px;
  --lyrics-hover-opacity: 0.88;
  --lyrics-animations: none;
  --lyrics-scale-duration: 0.44s;
  --lyrics-opacity-transition: 0.32s;
  --lyrics-vlist-mask: linear-gradient(to bottom, black 0%, black 85%, transparent 100%);
  --lyrics-active-shadow:
    0 0 1px rgba(255, 255, 255, 0.34),
    0 0 12px var(--theme-lyrics-glow-core, rgba(255, 255, 255, 0.48)),
    0 0 24px var(--theme-lyrics-glow-outer, rgba(210, 218, 232, 0.46)),
    0 0 36px var(--theme-lyrics-glow-ambient, rgba(175, 188, 210, 0.24));
}

html[data-lyrics-effect="luminescent"], :root[data-lyrics-effect="luminescent"] {
  --lyrics-font-size: max(clamp(20px, 1.98vmax, 31px), calc(16px / var(--ytm-zoom, 1)));
  --lyrics-line-height: 1.4;
  --lyrics-width: 100%;
  --lyrics-padding: 0.26em;
  --lyrics-inactive-font-weight: 600;
  --lyrics-inactive-opacity: 0.32;
  --lyrics-inactive-scale: 0.96;
  --lyrics-inactive-offset: 0;
  --lyrics-inactive-blur: 0px;
  --lyrics-active-font-weight: 700;
  --lyrics-active-opacity: 1;
  --lyrics-active-scale: 1.125;
  --lyrics-active-offset: 0;
  --lyrics-active-blur: 0px;
  --lyrics-hover-blur: 0px;
  --lyrics-hover-opacity: 0.86;
  --lyrics-animations: lyrics-luminescent-shimmer 1.1s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  --lyrics-scale-duration: 0.44s;
  --lyrics-opacity-transition: 0.32s;
  --lyrics-vlist-mask: linear-gradient(to bottom, black 0%, black 85%, transparent 100%);
  --lyrics-active-shadow:
    0 0 1px rgba(255, 255, 255, 0.36),
    0 0 16px var(--theme-lyrics-glow-core, rgba(255, 255, 255, 0.52)),
    0 0 36px var(--theme-lyrics-glow-outer, rgba(210, 218, 232, 0.56)),
    0 0 58px var(--theme-lyrics-glow-ambient, rgba(175, 188, 210, 0.32));
}

html[data-lyrics-effect="fancy"], :root[data-lyrics-effect="fancy"] {
  --lyrics-font-size: max(clamp(22px, 2.15vmax, 34px), calc(17px / var(--ytm-zoom, 1)));
  --lyrics-line-height: 1.333;
  --lyrics-width: 100%;
  --lyrics-padding: 0.42em;
  --lyrics-inactive-font-weight: 700;
  --lyrics-inactive-opacity: 0.33;
  --lyrics-inactive-scale: 0.94;
  --lyrics-inactive-offset: 0;
  --lyrics-active-font-weight: 700;
  --lyrics-active-opacity: 1;
  --lyrics-active-scale: 1.08;
  --lyrics-active-offset: 0;
  --lyrics-animations: lyrics-glow var(--lyrics-glow-duration, 2s) forwards, lyrics-wobble var(--lyrics-wobble-duration, 1s) forwards;
  --lyrics-scale-duration: 0.42s;
  --lyrics-vlist-mask: linear-gradient(to bottom, black 0%, black 85%, transparent 100%);
  --lyrics-active-shadow:
    0 0 1px rgba(255, 255, 255, 0.34),
    0 0 16px var(--theme-lyrics-glow-core, rgba(255, 255, 255, 0.50)),
    0 0 34px var(--theme-lyrics-glow-outer, rgba(210, 218, 232, 0.52)),
    0 0 54px var(--theme-lyrics-glow-ambient, rgba(175, 188, 210, 0.28));
}

html[data-lyrics-effect="scale"], :root[data-lyrics-effect="scale"] {
  --lyrics-font-size: max(clamp(19px, 1.9vmax, 30px), calc(15.5px / var(--ytm-zoom, 1)));
  --lyrics-width: 100%;
  --lyrics-padding: 0.22em;
  --lyrics-animations: none;
  --lyrics-inactive-font-weight: 500;
  --lyrics-inactive-opacity: 0.34;
  --lyrics-inactive-scale: 0.96;
  --lyrics-inactive-offset: 0;
  --lyrics-inactive-blur: 0px;
  --lyrics-active-font-weight: 700;
  --lyrics-active-opacity: 1;
  --lyrics-active-scale: 1.18;
  --lyrics-active-offset: 0;
  --lyrics-scale-duration: 0.42s;
  --lyrics-active-shadow:
    0 0 14px var(--theme-lyrics-glow-core, rgba(255, 255, 255, 0.46)),
    0 0 30px var(--theme-lyrics-glow-outer, rgba(210, 218, 232, 0.42));
}

html[data-lyrics-effect="offset"], :root[data-lyrics-effect="offset"] {
  --lyrics-font-size: max(clamp(19px, 1.9vmax, 30px), calc(15.5px / var(--ytm-zoom, 1)));
  --lyrics-width: 100%;
  --lyrics-padding: 0.22em;
  --lyrics-animations: none;
  --lyrics-inactive-font-weight: 500;
  --lyrics-inactive-opacity: 0.34;
  --lyrics-inactive-scale: 1;
  --lyrics-inactive-offset: 0;
  --lyrics-inactive-blur: 0px;
  --lyrics-active-font-weight: 700;
  --lyrics-active-opacity: 1;
  --lyrics-active-scale: 1.04;
  --lyrics-active-offset: 4%;
  --lyrics-active-shadow:
    0 0 14px var(--theme-lyrics-glow-core, rgba(255, 255, 255, 0.44)),
    0 0 28px var(--theme-lyrics-glow-outer, rgba(210, 218, 232, 0.40));
}

html[data-lyrics-effect="focus"], :root[data-lyrics-effect="focus"] {
  --lyrics-font-size: max(clamp(19px, 1.9vmax, 30px), calc(15.5px / var(--ytm-zoom, 1)));
  --lyrics-width: 100%;
  --lyrics-padding: 0.22em;
  --lyrics-animations: none;
  --lyrics-inactive-font-weight: 500;
  --lyrics-inactive-opacity: 0.33;
  --lyrics-inactive-scale: 1;
  --lyrics-inactive-offset: 0;
  --lyrics-inactive-blur: 0px;
  --lyrics-active-font-weight: 700;
  --lyrics-active-opacity: 1;
  --lyrics-active-scale: 1.06;
  --lyrics-active-offset: 0;
  --lyrics-active-shadow:
    0 0 16px var(--theme-lyrics-glow-outer, rgba(255, 255, 255, 0.42));
}

.lyrics-renderer {
  display: flex !important;
  flex-direction: column !important;
  height: 100% !important;
  min-height: 0 !important;
  width: 100% !important;
}

.lyrics-picker-sticky {
  position: sticky;
  top: 0;
  z-index: 100;
  background: linear-gradient(to bottom, rgba(10,10,10,0.92), rgba(10,10,10,0.58), transparent);
  transform: translateY(var(--lyrics-picker-top, -60px));
  opacity: var(--lyrics-picker-opacity, 0);
  pointer-events: var(--lyrics-picker-pointer, none);
  transition: transform 280ms cubic-bezier(0.25, 1, 0.5, 1), opacity 220ms ease-in-out;
}

.lyrics-picker {
  height: 4.5em;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-around;
  padding-block: 0.6em;
  box-sizing: border-box;
}

.lyrics-picker-left,
.lyrics-picker-right {
  display: flex;
  justify-content: center;
  align-items: center;
  transition: background-color 0.3s ease;
  border-radius: 50%;
  width: 36px;
  height: 36px;
  background: rgba(255, 255, 255, 0.08);
  border: 0;
  color: #fff;
  cursor: pointer;
  font-size: 1.4rem;
}
.lyrics-picker-left:hover,
.lyrics-picker-right:hover {
  background: rgba(255, 255, 255, 0.2);
}

.lyrics-picker-content {
  display: flex;
  width: 50%;
  min-width: 180px;
  flex-direction: column;
  justify-content: space-around;
  align-items: center;
  gap: 4px;
}

.lyrics-picker-content-label {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
}

.lyrics-picker-item {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 1.25rem;
  font-weight: 600;
  color: #ffffff;
}

.lyrics-picker-star {
  border: 0;
  background: transparent;
  color: #ffca28;
  cursor: pointer;
  font-size: 18px;
  line-height: 1;
  padding: 0 4px;
}

.lyrics-picker-status {
  font-size: 15px;
  display: inline-flex;
  align-items: center;
}
.lyrics-picker-status.done { color: #81c784; }
.lyrics-picker-status.fetching { color: #ffb74d; }
.lyrics-picker-status.error { color: #e57373; }
.lyrics-picker-status.warning { color: #ffb74d; }

.lyrics-picker-content-dots {
  display: flex;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.lyrics-picker-dot {
  display: inline-block;
  cursor: pointer;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  border: 1px solid rgba(255,255,255,0.4);
  background: transparent;
  transition: background 0.2s, transform 0.2s;
}
.lyrics-picker-dot.active {
  background: #ffffff;
  border-color: #ffffff;
  transform: scale(1.2);
}

.synced-lyrics-vlist {
  flex: 1 1 auto !important;
  display: block !important;
  height: 100% !important;
  min-height: 0 !important;
  overflow-y: auto !important;
  overflow-x: hidden !important;
  scrollbar-width: none !important;
  padding: 18px 18px 44vh 14px !important;
  box-sizing: border-box !important;
  contain: layout style !important;
}
.synced-lyrics-vlist::-webkit-scrollbar { display: none; }

.synced-lyrics-vlist.is-plain .text-lyrics {
  filter: none !important;
  opacity: 0.92 !important;
  scale: 1 !important;
}
.synced-lyrics-vlist.is-plain .text-lyrics > span > span {
  opacity: 0.92 !important;
}

.synced-line {
  width: var(--lyrics-width, 100%);
  max-width: 100%;
  box-sizing: border-box;
  margin: var(--global-margin) 0;
  contain: layout style;
  transition: transform 0.35s cubic-bezier(0.16, 1, 0.3, 1);
  display: block;
  position: relative;
  overflow: visible;
}

.synced-line .text-lyrics {
  cursor: pointer;
  padding-left: 18px;
  padding-right: 28px;
  box-sizing: border-box;
  overflow-wrap: break-word;
}

.synced-line,
.synced-line *,
.text-lyrics,
.text-lyrics * {
  -webkit-user-select: text !important;
  user-select: text !important;
}

#${CONTAINER_ID} ::selection,
#${CONTAINER_ID} *::selection {
  background: rgba(255, 255, 255, 0.30) !important;
  color: #ffffff !important;
  text-shadow: none !important;
}

.text-lyrics {
  font-family: var(--lyrics-font-family) !important;
  font-size: var(--lyrics-font-size) !important;
  font-weight: var(--lyrics-inactive-font-weight) !important;
  line-height: var(--lyrics-line-height) !important;
  letter-spacing: var(--lyrics-letter-spacing, normal) !important;
  padding-top: var(--lyrics-padding);
  padding-bottom: var(--lyrics-padding);
  scale: var(--lyrics-inactive-scale);
  translate: var(--lyrics-inactive-offset);
  transition:
    scale var(--lyrics-scale-duration) cubic-bezier(0.16, 1, 0.3, 1),
    translate 0.36s cubic-bezier(0.16, 1, 0.3, 1),
    opacity var(--lyrics-opacity-transition);
  display: block;
  text-align: left;
  transform-origin: 18px 50%;
  color: #ffffff !important;
}

.text-lyrics > span > span {
  display: inline-block;
  white-space: pre-wrap;
  opacity: var(--lyrics-inactive-opacity);
  text-shadow: none;
  transition:
    opacity var(--lyrics-opacity-transition),
    color 0.35s ease;
  color: #ffffff !important;
}

/* Smooth Apple-Music-style depth gradient on neighbor lines right before/after current (O(1) JS class, no :has scan) */
.synced-line.near-current .text-lyrics {
  scale: 0.99;
}
.synced-line.near-current .text-lyrics > span > span {
  opacity: 0.56;
}

.synced-line.current .text-lyrics {
  font-weight: var(--lyrics-active-font-weight) !important;
  scale: var(--lyrics-active-scale);
  translate: var(--lyrics-active-offset);
  will-change: scale, opacity;
  color: var(--lyrics-active-color, var(--theme-lyrics-tint, #ffffff)) !important;
  opacity: 1 !important;
}

.synced-line.current .text-lyrics > span > span {
  opacity: var(--lyrics-active-opacity);
  animation: var(--lyrics-animations);
  color: var(--lyrics-active-color, var(--theme-lyrics-tint, #ffffff)) !important;
  text-shadow: var(--lyrics-active-shadow);
}

.text-lyrics > .romaji {
  color: var(--ytmusic-text-secondary, rgba(255, 255, 255, 0.7)) !important;
  font-size: calc(var(--lyrics-font-size) * 0.7) !important;
  font-style: italic !important;
  display: block;
  margin-top: 4px;
  opacity: 0.58;
}
.synced-line.current .text-lyrics > .romaji {
  opacity: 1;
  text-shadow: 0 0 12px var(--theme-lyrics-glow-outer, rgba(255, 255, 255, 0.35));
}

@keyframes lyrics-tr-fade-in {
  from { opacity: 0; transform: translateY(2px); }
  to { opacity: 1; }
}

#${CONTAINER_ID} .text-lyrics > .lyrics-translation {
  display: block;
  margin-top: 3px;
  font-size: max(clamp(12px, 1.05vmax, 15.5px), calc(11.5px / var(--ytm-zoom, 1))) !important;
  line-height: 1.32 !important;
  font-weight: 400 !important;
  letter-spacing: 0.005em !important;
  color: rgba(255, 255, 255, 0.36) !important;
  text-shadow: none !important;
  filter: none !important;
  -webkit-user-select: none !important;
  user-select: none !important;
  transition: color 0.36s cubic-bezier(0.16, 1, 0.3, 1);
  animation: lyrics-tr-fade-in 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

#${CONTAINER_ID} .synced-line.near-current .text-lyrics > .lyrics-translation {
  color: rgba(255, 255, 255, 0.46) !important;
}

#${CONTAINER_ID} .synced-line.current .text-lyrics > .lyrics-translation {
  color: rgba(255, 255, 255, 0.66) !important;
  text-shadow: none !important;
}

.text-lyrics > .timecode {
  color: var(--ytmusic-text-secondary, rgba(255, 255, 255, 0.5));
  font-size: 0.45em;
  font-weight: 500;
  display: block;
  margin-bottom: 2px;
}

.kaomoji-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  width: 100%;
  box-sizing: border-box;
  color: rgba(255, 255, 255, 0.7);
  font-family: var(--lyrics-font-family);
  font-size: max(22px, calc(18px / var(--ytm-zoom, 1)));
  user-select: text;
  text-align: center;
}

.error-container {
  padding: 24px;
  margin-bottom: 5%;
}
.error-box {
  background-color: rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  padding: 12px;
  color: #ff8a80;
  font-family: monospace;
  font-size: 14px;
  white-space: pre-wrap;
  word-break: break-all;
}
.retry-btn {
  margin-top: 14px;
  border: 0;
  border-radius: 18px;
  padding: 8px 20px;
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
  cursor: pointer;
  font-size: 14px;
}

@keyframes lyrics-wobble {
  from { transform: translateY(0px); }
  33.33% { transform: translateY(1.75px); }
  66.66% { transform: translateY(-1.75px); }
  to { transform: translateY(0px); }
}

@keyframes lyrics-glow {
  0% { opacity: 0.76; }
  to { opacity: var(--lyrics-active-opacity, 1); }
}

@keyframes lyrics-luminescent-shimmer {
  0% { opacity: 0.72; }
  45% { opacity: 1; }
  100% { opacity: var(--lyrics-active-opacity, 1); }
}

.line-seek-pulse {
  animation: line-seek-flash 0.45s ease-out !important;
}

@keyframes line-seek-flash {
  0% {
    transform: scale(1.04);
    filter: brightness(1.45) drop-shadow(0 0 14px var(--theme-lyrics-glow-core, rgba(255, 255, 255, 0.9)));
  }
  100% {
    transform: scale(1);
    filter: brightness(1) drop-shadow(0 0 0 transparent);
  }
}

.synced-line:not(.current):hover .text-lyrics {
  filter: blur(var(--lyrics-hover-blur, 0px));
}
.synced-line:not(.current):hover .text-lyrics > span > span {
  opacity: var(--lyrics-hover-opacity, 0.85);
  text-shadow: 0 0 12px var(--theme-lyrics-glow-outer, rgba(255, 255, 255, 0.35));
  transition-delay: 0s !important;
}
.synced-line .seek-hint-icon {
  position: absolute;
  left: 2px;
  top: 50%;
  transform: translateY(-50%) scale(0.8);
  opacity: 0;
  transition: opacity 0.15s ease, transform 0.15s ease;
  color: rgba(255, 255, 255, 0.6);
  pointer-events: none;
  display: flex;
  align-items: center;
}
.synced-line:hover .seek-hint-icon {
  opacity: 0.8;
  transform: translateY(-50%) scale(1);
}
.synced-line.current .seek-hint-icon {
  display: none !important;
}

.synced-line.instrumental .text-lyrics {
  opacity: 0.45;
  letter-spacing: 0.15em;
  font-size: calc(var(--lyrics-font-size) * 0.85) !important;
}

.synced-line.instrumental.current .text-lyrics {
  opacity: 1;
  animation: pulse-note 1.8s infinite ease-in-out;
}

@keyframes pulse-note {
  0%, 100% { transform: scale(1); filter: brightness(1); }
  50% { transform: scale(1.1); filter: brightness(1.3) drop-shadow(0 0 10px var(--theme-lyrics-glow-core, var(--glow-color, rgba(255,255,255,0.7)))); }
}

.lyrics-sync-btn {
  position: absolute;
  bottom: 24px;
  right: 28px;
  background: rgba(24, 24, 24, 0.92);
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 24px;
  padding: 7px 16px 7px 12px;
  color: #fff;
  font-size: 13px;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  z-index: 105;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
  opacity: 0;
  pointer-events: none;
  transform: translateY(10px) scale(0.95);
  transition: opacity 0.22s ease, transform 0.22s cubic-bezier(0.16, 1, 0.3, 1), background 0.15s ease;
  font-family: inherit;
}
.lyrics-sync-btn.visible {
  opacity: 1;
  pointer-events: auto;
  transform: translateY(0) scale(1);
}
.lyrics-sync-btn:hover {
  background: rgba(45, 45, 45, 0.98);
  border-color: rgba(255, 255, 255, 0.35);
}
.lyrics-sync-btn:active {
  transform: scale(0.96);
}
.lyrics-sync-btn svg {
  width: 14px;
  height: 14px;
  fill: currentColor;
}
`;

  function installStyle() {
    let style = document.getElementById(STYLE_ID);
    if (!style) {
      style = document.createElement("style");
      style.id = STYLE_ID;
      const target = document.head || document.documentElement || document.body;
      if (target) {
        target.appendChild(style);
      } else {
        document.addEventListener("DOMContentLoaded", () => {
          const t = document.head || document.documentElement || document.body;
          if (t && !document.getElementById(STYLE_ID)) t.appendChild(style);
        }, { once: true });
      }
    }
    style.textContent = styles;
  }

  function getTabHeaders() {
    return Array.from(document.querySelectorAll(
      "#tabsContent > tp-yt-paper-tab, #tabsContent > .tab-header, ytmusic-player-page tp-yt-paper-tabs tp-yt-paper-tab, ytmusic-player-page #tabsContent tp-yt-paper-tab, #tab-header"
    ));
  }

  function getLyricsTabHeader() {
    const tabs = getTabHeaders();
    for (const tab of tabs) {
      if (/lyrics|текст|слова|тексти|paroles|songtext|letra|testo|歌詞|가사/i.test(tab.textContent || "")) return tab;
    }
    return tabs[1] || document.querySelector("#tabsContent > .tab-header:nth-of-type(2), #tabsContent > tp-yt-paper-tab:nth-of-type(2)");
  }

  function isTabHeaderSelected(tab) {
    if (!tab) return false;
    return tab.getAttribute("aria-selected") === "true" ||
      Boolean(tab.classList?.contains?.("iron-selected")) ||
      tab.active === true;
  }

  function isLyricsTabSelected() {
    const tabs = getTabHeaders();
    const header = getLyricsTabHeader();
    if (header) {
      if (isTabHeaderSelected(header)) return true;
      // If another tab header IS explicitly selected (e.g. Up Next / Comments / Related), Lyrics is definitely not active
      if (tabs.some(isTabHeaderSelected)) return false;
    }
    if (tabs.length > 0 && tabs.some(isTabHeaderSelected)) return false;
    return Boolean(document.querySelector('[page-type="MUSIC_PAGE_TYPE_TRACK_LYRICS"]'));
  }

  function clearLyricsHosts() {
    const hosts = document.querySelectorAll?.("[data-ytm-lyrics-host]");
    if (hosts) {
      for (const h of hosts) h.removeAttribute("data-ytm-lyrics-host");
    }
  }

  function syncTabVisibility() {
    const selected = isLyricsTabSelected();
    if (document?.documentElement?.dataset) {
      document.documentElement.dataset.ytmLyricsTabActive = selected ? "true" : "false";
    }
    if (!selected) {
      clearLyricsHosts();
      const existing = document.getElementById(CONTAINER_ID);
      if (existing) existing.remove();
    }
    return selected;
  }

  function getLyricsTabRenderer() {
    const selected = syncTabVisibility();
    if (!selected) return null;
    return document.querySelector(
      'ytmusic-tab-renderer[page-type="MUSIC_PAGE_TYPE_TRACK_LYRICS"], #tab-renderer > ytmusic-tab-renderer[page-type="MUSIC_PAGE_TYPE_TRACK_LYRICS"], #tab-renderer[page-type="MUSIC_PAGE_TYPE_TRACK_LYRICS"], ytmusic-player-page ytmusic-tab-renderer[page-type="MUSIC_PAGE_TYPE_TRACK_LYRICS"], ytmusic-player-page #tab-renderer, #side-panel #tab-renderer, ytmusic-player-page ytmusic-tab-renderer'
    );
  }

  const suffixesToRemove = [
    // artist names
    /\s*(- topic)$/i,
    /\s*vevo$/i,
    // video titles
    /\s*[(|[](official|audio|video|lyrics?|visualizer|remaster(ed)?|demo|live|extended|draft|special edition|deluxe|explicit|clean|4k|hd|hq|performance|clip|full album|slowed|reverb|sped up|sped-up|slowed\s*\+\s*reverb|color coded|eng sub|rom|han|eng|lyrics video).*?[)|\]]/gi,
    /\s*[(|[](20\d\d|19\d\d)\s*(remaster|version|edition|mix|anniversary)?[)|\]]/gi,
    /\s*[(|[](hd|hq|4k|upgrade|live|acoustic|instrumental)[)|\]]$/gi,
  ];

  function cleanupName(name) {
    if (!name) return "";
    let str = String(name);
    for (const suffix of suffixesToRemove) {
      str = str.replace(suffix, "");
    }
    return str.trim();
  }

  const KANA_MAP = {
    "あ": "a", "い": "i", "う": "u", "え": "e", "お": "o",
    "か": "ka", "き": "ki", "く": "ku", "け": "ke", "こ": "ko",
    "さ": "sa", "し": "shi", "す": "su", "せ": "se", "そ": "so",
    "た": "ta", "ち": "chi", "つ": "tsu", "て": "te", "と": "to",
    "な": "na", "に": "ni", "ぬ": "nu", "ね": "ne", "の": "no",
    "は": "ha", "ひ": "hi", "ふ": "fu", "へ": "he", "ほ": "ho",
    "ま": "ma", "み": "mi", "む": "mu", "め": "me", "も": "mo",
    "や": "ya", "ゆ": "yu", "よ": "yo",
    "ら": "ra", "り": "ri", "る": "ru", "れ": "re", "ろ": "ro",
    "わ": "wa", "を": "wo", "ん": "n",
    "が": "ga", "ぎ": "gi", "ぐ": "gu", "げ": "ge", "ご": "go",
    "ざ": "za", "じ": "ji", "ず": "zu", "ぜ": "ze", "ぞ": "zo",
    "だ": "da", "ぢ": "ji", "づ": "zu", "デ": "de", "ど": "do",
    "ば": "ba", "び": "bi", "ぶ": "bu", "べ": "be", "ぼ": "bo",
    "ぱ": "pa", "ぴ": "pi", "ぷ": "pu", "ぺ": "pe", "ぽ": "po",
    "きゃ": "kya", "きゅ": "kyu", "きょ": "kyo",
    "しゃ": "sha", "しゅ": "shu", "しょ": "sho",
    "ちゃ": "cha", "ちゅ": "chu", "ちょ": "cho",
    "にゃ": "nya", "にゅ": "nyu", "にょ": "nyo",
    "ひゃ": "hya", "ひゅ": "hyu", "ひょ": "hyo",
    "みゃ": "mya", "みゅ": "myu", "みょ": "myo",
    "りゃ": "rya", "りゅ": "ryu", "りょ": "ryo",
    "ぎゃ": "gya", "ぎゅ": "gyu", "ぎょ": "gyo",
    "じゃ": "ja", "じゅ": "ju", "ジョ": "jo",
    "びゃ": "bya", "びゅ": "byu", "びょ": "byo",
    "ぴゃ": "pya", "ぴゅ": "pyu", "ぴょ": "pyo",
    "ア": "a", "イ": "i", "ウ": "u", "エ": "e", "オ": "o",
    "カ": "ka", "キ": "ki", "ク": "ku", "ケ": "ke", "コ": "ko",
    "サ": "sa", "シ": "shi", "ス": "su", "セ": "se", "ソ": "so",
    "タ": "ta", "チ": "chi", "ツ": "tsu", "テ": "te", "ト": "to",
    "ナ": "na", "ニ": "ni", "ヌ": "nu", "ネ": "ne", "ノ": "no",
    "ハ": "ha", "ヒ": "hi", "フ": "fu", "ヘ": "he", "ホ": "ho",
    "マ": "ma", "ミ": "mi", "ム": "mu", "メ": "me", "モ": "mo",
    "ヤ": "ya", "ユ": "yu", "ヨ": "yo",
    "ラ": "ra", "リ": "ri", "ル": "ru", "レ": "re", "ロ": "ro",
    "ワ": "wa", "ヲ": "wo", "ン": "n",
    "ガ": "ga", "ギ": "gi", "グ": "gu", "ゲ": "ge", "ゴ": "go",
    "ザ": "za", "ジ": "ji", "ズ": "zu", "ゼ": "ze", "ゾ": "zo",
    "ダ": "da", "ヂ": "ji", "ヅ": "zu", "デ": "de", "ド": "do",
    "バ": "ba", "ビ": "bi", "ブ": "bu", "ベ": "be", "ボ": "bo",
    "パ": "pa", "ピ": "pi", "プ": "pu", "ペ": "pe", "ポ": "po",
    "キャ": "kya", "キュ": "kyu", "キョ": "kyo",
    "シャ": "sha", "シュ": "shu", "ショ": "sho",
    "チャ": "cha", "チュ": "chu", "チョ": "cho",
    "ニャ": "nya", "ニュ": "nyu", "ニョ": "nyo",
    "ヒャ": "hya", "ヒュ": "hyu", "ヒョ": "hyo",
    "ミャ": "mya", "ミュ": "myu", "ミョ": "myo",
    "リャ": "rya", "リュ": "ryu", "リョ": "ryo",
    "ギャ": "gya", "ギュ": "gyu", "ギョ": "gyo",
    "ジャ": "ja", "ジュ": "ju", "ジョ": "jo",
    "ビャ": "bya", "ビュ": "byu", "ビョ": "byo",
    "ピャ": "pya", "ピュ": "pyu", "ピョ": "pyo",
    "ー": "-",
  };

  const HANGUL_INITS = ["g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p", "h"];
  const HANGUL_MEDS = ["a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we", "wi", "yu", "eu", "ui", "i"];
  const HANGUL_FINS = ["", "k", "k", "ks", "n", "nj", "nh", "t", "l", "lg", "lm", "lb", "ls", "lt", "lp", "lh", "m", "p", "ps", "s", "ss", "ng", "j", "ch", "k", "t", "p", "h"];

  function romanizeText(text) {
    if (!text) return "";
    let out = "";
    let i = 0;
    while (i < text.length) {
      const code = text.charCodeAt(i);
      if (code >= 0xac00 && code <= 0xd7a3) {
        const syl = code - 0xac00;
        const init = Math.floor(syl / 588);
        const med = Math.floor((syl % 588) / 28);
        const fin = syl % 28;
        out += HANGUL_INITS[init] + HANGUL_MEDS[med] + HANGUL_FINS[fin];
        i++;
        continue;
      }
      if (i + 1 < text.length) {
        const pair = text.slice(i, i + 2);
        if (KANA_MAP[pair]) {
          out += KANA_MAP[pair];
          i += 2;
          continue;
        }
      }
      const char = text[i];
      if ((char === "っ" || char === "ッ") && i + 1 < text.length) {
        const nextPair = text.slice(i + 1, i + 3);
        const nextSingle = text[i + 1];
        const nextRom = KANA_MAP[nextPair] || KANA_MAP[nextSingle];
        if (nextRom) {
          out += nextRom[0];
          i++;
          continue;
        }
      }
      out += KANA_MAP[char] || char;
      i++;
    }
    return out !== text ? out : "";
  }

  function insertInstrumentalBreaks(lines) {
    if (!lines || !lines.length) return lines || [];
    const result = [];
    if (lines[0].timeInMs > 7500) {
      result.push({
        time: "00:00.00",
        timeInMs: 0,
        duration: lines[0].timeInMs,
        text: "♪ Instrumental ♪",
        isInstrumental: true,
        words: [],
      });
    }
    for (let i = 0; i < lines.length; i++) {
      result.push(lines[i]);
      if (i < lines.length - 1) {
        const lineDur = lines[i].duration < 60000 ? lines[i].duration : 3000;
        const gap = lines[i + 1].timeInMs - (lines[i].timeInMs + lineDur);
        if (gap > 9500) {
          const startTime = lines[i].timeInMs + lineDur;
          result.push({
            time: formatTime(startTime),
            timeInMs: startTime,
            duration: gap,
            text: "♪ Instrumental ♪",
            isInstrumental: true,
            words: [],
          });
        }
      }
    }
    return result;
  }

  function getPlayer() {
    return document.getElementById("movie_player") || document.querySelector("#movie_player, .html5-video-player");
  }

  function trackInfo(override) {
    if (override && override.title) {
      const title = clean(cleanupName(override.title));
      const artist = clean(cleanupName(override.artist || ""));
      const album = clean(override.album || "");
      const duration = Number(override.duration_seconds || 0);
      let videoId = "";
      if (override.url) {
        try {
          videoId = new URL(override.url, location.origin).searchParams.get("v") || "";
        } catch {}
      }
      if (!videoId) {
        const watchHref = document.querySelector("ytmusic-player-bar .title a, ytmusic-player-bar a[href*='watch?v='], ytmusic-player-bar a[href*='v='], [slot='player-bar'] .title a, [slot='player-bar'] a[href*='watch?v='], [slot='player-bar'] a[href*='v=']")?.getAttribute("href");
        if (watchHref) {
          try {
            videoId = new URL(watchHref, location.origin).searchParams.get("v") || "";
          } catch {}
        }
      }
      if (!videoId) {
        videoId = title ? `${artist}-${title}` : "";
      }
      return {
        videoId,
        title,
        alternativeTitle: "",
        artist,
        album,
        songDuration: duration,
        tags: [],
      };
    }

    const player = getPlayer();
    const media = runtime.media();

    let title = clean(
      cleanupName(
        navigator.mediaSession?.metadata?.title ||
        document.querySelector("ytmusic-player-bar .title, [slot='player-bar'] .title")?.textContent ||
        document.querySelector("ytmusic-player-bar yt-formatted-string.title, [slot='player-bar'] yt-formatted-string.title")?.textContent ||
        player?.getVideoData?.()?.title
      )
    );
    let artist = clean(
      cleanupName(
        navigator.mediaSession?.metadata?.artist ||
        document.querySelector("ytmusic-player-bar .byline a, [slot='player-bar'] .byline a")?.textContent ||
        document.querySelector("ytmusic-player-bar .byline, [slot='player-bar'] .byline")?.textContent?.split(/[•·]/)[0] ||
        player?.getVideoData?.()?.author
      )
    );
    let album = clean(
      navigator.mediaSession?.metadata?.album ||
      document.querySelector("ytmusic-player-bar .byline, [slot='player-bar'] .byline")?.textContent?.split(/[•·]/)[1] ||
      player?.getPlayerResponse?.()?.videoDetails?.album ||
      ""
    );
    const duration = Number(
      media?.duration ||
      player?.getVideoData?.()?.length_seconds ||
      player?.getPlayerResponse?.()?.videoDetails?.lengthSeconds ||
      0
    );

    let videoId = "";
    const watchHref = document.querySelector("ytmusic-player-bar .title a, ytmusic-player-bar a[href*='watch?v='], ytmusic-player-bar a[href*='v='], [slot='player-bar'] .title a, [slot='player-bar'] a[href*='watch?v='], [slot='player-bar'] a[href*='v=']")?.getAttribute("href");
    if (watchHref) {
      try {
        videoId = new URL(watchHref, location.origin).searchParams.get("v") || "";
      } catch {}
    }
    if (!videoId) {
      try {
        videoId = new URL(location.href).searchParams.get("v") || "";
      } catch {}
    }
    if (!videoId) {
      videoId = player?.getVideoData?.()?.video_id || "";
    }
    if (!videoId) {
      videoId = title ? `${artist}-${title}` : "";
    }

    const playerResponse = player?.getPlayerResponse?.();
    const alternativeTitle = clean(
      playerResponse?.microformat?.microformatDataRenderer?.linkAlternates?.find?.((l) => l.title)?.title || ""
    );
    const tags = Array.isArray(playerResponse?.microformat?.microformatDataRenderer?.tags)
      ? playerResponse.microformat.microformatDataRenderer.tags
      : [];

    if (!title && !videoId) return null;

    return {
      videoId,
      title,
      alternativeTitle,
      artist,
      album,
      songDuration: duration,
      tags,
    };
  }

  function blankProviderState() {
    return Object.fromEntries(PROVIDERS.map((name) => [name, { state: "idle", data: null, error: null }]));
  }

  function touchCache(videoId, entry) {
    CACHE.delete(videoId);
    CACHE.set(videoId, entry);
  }

  function pruneCache() {
    while (CACHE.size > CACHE_LIMIT) {
      let staleKey = null;
      for (const key of CACHE.keys()) {
        if (key !== activeTrack?.videoId) {
          staleKey = key;
          break;
        }
      }
      if (!staleKey) return;
      CACHE.delete(staleKey);
    }
  }

  const REMEMBER_KEY = "ytmd-sl-selected-";

  function readRemembered(videoId) {
    if (!videoId) return null;
    try {
      const val = localStorage.getItem(REMEMBER_KEY + videoId) || sessionStorage.getItem(REMEMBER_KEY + videoId);
      return PROVIDERS.includes(val) ? val : null;
    } catch {
      return null;
    }
  }

  function saveRemembered(videoId, provider) {
    if (!videoId || !provider) return;
    try {
      localStorage.setItem(REMEMBER_KEY + videoId, provider);
      sessionStorage.setItem(REMEMBER_KEY + videoId, provider);
    } catch {}
  }

  function ensureTrack(info, epoch = currentTrackEpoch, signal = null) {
    let entry = CACHE.get(info.videoId);
    const prevDuration = Number(entry?.songDuration || 0);
    const nextDuration = Number(info.songDuration || 0);
    const shouldRefreshForDuration = Boolean(
      entry &&
      prevDuration <= 0 &&
      nextDuration > 0 &&
      !entry.providers?.Unified?.data?.lines?.length
    );

    if (!entry) {
      const initialProvider = PROVIDERS[0];
      entry = {
        status: "loading",
        songDuration: nextDuration,
        providers: blankProviderState(),
        selectedProvider: initialProvider,
      };
      touchCache(info.videoId, entry);
      pruneCache();
      currentProvider = initialProvider;
      for (const name of PROVIDERS) fetchProvider(name, info, entry, epoch, signal);
    } else {
      touchCache(info.videoId, entry);
      if (shouldRefreshForDuration) {
        entry.songDuration = nextDuration;
        if (entry.providers.Unified) {
          entry.providers.Unified.state = "idle";
          entry.providers.Unified.data = null;
        }
      }
      if (entry.selectedProvider && PROVIDERS.includes(entry.selectedProvider)) {
        currentProvider = entry.selectedProvider;
      } else {
        currentProvider = PROVIDERS[0];
      }
      // recover idle or aborted fetch
      for (const name of PROVIDERS) {
        const p = entry.providers[name];
        if (!p || p.state === "idle" || (p.state === "fetching" && p.epoch !== epoch)) {
          fetchProvider(name, info, entry, epoch, signal);
        }
      }
    }
    return entry;
  }

  async function fetchProvider(name, info, entry, epoch = currentTrackEpoch, signal = null) {
    const provider = entry.providers[name];
    if (provider.state === "fetching" || (provider.state === "done" && provider.data)) {
      if (provider.state === "fetching" && provider.epoch !== epoch) {
        provider.state = "idle";
      } else {
        return;
      }
    }
    const requestId = (provider.requestId || 0) + 1;
    provider.requestId = requestId;
    provider.epoch = epoch;
    provider.state = "fetching";
    provider.data = null;
    provider.error = null;
    if (epoch === currentTrackEpoch && CACHE.get(info.videoId) === entry) {
      render();
    }
    try {
      const data = await Promise.race([
        providerSearch(name, info, epoch, signal),
        new Promise((_, reject) => setTimeout(() => reject(new Error("Lyrics lookup timed out.")), 12000)),
      ]);
      if (provider.requestId !== requestId) return;
      if (signal?.aborted || epoch !== currentTrackEpoch) {
        // abort cleanup
        provider.state = "idle";
        provider.data = null;
        return;
      }
      provider.state = "done";
      provider.data = data;
    } catch (error) {
      if (provider.requestId !== requestId) return;
      if (error?.name === "AbortError" || signal?.aborted || epoch !== currentTrackEpoch) {
        // abort cleanup
        provider.state = "idle";
        provider.data = null;
        return;
      }
      provider.state = "error";
      provider.error = error instanceof Error ? error : new Error(String(error));
    }
    entry.status = PROVIDERS.every((providerName) => entry.providers[providerName].state !== "fetching") ? "done" : "loading";
    if (activeTrack?.videoId === info.videoId && CACHE.get(info.videoId) === entry) {
      if (!entry.selectedProvider || !usable(entry.selectedProvider)) {
        autoPickProvider();
      }
      render();
    }
  }

  function providerBias(name) {
    const item = state()?.[name];
    if (!item) return -99;
    return (item.state === "done" ? 1 : -1)
      + (item.data?.lines?.length ? 2 : -1)
      + (item.data?.lines?.length && name === "YTMusic" ? 1 : 0)
      + (item.data?.lyrics ? 1 : -1);
  }

  function usable(name) {
    const data = state()?.[name]?.data;
    return Boolean(data?.lines?.length || data?.lyrics);
  }

  function autoPickProvider() {
    if (manuallySwitched || !activeTrack) return;
    const starred = readStarred(activeTrack.videoId);
    if (starred && usable(starred)) {
      currentProvider = starred;
      const entry = CACHE.get(activeTrack.videoId);
      if (entry) entry.selectedProvider = currentProvider;
      saveRemembered(activeTrack.videoId, currentProvider);
      return;
    }
    const remembered = readRemembered(activeTrack.videoId);
    if (remembered && usable(remembered)) {
      currentProvider = remembered;
      const entry = CACHE.get(activeTrack.videoId);
      if (entry) entry.selectedProvider = currentProvider;
      return;
    }
    const available = PROVIDERS.filter(usable).sort((a, b) => providerBias(b) - providerBias(a));
    if (available.length && (providerBias(available[0]) > providerBias(currentProvider) || !usable(currentProvider))) {
      currentProvider = available[0];
      const entry = CACHE.get(activeTrack.videoId);
      if (entry) entry.selectedProvider = currentProvider;
      saveRemembered(activeTrack.videoId, currentProvider);
    }
  }

  function readStarred(videoId) {
    if (!videoId) return null;
    try {
      const value = JSON.parse(localStorage.getItem(STAR_KEY + videoId) || "null");
      return PROVIDERS.includes(value?.provider) ? value.provider : null;
    } catch {
      return null;
    }
  }

  function toggleStar() {
    if (!activeTrack) return;
    const key = STAR_KEY + activeTrack.videoId;
    if (readStarred(activeTrack.videoId) === currentProvider) {
      localStorage.removeItem(key);
    } else {
      localStorage.setItem(key, JSON.stringify({ provider: currentProvider }));
    }
    const entry = CACHE.get(activeTrack.videoId);
    if (entry) entry.selectedProvider = currentProvider;
    saveRemembered(activeTrack.videoId, currentProvider);
    render();
  }

  function selectProvider(index, manual = true) {
    currentProvider = PROVIDERS[(index + PROVIDERS.length) % PROVIDERS.length];
    if (manual) manuallySwitched = true;
    if (activeTrack) {
      const entry = CACHE.get(activeTrack.videoId);
      if (entry) entry.selectedProvider = currentProvider;
      saveRemembered(activeTrack.videoId, currentProvider);
    }
    render();
  }

  function providerSearch(name, info, epoch = currentTrackEpoch, signal = null) {
    switch (name) {
      case "Unified": return fetchUnified(info, epoch, signal);
      case "LRCLib": return fetchLrcLib(info, epoch, signal);
      default: return fetchUnified(info, epoch, signal);
    }
  }

  async function resolveYtmBrowseId(info, epoch, signal) {
    try {
      // Check already-loaded watch tabs in DOM first (0ms), fallback to short 150ms /next peek so external race never stalls
      const pageData = document.querySelector("ytmusic-player-page")?.data || runtime.app()?.data;
      const cachedTabs = pageData?.contents?.singleColumnMusicWatchNextResultsRenderer?.tabbedRenderer?.watchNextTabbedResultsRenderer?.tabs;
      if (Array.isArray(cachedTabs)) {
        const tab = cachedTabs.find((item) => item?.tabRenderer?.endpoint?.browseEndpoint?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType === "MUSIC_PAGE_TYPE_TRACK_LYRICS");
        const id = tab?.tabRenderer?.endpoint?.browseEndpoint?.browseId;
        if (id) return id;
      }
      const app = runtime.app();
      const manager = app?.networkManager;
      if (!manager?.fetch || !info.videoId) return null;
      const fetchPromise = manager.fetch("/next?prettyPrint=false", { videoId: info.videoId, signal }).catch(() => null);
      const timeoutPromise = new Promise((resolve) => setTimeout(() => resolve(null), 150));
      const data = await Promise.race([fetchPromise, timeoutPromise]);
      if (epoch !== currentTrackEpoch || signal?.aborted || !data) return null;
      const tabs = data?.contents?.singleColumnMusicWatchNextResultsRenderer?.tabbedRenderer?.watchNextTabbedResultsRenderer?.tabs;
      if (!Array.isArray(tabs)) return null;
      const tab = tabs.find((item) => item?.tabRenderer?.endpoint?.browseEndpoint?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType === "MUSIC_PAGE_TYPE_TRACK_LYRICS");
      return tab?.tabRenderer?.endpoint?.browseEndpoint?.browseId || null;
    } catch {
      return null;
    }
  }

  async function fetchUnified(info, epoch = currentTrackEpoch, signal = null) {
    if (epoch !== currentTrackEpoch || signal?.aborted || typeof runtime.lyrics !== "function") {
      return null;
    }
    const browseId = await resolveYtmBrowseId(info, epoch, signal);
    if (epoch !== currentTrackEpoch || signal?.aborted) return null;
    const query = {
      title: String(info.title || ""),
      artist: String(info.artist || ""),
      album: info.album ? String(info.album) : null,
      duration: Number(info.songDuration) > 0 ? Number(info.songDuration) : 0,
    };
    if (browseId) {
      query.browse_id = String(browseId);
    }
    return nativeLyrics(query, epoch, signal);
  }

  // native lyrics lookup
  async function fetchLrcLib(info, epoch = currentTrackEpoch, signal = null) {
    if (epoch !== currentTrackEpoch || signal?.aborted || typeof runtime.lyrics !== "function") {
      return null;
    }
    return nativeLyrics({
      title: String(info.title || ""),
      artist: String(info.artist || ""),
      album: info.album ? String(info.album) : null,
      duration: Number(info.songDuration) > 0 ? Number(info.songDuration) : 0,
    }, epoch, signal);
  }

  async function nativeLyrics(query, epoch, signal) {
    if (typeof runtime.lyrics !== "function") return null;
    const response = await runtime.lyrics(query);
    if (epoch !== currentTrackEpoch || signal?.aborted || !response?.body) return null;
    try {
      const data = JSON.parse(response.body);
      return data && (data.lines?.length || data.lyrics) ? data : null;
    } catch {
      return null;
    }
  }

  function formatTime(ms) {
    const minutes = Math.floor(ms / 60_000);
    const seconds = Math.floor((ms % 60_000) / 1_000);
    const centiseconds = Math.floor((ms % 1_000) / 10);
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${String(centiseconds).padStart(2, "0")}`;
  }

  function ensureContainer() {
    if (!syncTabVisibility()) {
      return null;
    }
    let container = document.getElementById(CONTAINER_ID);
    const tabRenderer = getLyricsTabRenderer();
    if (!tabRenderer) {
      clearLyricsHosts();
      if (container) container.remove();
      return null;
    }

    if (tabRenderer.getAttribute?.("data-ytm-lyrics-host") !== "true") {
      clearLyricsHosts();
      tabRenderer.setAttribute?.("data-ytm-lyrics-host", "true");
    }
    if (!container) {
      container = document.createElement("div");
      container.id = CONTAINER_ID;
    }
    if (container.parentElement !== tabRenderer) {
      tabRenderer.appendChild(container);
    }
    return container;
  }

  let observedTabsBar = null;
  let tabsBarObserver = null;

  function onLyricsHeaderClick() {
    const prev = document?.documentElement?.dataset?.ytmLyricsTabActive === "true";
    const next = syncTabVisibility();
    if (next !== prev || (next && !document.getElementById(CONTAINER_ID))) {
      render();
    }
    setTimeout(() => {
      const was = document?.documentElement?.dataset?.ytmLyricsTabActive === "true";
      const now = syncTabVisibility();
      if (now !== was || (now && !document.getElementById(CONTAINER_ID))) render();
    }, 30);
    setTimeout(() => {
      const was = document?.documentElement?.dataset?.ytmLyricsTabActive === "true";
      const now = syncTabVisibility();
      if (now !== was || (now && !document.getElementById(CONTAINER_ID))) render();
    }, 180);
  }

  function enableHeaderTab(header) {
    header.removeAttribute("disabled");
    header.removeAttribute("aria-disabled");
    try {
      if ("disabled" in header && header.disabled) header.disabled = false;
    } catch {}
  }

  function setupHeaderObserver() {
    const header = getLyricsTabHeader();
    if (header) {
      enableHeaderTab(header);

      if (observedHeader !== header) {
        if (headerObserver) {
          headerObserver.disconnect();
        }
        if (observedHeader) {
          observedHeader.removeEventListener("click", onLyricsHeaderClick);
        }
        observedHeader = header;
        header.addEventListener("click", onLyricsHeaderClick);
        headerObserver = new MutationObserver((mutations) => {
          for (const mutation of mutations) {
            if (mutation.attributeName === "disabled") {
              enableHeaderTab(header);
            } else if (mutation.attributeName === "aria-selected" || mutation.attributeName === "class") {
              const prev = document?.documentElement?.dataset?.ytmLyricsTabActive === "true";
              const next = syncTabVisibility();
              if (next !== prev || (next && !document.getElementById(CONTAINER_ID))) render();
            }
          }
        });
        headerObserver.observe(header, { attributes: true, attributeFilter: ["disabled", "aria-selected", "class"] });
      }
    }

    const tabsBar = header?.parentElement || document.getElementById("tabsContent");
    if (tabsBar && observedTabsBar !== tabsBar) {
      if (tabsBarObserver) tabsBarObserver.disconnect();
      if (observedTabsBar) observedTabsBar.removeEventListener("click", onLyricsHeaderClick);
      observedTabsBar = tabsBar;
      tabsBar.addEventListener("click", onLyricsHeaderClick);
      tabsBarObserver = new MutationObserver(() => {
        const prev = document?.documentElement?.dataset?.ytmLyricsTabActive === "true";
        const next = syncTabVisibility();
        if (next !== prev || (next && !document.getElementById(CONTAINER_ID))) render();
      });
      tabsBarObserver.observe(tabsBar, { subtree: true, attributes: true, attributeFilter: ["aria-selected", "class"] });
    }
    syncTabVisibility();
  }

  let isProgrammaticScroll = false;
  let programmaticScrollTimer = null;

  let isInitialTrackScroll = true;

  function scrollToLineIndex(index, behavior = "smooth") {
    const container = document.getElementById(CONTAINER_ID);
    if (!container) return;
    const vlist = container.querySelector(".synced-lyrics-vlist");
    if (!vlist) return;

    const lineCount = currentLineElements.length;
    if (index >= 0 && index < lineCount) {
      let targetTop = 0;
      if (currentLineOffsets && currentLineOffsets.length > index) {
        targetTop = currentLineOffsets[index];
      } else {
        targetTop = currentLineElements[index]?.offsetTop || 0;
      }
      const cHeight = cachedContainerHeight || vlist.clientHeight || 400;
      const top = targetTop - cHeight * 0.38;
      isProgrammaticScroll = true;
      if (programmaticScrollTimer) window.clearTimeout(programmaticScrollTimer);
      const actualBehavior = isInitialTrackScroll ? "instant" : behavior;
      isInitialTrackScroll = false;
      if (typeof vlist.scrollTo === "function") {
        vlist.scrollTo({ top: Math.max(0, top), behavior: actualBehavior });
      } else {
        vlist.scrollTop = Math.max(0, top);
      }
      programmaticScrollTimer = window.setTimeout(() => {
        isProgrammaticScroll = false;
      }, 400);
    }
  }

  function hasActiveLyricsSelection() {
    try {
      const sel = window.getSelection?.();
      if (!sel || sel.isCollapsed || !sel.toString().trim()) return false;
      const container = document.getElementById(CONTAINER_ID);
      if (!container) return false;
      const anchor = sel.anchorNode?.nodeType === 3 ? sel.anchorNode.parentElement : sel.anchorNode;
      const focus = sel.focusNode?.nodeType === 3 ? sel.focusNode.parentElement : sel.focusNode;
      return Boolean((anchor && container.contains?.(anchor)) || (focus && container.contains?.(focus)));
    } catch {
      return false;
    }
  }

  function scrollToActiveLine() {
    if (isUserScrolling || isPointerSelecting || hasActiveLyricsSelection()) return;
    const current = state()?.[currentProvider]?.data?.lines;
    if (!current?.length) return;

    const player = getPlayer();
    const currentTimeSec = typeof player?.getCurrentTime === "function"
      ? player.getCurrentTime()
      : (runtime.media()?.currentTime || 0);
    const currentTimeMs = currentTimeSec * 1000;
    const currentIndex = current.findLastIndex((line) => line.timeInMs <= currentTimeMs);
    scrollToLineIndex(currentIndex);
  }

  function updateSyncButtonVisibility() {
    const container = document.getElementById(CONTAINER_ID);
    const syncBtn = container?.querySelector(".lyrics-sync-btn");
    if (syncBtn) {
      syncBtn.classList.toggle("visible", isUserScrolling);
    }
  }

  function onUserScroll() {
    if (isProgrammaticScroll) return;
    isUserScrolling = true;
    updateSyncButtonVisibility();
    if (userScrollTimeout) window.clearTimeout(userScrollTimeout);
    if (config().lyrics_auto_sync !== false) {
      userScrollTimeout = window.setTimeout(() => {
        isUserScrolling = false;
        updateSyncButtonVisibility();
        scrollToActiveLine();
      }, 3000);
    }
  }

  const VALID_EFFECTS = new Set(["fancy", "scale", "offset", "focus", "cinematic", "studio", "luminescent"]);

  function applyEffect() {
    const raw = config().lyrics_line_effect;
    const effect = VALID_EFFECTS.has(raw) ? raw : "fancy";
    if (document?.documentElement?.dataset) {
      document.documentElement.dataset.lyricsEffect = effect;
    }
  }

  function render() {
    if (!running) return;
    if (resizeObserver) {
      resizeObserver.disconnect();
      resizeObserver = null;
    }
    if (scrollRafId) {
      window.cancelAnimationFrame(scrollRafId);
      scrollRafId = 0;
    }
    currentLineElements = [];
    currentLineOffsets = null;
    cachedContainerHeight = 0;
    const version = ++renderVersion;
    applyEffect();
    const container = ensureContainer();
    if (!container) return;

    if (activeTrack) {
      const entry = CACHE.get(activeTrack.videoId);
      if (entry?.selectedProvider && PROVIDERS.includes(entry.selectedProvider)) {
        currentProvider = entry.selectedProvider;
      } else {
        currentProvider = PROVIDERS[0];
      }
    }

    const current = state()?.[currentProvider];

    container.replaceChildren();
    const root = document.createElement("div");
    root.className = "lyrics-renderer";

    const vlist = document.createElement("div");
    vlist.className = "synced-lyrics-vlist";
    vlist.addEventListener("scroll", () => {
      vlist.classList.toggle("is-scrolled", vlist.scrollTop > 10);
    }, { passive: true });
    vlist.addEventListener("mousedown", (e) => {
      if (e.button === 0) isPointerSelecting = true;
    }, { passive: true });
    vlist.addEventListener("wheel", onUserScroll, { passive: true });
    vlist.addEventListener("touchmove", onUserScroll, { passive: true });
    root.appendChild(vlist);
    container.appendChild(root);

    container.onmousemove = null;
    container.onmouseleave = null;

    const syncBtn = document.createElement("button");
    syncBtn.type = "button";
    syncBtn.className = `lyrics-sync-btn${isUserScrolling ? " visible" : ""}`;
    syncBtn.innerHTML = `
      <svg viewBox="0 0 24 24"><path d="M12 4V1L8 5l4 4V6c3.31 0 6 2.69 6 6 0 1.01-.25 1.97-.7 2.8l1.46 1.46A7.93 7.93 0 0020 12c0-4.42-3.58-8-8-8zm0 14c-3.31 0-6-2.69-6-6 0-1.01.25-1.97.7-2.8L5.24 7.74A7.93 7.93 0 004 12c0 4.42 3.58 8 8 8v3l4-4-4-4v3z"/></svg>
      <span>Sync</span>
    `;
    syncBtn.addEventListener("click", () => {
      isUserScrolling = false;
      if (userScrollTimeout) window.clearTimeout(userScrollTimeout);
      syncBtn.classList.remove("visible");
      scrollToActiveLine();
    });
    container.appendChild(syncBtn);

    if (!current || current.state === "fetching") {
      renderLoadingKaomoji(vlist);
      return;
    }
    if (current.state === "error") {
      renderErrorDisplay(vlist, current.error?.message || "Failed to fetch lyrics.", () => retryCurrentProvider(version));
      return;
    }
    if (current.data?.lines?.length) {
      renderSynced(vlist, current.data.lines, version);
      return;
    }
    if (current.data?.lyrics) {
      renderPlain(vlist, current.data.lyrics, version);
      return;
    }
    renderNotFoundKaomoji(vlist);
  }

  function pickerButton(glyph, className, onClick) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = className;
    button.textContent = glyph;
    button.addEventListener("click", onClick);
    return button;
  }

  function pickerCenter(current, starred) {
    const center = document.createElement("div");
    center.className = "lyrics-picker-content";

    const labelRow = document.createElement("div");
    labelRow.className = "lyrics-picker-content-label";

    const item = document.createElement("div");
    item.className = "lyrics-picker-item";

    const status = document.createElement("span");
    const statusClass = current?.state === "fetching"
      ? "fetching"
      : current?.state === "error"
      ? "error"
      : current?.data?.lines?.length || current?.data?.lyrics
      ? "done"
      : "warning";
    status.className = `lyrics-picker-status ${statusClass}`;
    status.textContent = current?.state === "fetching" ? "…" : current?.state === "error" ? "✕" : current?.data?.lines?.length ? "✓" : current?.data?.lyrics ? "≡" : "!";
    item.appendChild(status);

    const title = document.createElement("span");
    title.textContent = currentProvider;
    item.appendChild(title);

    const star = document.createElement("button");
    star.type = "button";
    star.className = "lyrics-picker-star";
    star.textContent = starred === currentProvider ? "★" : "☆";
    star.addEventListener("click", toggleStar);
    item.appendChild(star);

    labelRow.appendChild(item);
    center.appendChild(labelRow);

    const dots = document.createElement("ul");
    dots.className = "lyrics-picker-content-dots";
    PROVIDERS.forEach((name, index) => {
      const dot = document.createElement("li");
      dot.className = `lyrics-picker-dot${name === currentProvider ? " active" : ""}`;
      dot.addEventListener("click", () => selectProvider(index));
      dots.appendChild(dot);
    });
    center.appendChild(dots);
    return center;
  }

  function renderLoadingKaomoji(target) {
    const box = document.createElement("div");
    box.className = "kaomoji-container";
    box.textContent = "{ (>_<) }";
    target.appendChild(box);
  }

  function renderNotFoundKaomoji(target) {
    const box = document.createElement("div");
    box.className = "kaomoji-container";
    box.textContent = "＼(〇_ｏ)／";
    target.appendChild(box);
  }

  function renderErrorDisplay(target, message, onRetry) {
    const wrap = document.createElement("div");
    wrap.className = "error-container";
    const box = document.createElement("div");
    box.className = "error-box";
    box.textContent = message;
    wrap.appendChild(box);
    if (onRetry) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "retry-btn";
      btn.textContent = "Retry";
      btn.addEventListener("click", onRetry);
      wrap.appendChild(btn);
    }
    target.appendChild(wrap);
  }

  function retryCurrentProvider(version) {
    if (!activeTrack) return;
    const entry = CACHE.get(activeTrack.videoId);
    if (!entry) return;
    fetchProvider(currentProvider, activeTrack, entry).then(() => {
      if (version === renderVersion) render();
    });
  }

  function detectTargetLanguage() {
    const ytLang = (() => {
      try {
        return window?.ytcfg?.get?.("HL") || window?.ytcfg?.data_?.HL || "";
      } catch {
        return "";
      }
    })();
    const candidates = [
      ytLang,
      document?.documentElement?.lang,
      Array.isArray(navigator?.languages) ? navigator.languages[0] : "",
      navigator?.language,
    ];
    for (const raw of candidates) {
      const tag = String(raw || "").trim().toLowerCase();
      if (!tag) continue;
      if (tag.startsWith("zh")) {
        return tag.includes("tw") || tag.includes("hant") ? "zh-TW" : "zh-CN";
      }
      const base = tag.split(/[-_]/)[0];
      if (/^[a-z]{2,3}$/.test(base)) return base;
    }
    return "ru";
  }

  function needsTranslation(lineText, targetLang) {
    const s = String(lineText || "").trim();
    if (!s || s === "♪" || s === "♪ Instrumental ♪" || s === "..." || s === "•••") return false;
    if (!/[A-Za-z\u00C0-\u024F\u0400-\u04FF\u3040-\u30ff\u3400-\u9fff\uac00-\ud7af\u0600-\u06ff]/.test(s)) {
      return false;
    }
    const hasLatin = /[A-Za-z\u00C0-\u024F]/.test(s);
    const hasCjkOrHangul = /[\u3040-\u30ff\u3400-\u9fff\uac00-\ud7af]/.test(s);
    const hasCyrillic = /[\u0400-\u04FF]/.test(s);
    if (targetLang === "ru") {
      const hasUkChars = /[\u0404\u0406\u0407\u0490\u0454\u0456\u0457\u0491]/.test(s);
      if (hasCyrillic && !hasLatin && !hasCjkOrHangul && !hasUkChars) return false;
    } else if (targetLang === "uk") {
      const hasRuChars = /[\u042B\u044B\u042D\u044D\u0401\u0451\u042A\u044A]/.test(s);
      if (hasCyrillic && !hasLatin && !hasCjkOrHangul && !hasRuChars) return false;
    } else if (targetLang === "en") {
      if (hasLatin && !hasCyrillic && !hasCjkOrHangul && !/[\u00C0-\u024F]/.test(s)) return false;
    }
    return true;
  }

  function appendTranslationNodes(lineElements, texts, translations) {
    for (let i = 0; i < lineElements.length; i++) {
      const tr = translations[i];
      if (!tr) continue;
      const textEl = lineElements[i];
      if (!textEl || textEl.querySelector?.(".lyrics-translation")) continue;
      const origNorm = clean(texts[i]).toLowerCase();
      const trNorm = clean(tr).toLowerCase();
      if (!trNorm || trNorm === origNorm) continue;
      const span = document.createElement("span");
      span.className = "lyrics-translation";
      span.textContent = tr;
      textEl.appendChild(span);
    }
  }

  async function translateRenderedLines(lineElements, texts, version = renderVersion, epoch = currentTrackEpoch, onDone = null) {
    if (config().lyrics_translation === false || typeof runtime.translate !== "function") {
      return;
    }
    const targetLang = detectTargetLanguage();
    const uniqueLines = [];
    const uniqueToIndices = new Map();
    for (let i = 0; i < texts.length; i++) {
      const normalized = clean(texts[i]);
      if (!needsTranslation(normalized, targetLang)) continue;
      let list = uniqueToIndices.get(normalized);
      if (!list) {
        list = [];
        uniqueToIndices.set(normalized, list);
        uniqueLines.push(normalized);
      }
      list.push(i);
    }
    if (!uniqueLines.length) return;

    const cacheKey = `${targetLang}|${activeTrack?.videoId || ""}|${uniqueLines.length}|${uniqueLines[0]}`;
    const cached = TRANSLATION_CACHE.get(cacheKey);
    if (cached && cached.some(Boolean)) {
      if (version !== renderVersion || epoch !== currentTrackEpoch) return;
      appendTranslationNodes(lineElements, texts, cached);
      if (typeof onDone === "function") onDone();
      return;
    }

    // Batch unique lines under WebView2/Chromium document.title 4KB limit (~1400 JSON chars / 16 lines max per bridge call)
    const chunks = [];
    let currentChunk = [];
    let currentLen = 0;
    for (const line of uniqueLines) {
      const lineCost = line.length + 4;
      if (currentChunk.length >= 16 || (currentChunk.length > 0 && currentLen + lineCost > 1400)) {
        chunks.push(currentChunk);
        currentChunk = [];
        currentLen = 0;
      }
      currentChunk.push(line);
      currentLen += lineCost;
    }
    if (currentChunk.length) chunks.push(currentChunk);

    const mapped = new Array(texts.length).fill(null);
    let anyTranslated = false;

    try {
      for (const chunk of chunks) {
        if (version !== renderVersion || epoch !== currentTrackEpoch) return;
        const res = await runtime.translate(targetLang, chunk);
        if (version !== renderVersion || epoch !== currentTrackEpoch || !res?.body) return;
        const parsed = JSON.parse(res.body);
        const list = Array.isArray(parsed?.translations) ? parsed.translations : [];
        const chunkMapped = new Array(texts.length).fill(null);
        let chunkHasAny = false;
        for (let k = 0; k < chunk.length; k++) {
          const val = typeof list[k] === "string" && list[k].trim() ? list[k].trim() : null;
          if (!val) continue;
          const indices = uniqueToIndices.get(chunk[k]) || [];
          for (const idx of indices) {
            mapped[idx] = val;
            chunkMapped[idx] = val;
          }
          chunkHasAny = true;
          anyTranslated = true;
        }
        if (chunkHasAny) {
          appendTranslationNodes(lineElements, texts, chunkMapped);
          if (typeof onDone === "function") onDone();
        }
      }
    } catch {
      return;
    }

    if (anyTranslated) {
      if (TRANSLATION_CACHE.size >= CACHE_LIMIT) {
        const oldest = TRANSLATION_CACHE.keys().next().value;
        if (oldest) TRANSLATION_CACHE.delete(oldest);
      }
      TRANSLATION_CACHE.set(cacheKey, mapped);
    }
  }

  function renderPlain(target, text, version = renderVersion) {
    target?.classList?.add?.("is-plain");
    const isTag = /^\[(?:ti|ar|al|au|by|re|ve|length|offset):/i;
    const timestampPrefix = /^\[\d+:\d+(?:[.,:]\d+)?\]\s*/;
    const lines = String(text || "")
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l && !isTag.test(l))
      .map((l) => l.replace(timestampPrefix, "").trim())
      .filter(Boolean);
    const plainTextEls = [];
    lines.forEach((line) => {
      const wrap = document.createElement("div");
      wrap.className = "synced-line";
      const lineText = document.createElement("div");
      lineText.className = "text-lyrics";
      const inner = document.createElement("span");
      const span = document.createElement("span");
      span.textContent = line;
      inner.appendChild(span);
      lineText.appendChild(inner);
      wrap.appendChild(lineText);
      target.appendChild(wrap);
      plainTextEls.push(lineText);
    });
    translateRenderedLines(plainTextEls, lines, version, currentTrackEpoch);
  }

  function renderSynced(target, rawLines, version = renderVersion) {
    target?.classList?.remove?.("is-plain");
    currentLineElements = [];
    const lines = insertInstrumentalBreaks(rawLines);
    const lineTextElements = [];
    const lineRawTexts = [];
    lines.forEach((line, index) => {
      const lineEl = document.createElement("div");
      currentLineElements.push(lineEl);
      const rawText = clean(line.text);
      const isInstrumental = Boolean(line.isInstrumental || !rawText || rawText === "♪" || rawText === "..." || rawText === "•••");
      lineEl.className = `synced-line${isInstrumental ? " instrumental" : ""}`;
      lineEl.dataset.index = String(index);

      if (!isInstrumental) {
        const seekHint = document.createElement("span");
        seekHint.className = "seek-hint-icon";
        seekHint.innerHTML = `<svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>`;
        lineEl.appendChild(seekHint);
      }

      const text = document.createElement("div");
      text.className = "text-lyrics";
      text.style.setProperty("--lyrics-duration", `${Math.max(line.duration, 1000) / 1000}s`, "important");
      let downX = 0, downY = 0, hasDown = false, isDragging = false;
      text.addEventListener("mousedown", (e) => {
        if (e.button !== 0) return;
        downX = e.clientX;
        downY = e.clientY;
        hasDown = true;
        isDragging = false;
      });
      text.addEventListener("mousemove", (e) => {
        if (hasDown && (Math.abs(e.clientX - downX) > 5 || Math.abs(e.clientY - downY) > 5)) {
          isDragging = true;
        }
      });
      text.addEventListener("click", () => {
        const wasDragging = isDragging;
        hasDown = false;
        isDragging = false;
        const sel = window.getSelection()?.toString();
        if (wasDragging || (sel && sel.trim().length > 0)) {
          return;
        }
        seekToLine(line.timeInMs, lineEl);
      });

      if (config().lyrics_show_timecodes && line.time) {
        const time = document.createElement("span");
        time.className = "timecode";
        time.textContent = `[${line.time}]`;
        text.appendChild(time);
      }

      const wordsWrap = document.createElement("span");
      const lineSpan = document.createElement("span");
      lineSpan.textContent = isInstrumental ? "♪ Instrumental ♪" : (line.text || "♪");
      wordsWrap.appendChild(lineSpan);
      text.appendChild(wordsWrap);

      const isNonLatin = /[\u3040-\u30ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uff66-\uff9f\u1100-\u11ff\u3130-\u318f\ua960-\ua97f\ud7b0-\ud7ff\uac00-\ud7af]/.test(line.text);
      if (config().lyrics_romanization && line.text && isNonLatin && !isInstrumental) {
        const romanized = romanizeText(line.text);
        if (romanized) {
          const romaji = document.createElement("span");
          romaji.className = "romaji";
          romaji.textContent = romanized;
          text.appendChild(romaji);
        }
      }

      lineTextElements.push(text);
      lineRawTexts.push(isInstrumental ? "" : (line.text || ""));
      lineEl.appendChild(text);
      target.appendChild(lineEl);
    });

    const measureOffsets = () => {
      cachedContainerHeight = target.clientHeight;
      if (!currentLineElements.length) return;
      const offsets = new Float64Array(currentLineElements.length);
      for (let i = 0; i < currentLineElements.length; i++) {
        offsets[i] = currentLineElements[i].offsetTop;
      }
      currentLineOffsets = offsets;
    };

    if (typeof window.requestAnimationFrame === "function") {
      window.requestAnimationFrame(measureOffsets);
    } else {
      measureOffsets();
    }

    if (typeof ResizeObserver === "function") {
      resizeObserver = new ResizeObserver(() => {
        measureOffsets();
      });
      resizeObserver.observe(target);
    }

    updateCurrentLine();

    translateRenderedLines(lineTextElements, lineRawTexts, version, currentTrackEpoch, () => {
      if (typeof window.requestAnimationFrame === "function") {
        window.requestAnimationFrame(() => {
          measureOffsets();
          scrollToActiveLine();
        });
      } else {
        measureOffsets();
        scrollToActiveLine();
      }
    });
  }

  function seekToLine(timeInMs, lineEl) {
    if (lineEl) {
      lineEl.classList.remove("line-seek-pulse");
      void lineEl.offsetWidth;
      lineEl.classList.add("line-seek-pulse");
      setTimeout(() => lineEl?.classList.remove("line-seek-pulse"), 450);
    }
    isUserScrolling = false;
    if (userScrollTimeout) {
      window.clearTimeout(userScrollTimeout);
      userScrollTimeout = null;
    }
    updateSyncButtonVisibility();
    const timeSec = Math.max(0, (timeInMs + 10) / 1000);
    const player = getPlayer();
    if (typeof player?.seekTo === "function") {
      try { player.seekTo(timeSec, true); } catch {}
    }
    const media = runtime.media() || document.querySelector("video, audio");
    if (media) {
      try { media.currentTime = timeSec; } catch {}
    }
    lastCurrentIndex = -1;
    updateCurrentLine();
  }

  function updateCurrentLine() {
    if (typeof document !== "undefined" && document.hidden) return;
    const container = document.getElementById(CONTAINER_ID);
    if (!container) return;
    const current = state()?.[currentProvider]?.data?.lines;
    if (!current?.length || !currentLineElements.length) return;

    const player = getPlayer();
    const currentTimeSec = typeof player?.getCurrentTime === "function"
      ? player.getCurrentTime()
      : (runtime.media()?.currentTime || 0);
    if (lastCurrentIndex >= 0 && Math.abs(currentTimeSec - (updateCurrentLine._lastSec ?? -1)) < 0.02) {
      return;
    }
    updateCurrentLine._lastSec = currentTimeSec;
    const currentTimeMs = currentTimeSec * 1000;
    const currentIndex = current.findLastIndex((line) => line.timeInMs <= currentTimeMs);

    if (currentIndex === lastCurrentIndex) return;
    const prevIndex = lastCurrentIndex;
    lastCurrentIndex = currentIndex;

    const applyLineUpdate = () => {
      scrollRafId = 0;
      const clearNeighbor = (idx) => {
        if (idx >= 0 && currentLineElements[idx]) {
          currentLineElements[idx].classList.remove("current", "near-current");
        }
      };
      if (prevIndex >= 0) {
        clearNeighbor(prevIndex - 1);
        clearNeighbor(prevIndex);
        clearNeighbor(prevIndex + 1);
      } else {
        for (let i = 0; i < currentLineElements.length; i++) {
          if (i !== currentIndex) {
            currentLineElements[i].classList.remove("current", "near-current");
          }
        }
      }
      if (currentIndex >= 0 && currentLineElements[currentIndex]) {
        currentLineElements[currentIndex].classList.remove("near-current");
        currentLineElements[currentIndex].classList.add("current");
        if (currentLineElements[currentIndex - 1]) currentLineElements[currentIndex - 1].classList.add("near-current");
        if (currentLineElements[currentIndex + 1]) currentLineElements[currentIndex + 1].classList.add("near-current");
      }
      if (currentIndex >= 0 && currentLineElements[currentIndex] && !isUserScrolling && !isPointerSelecting && !hasActiveLyricsSelection()) {
        scrollToLineIndex(currentIndex);
      }
    };

    if (typeof window.requestAnimationFrame === "function") {
      if (scrollRafId) window.cancelAnimationFrame(scrollRafId);
      scrollRafId = window.requestAnimationFrame(applyLineUpdate);
    } else {
      applyLineUpdate();
    }
  }

  let trackEventListener = null;
  let navigateListener = null;

  const playerVideoDataListener = () => refreshTrack();
  const playerEmptiedListener = () => invalidateActiveTrack();
  const mediaMetadataListener = () => refreshTrack();
  const mediaEmptiedListener = () => invalidateActiveTrack();

  function bindPlayerListeners() {
    const player = getPlayer();
    if (player === boundPlayer) return;
    if (boundPlayer) {
      boundPlayer.removeEventListener("videodatachange", playerVideoDataListener);
      boundPlayer.removeEventListener("emptied", playerEmptiedListener);
    }
    boundPlayer = player || null;
    if (boundPlayer) {
      boundPlayer.addEventListener("videodatachange", playerVideoDataListener);
      boundPlayer.addEventListener("emptied", playerEmptiedListener);
    }
  }

  function bindMediaListeners() {
    const media = runtime.media();
    if (media === boundMedia) return;
    if (boundMedia) {
      boundMedia.removeEventListener("loadedmetadata", mediaMetadataListener);
      boundMedia.removeEventListener("durationchange", mediaMetadataListener);
      boundMedia.removeEventListener("emptied", mediaEmptiedListener);
    }
    boundMedia = media || null;
    if (boundMedia) {
      boundMedia.addEventListener("loadedmetadata", mediaMetadataListener);
      boundMedia.addEventListener("durationchange", mediaMetadataListener);
      boundMedia.addEventListener("emptied", mediaEmptiedListener);
    }
  }

  function restartUpdateInterval() {
    window.clearInterval(updateInterval);
    updateInterval = window.setInterval(
      updateCurrentLine,
      config().lyrics_precise_timing ? 100 : 250,
    );
  }

  function invalidateActiveTrack() {
    if (currentAbortController) {
      currentAbortController.abort();
      currentAbortController = null;
    }
    currentTrackEpoch++;
    isUserScrolling = false;
    isInitialTrackScroll = true;
    if (userScrollTimeout) window.clearTimeout(userScrollTimeout);
    activeTrack = null;
    manuallySwitched = false;
    lastCurrentIndex = -1;
    render();
  }

  function refreshTrack(force = false, override = null) {
    const info = trackInfo(override);
    if (!info) {
      if (activeTrack) {
        invalidateActiveTrack();
      }
      return;
    }

    const isDifferent =
      activeTrack?.videoId !== info.videoId ||
      activeTrack?.title !== info.title ||
      activeTrack?.artist !== info.artist;
    const prevDuration = activeTrack?.songDuration || 0;
    const hasSyncedAlready = Boolean(CACHE.get(info.videoId)?.providers?.Unified?.data?.lines?.length);
    const durationChanged = Boolean(
      activeTrack &&
      info.songDuration > 0 &&
      !hasSyncedAlready &&
      (prevDuration <= 0 || Math.abs(prevDuration - info.songDuration) > 6)
    );

    if (isDifferent || durationChanged || force) {
      if (currentAbortController) {
        currentAbortController.abort();
        currentAbortController = null;
      }
      currentTrackEpoch++;
      const epoch = currentTrackEpoch;
      currentAbortController = typeof AbortController === "function" ? new AbortController() : null;
      const signal = currentAbortController?.signal || null;
      isUserScrolling = false;
      isInitialTrackScroll = true;
      if (userScrollTimeout) window.clearTimeout(userScrollTimeout);
      activeTrack = info;
      manuallySwitched = false;
      lastCurrentIndex = -1;
      ensureTrack(info, epoch, signal);
      render();
    }
  }

  function start() {
    if (running) return;
    running = true;
    installStyle();
    setupHeaderObserver();
    refreshTrack();

    bindPlayerListeners();

    trackEventListener = (event) => refreshTrack(true, event?.detail);
    window.addEventListener("ytm-track-change", trackEventListener);

    pointerEndListener = () => { isPointerSelecting = false; };
    window.addEventListener("mouseup", pointerEndListener, { passive: true });
    window.addEventListener("blur", pointerEndListener, { passive: true });

    bindMediaListeners();

    navigateListener = () => {
      setupHeaderObserver();
      refreshTrack();
      ensureContainer();
    };
    document.addEventListener("yt-navigate-finish", navigateListener);

    restartUpdateInterval();
    trackPollInterval = window.setInterval(() => {
      if (typeof document !== "undefined" && document.hidden) return;
      bindPlayerListeners();
      bindMediaListeners();
      setupHeaderObserver();
      refreshTrack();
      ensureContainer();
    }, 2500);
    if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
      document.addEventListener("visibilitychange", () => {
        if (!document.hidden && running) {
          refreshTrack();
          updateCurrentLine();
        }
      });
    }

    render();
  }

  function stop() {
    running = false;
    if (currentAbortController) {
      currentAbortController.abort();
      currentAbortController = null;
    }
    if (resizeObserver) {
      resizeObserver.disconnect();
      resizeObserver = null;
    }
    if (scrollRafId) {
      window.cancelAnimationFrame(scrollRafId);
      scrollRafId = 0;
    }
    currentLineElements = [];
    currentLineOffsets = null;
    cachedContainerHeight = 0;
    currentTrackEpoch++;
    activeTrack = null;
    if (headerObserver) headerObserver.disconnect();
    headerObserver = null;
    if (tabsBarObserver) tabsBarObserver.disconnect();
    tabsBarObserver = null;
    if (observedHeader) observedHeader.removeEventListener("click", onLyricsHeaderClick);
    observedHeader = null;
    if (observedTabsBar) observedTabsBar.removeEventListener("click", onLyricsHeaderClick);
    observedTabsBar = null;
    if (trackEventListener) {
      window.removeEventListener("ytm-track-change", trackEventListener);
      trackEventListener = null;
    }
    if (pointerEndListener) {
      window.removeEventListener("mouseup", pointerEndListener);
      window.removeEventListener("blur", pointerEndListener);
      pointerEndListener = null;
    }
    isPointerSelecting = false;
    if (navigateListener) {
      document.removeEventListener("yt-navigate-finish", navigateListener);
      navigateListener = null;
    }
    if (boundPlayer) {
      boundPlayer.removeEventListener("videodatachange", playerVideoDataListener);
      boundPlayer.removeEventListener("emptied", playerEmptiedListener);
      boundPlayer = null;
    }
    if (boundMedia) {
      boundMedia.removeEventListener("loadedmetadata", mediaMetadataListener);
      boundMedia.removeEventListener("durationchange", mediaMetadataListener);
      boundMedia.removeEventListener("emptied", mediaEmptiedListener);
      boundMedia = null;
    }
    window.clearInterval(updateInterval);
    window.clearInterval(trackPollInterval);
    updateInterval = 0;
    trackPollInterval = 0;
    const container = document.getElementById(CONTAINER_ID);
    if (container) container.remove();
  }

  function update() {
    setupHeaderObserver();
    restartUpdateInterval();
    render();
  }

  runtime.register("synced_lyrics", { start, stop, update, applyEffect, fetchLrcLib, renderPlain });
})();
