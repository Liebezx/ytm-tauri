(() => {
  if (location.hostname !== "music.youtube.com") {
    return;
  }
  if (window.__ytMusicTauriAdBlockInstalled) {
    return;
  }
  window.__ytMusicTauriAdBlockInstalled = true;

  const stripAdPayload = (payload) => {
    if (!payload || typeof payload !== "object") return payload;
    delete payload.adPlacements;
    delete payload.playerAds;
    delete payload.adSlots;
    delete payload.adBreakHeartbeatParams;
    return payload;
  };

  const installPlayerJsonFilter = () => {
    if (window.__ytMusicTauriAdJsonFilterInstalled) return;
    window.__ytMusicTauriAdJsonFilterInstalled = true;
    try {
      if (window.ytInitialPlayerResponse && window.__ytMusicTauriAdBlockEnabled !== false) {
        stripAdPayload(window.ytInitialPlayerResponse);
      }
    } catch {}

    if (typeof window.fetch === "function" && typeof Response === "function") {
      const nativeFetch = window.fetch.bind(window);
      window.fetch = function (input, init) {
        const result = nativeFetch(input, init);
        if (window.__ytMusicTauriAdBlockEnabled === false) return result;
        const url = typeof input === "string" ? input : input?.url || "";
        if (!/\/youtubei\/v1\/(player|next|get_watch)\b/.test(url)) {
          return result;
        }
        return result.then((response) => {
          if (!response || !response.ok) return response;
          const ct = response.headers?.get?.("content-type") || "";
          if (ct && !ct.includes("json")) return response;
          return response
            .clone()
            .text()
            .then((text) => {
              if (
                !text.includes("adPlacements") &&
                !text.includes("playerAds") &&
                !text.includes("adSlots") &&
                !text.includes("adBreakHeartbeatParams")
              ) {
                return response;
              }
              const parsed = stripAdPayload(JSON.parse(text));
              const headers = new Headers(response.headers);
              headers.delete("content-length");
              return new Response(JSON.stringify(parsed), {
                status: response.status,
                statusText: response.statusText,
                headers,
              });
            })
            .catch(() => response);
        });
      };
    }
  };
  installPlayerJsonFilter();

  const savedMediaState = new WeakMap();
  let hasSavedMedia = false;
  let adMuteActive = false;
  const style = document.createElement("style");
  style.textContent = `
    ytd-ad-slot-renderer,
    ytd-display-ad-renderer,
    ytd-in-feed-ad-layout-renderer,
    ytd-promoted-sparkles-web-renderer,
    ytmusic-guide-section-renderer:first-of-type ytmusic-guide-entry-renderer:nth-child(n+4),
    ytmusic-guide-entry-renderer:has(a[href*="/upgrade"]),
    ytmusic-guide-entry-renderer:has(a[href*="music_premium"]),
    ytmusic-guide-entry-renderer:has(tp-yt-paper-item[aria-label*="Upgrade"]),
    ytmusic-guide-entry-renderer:has([aria-label*="Upgrade"]),
    ytmusic-guide-entry-renderer[tab-identifier="FEmusic_premium"],
    ytmusic-mini-guide-entry-renderer:has(a[href*="/upgrade"]),
    ytmusic-mini-guide-entry-renderer:has(a[href*="music_premium"]),
    ytmusic-mini-guide-entry-renderer:has([aria-label*="Upgrade"]),
    ytmusic-mini-guide-entry-renderer[tab-identifier="FEmusic_premium"],
    ytmusic-mini-guide-renderer ytmusic-mini-guide-entry-renderer:nth-child(n+4),
    #mini-guide #items > *:nth-child(n+4),
    ytmusic-pivot-bar-item-renderer:has(a[href*="/upgrade"]),
    ytmusic-pivot-bar-item-renderer:has(a[href*="music_premium"]),
    ytmusic-compact-link-renderer:has(a[href*="/upgrade"]),
    ytmusic-compact-link-renderer:has(a[href*="music_premium"]),
    #guide ytmusic-guide-entry-renderer:has(a[href*="upgrade"]),
    #mini-guide ytmusic-mini-guide-entry-renderer:has(a[href*="upgrade"]),
    #mini-guide ytmusic-guide-entry-renderer:has(a[href*="upgrade"]),
    .html5-video-player.ad-showing .ytp-ad-module,
    .html5-video-player.ad-interrupting .ytp-ad-module {
      display: none !important;
    }
    ytmusic-mealbar-promo-renderer {
      opacity: 0 !important;
      pointer-events: none !important;
    }
  `;

  const injectStyle = () => {
    if (window.__ytMusicTauriAdBlockEnabled === false) {
      return;
    }
    const target = document.head || document.documentElement;
    if (target) {
      target.appendChild(style);
    } else {
      window.setTimeout(injectStyle, 50);
    }
  };
  injectStyle();

  const isMusicHost = () => location.hostname === "music.youtube.com";

  const isAdShowing = () => {
    const player = document.querySelector(".html5-video-player");
    return Boolean(
      player &&
        player.classList &&
        (player.classList.contains("ad-showing") ||
          player.classList.contains("ad-interrupting")),
    );
  };

  let popupObserver = null;
  let mealbarTimer = 0;
  const scheduleMealbar = () => {
    if (mealbarTimer || window.__ytMusicTauriAdBlockEnabled === false) return;
    mealbarTimer = window.setTimeout(() => {
      mealbarTimer = 0;
      dismissMealbar();
    }, 80);
  };
  const dismissMealbar = () => {
    if (window.__ytMusicTauriAdBlockEnabled === false) {
      return;
    }
    const mealbar = document.querySelector("ytmusic-mealbar-promo-renderer");
    if (!mealbar) {
      return;
    }
    if (typeof mealbar.dismiss === "function") {
      mealbar.dismiss();
      return;
    }
    const dismissBtn = mealbar.querySelector(
      "#dismiss-button, [aria-label*='Dismiss' i], yt-button-renderer:last-child button"
    );
    if (dismissBtn) {
      dismissBtn.click();
    }
  };

  const attachPopupObserver = () => {
    if (popupObserver) {
      return;
    }
    const container = document.querySelector("ytmusic-popup-container");
    if (!container) {
      return;
    }
    popupObserver = new MutationObserver(scheduleMealbar);
    popupObserver.observe(container, { childList: true, subtree: true });
  };

  const setAudioEngineAdMute = (muted) => {
    try {
      const engine = window.__ytmFeatures?.audioEngine;
      if (typeof engine?.setFaderGain === "function") {
        engine.setFaderGain(muted ? 0 : 1, 0);
      }
    } catch {}
  };

  const restoreMedia = () => {
    if (!hasSavedMedia) {
      if (adMuteActive) {
        adMuteActive = false;
        setAudioEngineAdMute(false);
      }
      return;
    }
    adMuteActive = false;
    setAudioEngineAdMute(false);
    for (const media of document.querySelectorAll("video, audio")) {
      const saved = savedMediaState.get(media);
      if (!saved) {
        continue;
      }
      media.muted = saved.muted;
      if (typeof saved.volume === "number") {
        try {
          media.volume = saved.volume;
        } catch {}
      }
      media.playbackRate = saved.playbackRate;
      savedMediaState.delete(media);
    }
    hasSavedMedia = false;
  };

  const silenceMediaForAd = (media) => {
    if (!media) return;
    if (!savedMediaState.has(media)) {
      savedMediaState.set(media, {
        muted: Boolean(media.muted),
        volume: typeof media.volume === "number" ? media.volume : 1,
        playbackRate: typeof media.playbackRate === "number" ? media.playbackRate : 1,
      });
      hasSavedMedia = true;
    }
    media.muted = true;
    if (typeof media.volume === "number") {
      try {
        media.volume = 0;
      } catch {}
    }
  };

  const skipAd = () => {
    adMuteActive = true;
    setAudioEngineAdMute(true);

    const moviePlayer =
      document.getElementById?.("movie_player") || document.querySelector(".html5-video-player");
    if (moviePlayer && typeof moviePlayer.skipAd === "function") {
      try { moviePlayer.skipAd(); } catch {}
    }

    const skipButton = document.querySelector(
      ".ytp-ad-skip-button-modern, .ytp-ad-skip-button, .ytp-ad-skip-button-container button, .ytp-ad-skip-button-slot button, button.ytp-ad-skip-button-modern, .ytp-ad-overlay-close-button",
    );
    if (skipButton && !skipButton.disabled) {
      try { skipButton.click(); } catch {}
    }

    for (const media of document.querySelectorAll("video, audio")) {
      silenceMediaForAd(media);
      media.playbackRate = 16;
      if (Number.isFinite(media.duration) && media.duration > 0) {
        try {
          media.currentTime = media.duration;
        } catch {
          try { media.currentTime = media.duration - 0.05; } catch {}
        }
      }
    }
  };

  let playerClassObserver = null;
  let observedPlayer = null;
  const attachPlayerObserver = () => {
    const player = document.querySelector(".html5-video-player");
    if (!player || player === observedPlayer) return;
    if (playerClassObserver) {
      playerClassObserver.disconnect?.();
    }
    observedPlayer = player;
    playerClassObserver = new MutationObserver(() => {
      if (window.__ytMusicTauriAdBlockEnabled === false || !isMusicHost()) return;
      if (isAdShowing()) {
        skipAd();
      } else if (hasSavedMedia || adMuteActive) {
        restoreMedia();
      }
    });
    playerClassObserver.observe(player, { attributes: true, attributeFilter: ["class"] });
  };

  const run = () => {
    if (window.__ytMusicTauriAdBlockEnabled === false) {
      restoreMedia();
      return;
    }
    attachPlayerObserver();
    if (!document.hidden) {
      attachPopupObserver();
      dismissMealbar();
    }

    if (
      !isMusicHost() ||
      !isAdShowing()
    ) {
      restoreMedia();
      return;
    }

    skipAd();
  };

  const install = () => {
    if (!document.documentElement) {
      window.setTimeout(install, 50);
      return;
    }

    if (typeof HTMLMediaElement === "function" && HTMLMediaElement.prototype?.play) {
      const nativePlay = HTMLMediaElement.prototype.play;
      if (!nativePlay.__ytmAdPatched) {
        const guardedPlay = function (...args) {
          if (window.__ytMusicTauriAdBlockEnabled !== false && isMusicHost() && isAdShowing()) {
            silenceMediaForAd(this);
            skipAd();
          }
          return nativePlay.apply(this, args);
        };
        guardedPlay.__ytmAdPatched = true;
        HTMLMediaElement.prototype.play = guardedPlay;
      }
    }

    // coalesce DOM insertion bursts cleanly; player attribute observer handles ads with 0ms delay
    let scheduled = false;
    const scheduleRun = () => {
      if (window.__ytMusicTauriAdBlockEnabled !== false && isMusicHost() && isAdShowing()) {
        skipAd();
      }
      if (scheduled) return;
      scheduled = true;
      window.setTimeout(() => {
        scheduled = false;
        run();
      }, 150);
    };
    const observer = new MutationObserver(scheduleRun);
    observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
    });

    const onMediaEvent = (e) => {
      if (window.__ytMusicTauriAdBlockEnabled === false || !isMusicHost()) return;
      if (e.target && (e.target.tagName === "VIDEO" || e.target.tagName === "AUDIO")) {
        if (isAdShowing()) {
          silenceMediaForAd(e.target);
          skipAd();
        } else if (hasSavedMedia || adMuteActive) {
          restoreMedia();
        }
      }
    };
    for (const evt of ["loadstart", "loadedmetadata", "durationchange", "play", "playing", "volumechange", "timeupdate"]) {
      document.addEventListener(evt, onMediaEvent, true);
    }

    document.addEventListener("yt-navigate-finish", run, true);
    window.addEventListener("pageshow", run, true);
    window.setInterval(run, 1200);
    run();
  };

  install();
})();
