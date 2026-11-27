(() => {
  const runtime = window.__ytmFeatures;
  if (!runtime) return;

  const STYLE_ID = "ytm-tauri-blur-nav-bar-style";
  const css = `
    ytmusic-tabs.stuck,
    #search-page #tabs.stuck {
      background: rgba(18, 18, 18, 0.78) !important;
      backdrop-filter: blur(8px) !important;
      -webkit-backdrop-filter: blur(8px) !important;
    }

    ytmusic-menu-popup-renderer {
      background: rgba(24, 24, 24, 0.88) !important;
      backdrop-filter: blur(8px) !important;
      -webkit-backdrop-filter: blur(8px) !important;
      border-radius: 12px !important;
      border: 1px solid rgba(255, 255, 255, 0.1) !important;
    }

    #nav-bar-divider {
      display: none !important;
    }
  `;

  function start() {
    let style = document.getElementById(STYLE_ID);
    if (!style) {
      style = document.createElement("style");
      style.id = STYLE_ID;
      style.textContent = css;
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
  }

  function stop() {
    document.getElementById(STYLE_ID)?.remove();
  }

  runtime.register("blur_nav_bar", { start, stop });
})();
