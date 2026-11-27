(() => {
  const runtime = window.__ytmFeatures;
  if (!runtime) return;

  const STYLE_ID = "ytm-tauri-ambient-mode-style";
  const BLUR_CLASS = "html5-blur-image";

  const css = `
    #song-video canvas.html5-blur-canvas,
    #song-image .html5-blur-image {
      opacity: 0.72;
      width: 100%;
      height: 100%;
      pointer-events: none;
      position: absolute !important;
      left: 50% !important;
      top: 50% !important;
      transform: translate(-50%, -50%) scale(1.36) !important;
      border-radius: 50% !important;
      -webkit-mask-image: radial-gradient(ellipse 66% 66% at 50% 50%, rgba(0, 0, 0, 1) 30%, rgba(0, 0, 0, 0.42) 62%, rgba(0, 0, 0, 0) 96%) !important;
      mask-image: radial-gradient(ellipse 66% 66% at 50% 50%, rgba(0, 0, 0, 1) 30%, rgba(0, 0, 0, 0.42) 62%, rgba(0, 0, 0, 0) 96%) !important;
    }
    #song-image img.${BLUR_CLASS} {
      width: 24%;
      height: 24%;
      filter: blur(10px);
      transform: translate(-50%, -50%) scale(3.4) !important;
    }

    #song-image {
      position: relative !important;
    }

    #player:not([video-mode]):not(.video-mode):not([player-ui-state='MINIPLAYER']):not([is-mweb-modernization-enabled]) {
      width: 100%;
      margin: 0 auto !important;
      overflow: visible !important;
    }

    .song-button.ytmusic-av-toggle,
    .video-button.ytmusic-av-toggle {
      z-index: 1;
      background-color: transparent;
    }

    #side-panel.side-panel.ytmusic-player-page {
      z-index: 0;
    }
  `;

  let observer = null;
  let observedImg = null;
  let interval = null;
  let lastImageSource = null;
  let blurElement = null;
  let ambientCanvas = null;

  function paintPreblurredCanvas(bitmap, songImage) {
    if (!songImage?.isConnected) return false;
    const canvas = ambientCanvas || document.createElement("canvas");
    canvas.width = 48;
    canvas.height = 48;
    const ctx = canvas.getContext("2d");
    if (!ctx) return false;
    ambientCanvas = canvas;
    ctx.clearRect(0, 0, 48, 48);
    if ("filter" in ctx) ctx.filter = "blur(6px)";
    ctx.drawImage(bitmap, 6, 6, 36, 36);
    if ("filter" in ctx) ctx.filter = "none";
    if (typeof ctx.createRadialGradient === "function") {
      ctx.globalCompositeOperation = "destination-in";
      const mask = ctx.createRadialGradient(24, 24, 9, 24, 24, 24);
      mask.addColorStop(0, "rgba(0, 0, 0, 1)");
      mask.addColorStop(0.62, "rgba(0, 0, 0, 0.48)");
      mask.addColorStop(1, "rgba(0, 0, 0, 0)");
      ctx.fillStyle = mask;
      ctx.fillRect(0, 0, 48, 48);
      ctx.globalCompositeOperation = "source-over";
    }
    canvas.className = BLUR_CLASS;
    if (blurElement && blurElement !== canvas) blurElement.remove();
    blurElement = canvas;
    if (canvas.parentNode !== songImage) {
      songImage.prepend(canvas);
    }
    return true;
  }

  function renderPreblurredCanvas(src, songImage) {
    if (typeof Image !== "function") return false;
    const probe = new Image();
    probe.crossOrigin = "anonymous";
    const clearHandlers = () => {
      probe.onload = null;
      probe.onerror = null;
    };
    probe.onload = () => {
      clearHandlers();
      if (src !== lastImageSource || !songImage?.isConnected) return;
      try {
        // Zero live CSS blur pass per frame: GPU stretches tiny 48x48 pre-blurred feathered texture for free
        paintPreblurredCanvas(probe, songImage);
      } catch {
        // fallback handled below
      }
    };
    probe.onerror = () => {
      clearHandlers();
      if (typeof fetch !== "function" || typeof URL?.createObjectURL !== "function") return;
      fetch(src, { mode: "cors" })
        .then((res) => (res.ok ? res.blob() : null))
        .then((blob) => {
          if (!blob || src !== lastImageSource || !songImage?.isConnected) return;
          const blobUrl = URL.createObjectURL(blob);
          const bImg = new Image();
          const done = () => {
            URL.revokeObjectURL(blobUrl);
            bImg.onload = null;
            bImg.onerror = null;
          };
          bImg.onload = () => {
            try {
              if (src === lastImageSource) paintPreblurredCanvas(bImg, songImage);
            } finally {
              done();
            }
          };
          bImg.onerror = done;
          bImg.src = blobUrl;
        })
        .catch(() => {});
    };
    probe.src = src;
    return true;
  }

  function syncAmbient() {
    if (document.hidden) return;
    const layout = document.querySelector("#layout, ytmusic-app-layout");
    const isPageOpen = layout?.hasAttribute("player-page-open");
    const songImage = document.querySelector("#song-image");
    const image = songImage?.querySelector("yt-img-shadow > img");

    if (!isPageOpen || !songImage || !image || !image.src || image.src.startsWith("data:")) {
      if (blurElement) {
        blurElement.remove();
        blurElement = null;
        lastImageSource = null;
      }
      return;
    }

    if (image !== observedImg && typeof MutationObserver === "function") {
      observer?.disconnect();
      observedImg = image;
      observer = new MutationObserver(syncAmbient);
      observer.observe(image, { attributes: true, attributeFilter: ["src"] });
    }

    if (blurElement && lastImageSource === image.src) return;

    lastImageSource = image.src;
    if (!blurElement || blurElement.tagName !== "IMG") {
      blurElement?.remove();
      blurElement = document.createElement("img");
      blurElement.className = BLUR_CLASS;
      songImage.prepend(blurElement);
    }
    blurElement.src = image.src;
    renderPreblurredCanvas(image.src, songImage);
  }

  function start() {
    let style = document.getElementById(STYLE_ID);
    if (!style) {
      style = document.createElement("style");
      style.id = STYLE_ID;
      style.textContent = css;
      const target = document.head || document.documentElement || document.body;
      if (target) target.appendChild(style);
    }

    syncAmbient();
    window.addEventListener("ytm-track-change", syncAmbient);
    if (interval) clearInterval(interval);
    interval = setInterval(syncAmbient, 2500);
  }

  function stop() {
    window.removeEventListener("ytm-track-change", syncAmbient);
    observer?.disconnect();
    observer = null;
    observedImg = null;
    if (interval) clearInterval(interval);
    interval = null;
    if (blurElement) {
      blurElement.remove();
      blurElement = null;
    }
    lastImageSource = null;
    document.getElementById(STYLE_ID)?.remove();
  }

  runtime.register("ambient_mode", { start, stop });
})();
