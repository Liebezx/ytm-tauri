import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const source = await readFile(new URL("../src-tauri/src/feature_probe.js", import.meta.url), "utf8");

function createRuntime(initialTitle = "YouTube Music") {
  let timerId = 0;
  const timers = [];
  const context = {
    clearTimeout() {},
    console,
    Date,
    document: {
      addEventListener() {},
      documentElement: {},
      querySelector() {
        return null;
      },
      createElement() {
        return { id: "", textContent: "", appendChild() {} };
      },
      getElementById() {
        return null;
      },
      readyState: "complete",
      title: initialTitle,
    },
    setTimeout(callback, delay = 0) {
      timers.push({ callback, delay });
      return ++timerId;
    },
    addEventListener() {},
  };
  context.window = context;
  vm.runInNewContext(source, context);
  context.runImmediateTimers = () => {
    const ready = timers.splice(0).filter((timer) => timer.delay <= 10);
    for (const timer of ready) timer.callback();
  };
  return context;
}

test("feature requests release the document title after a native response", async () => {
  const context = createRuntime();
  const pending = context.window.__ytmFeatures.request("https://lrclib.net/api/search");
  const request = JSON.parse(context.document.title.slice("YTMFEATURE:".length));

  context.window.__ytmFeatures.receive(request.id, { ok: true });

  await pending;
  assert.equal(context.document.title, "YouTube Music");
});

test("feature responses do not overwrite a newer track title", async () => {
  const context = createRuntime();
  const pending = context.window.__ytmFeatures.request("https://lrclib.net/api/search");
  const request = JSON.parse(context.document.title.slice("YTMFEATURE:".length));
  const trackTitle = 'YTMRPC:{"type":"track"}';
  context.document.title = trackTitle;

  context.window.__ytmFeatures.receive(request.id, { ok: true });

  await pending;
  assert.equal(context.document.title, trackTitle);
});

test("setSetting emits feature request and updates config", async () => {
  const context = createRuntime();
  const pending = context.window.__ytmFeatures.setSetting("synced_lyrics", false);
  const request = JSON.parse(context.document.title.slice("YTMFEATURE:".length));

  assert.equal(request.kind, "set_setting");
  assert.equal(request.key, "synced_lyrics");
  assert.equal(request.value, false);
  assert.equal(context.window.__ytmFeatures.config.synced_lyrics, false);

  context.window.__ytmFeatures.receive(request.id, { ok: true });
  await pending;
});

test("triggerAction emits action request", async () => {
  const context = createRuntime();
  const pending = context.window.__ytmFeatures.triggerAction("check_updates");
  const request = JSON.parse(context.document.title.slice("YTMFEATURE:".length));

  assert.equal(request.kind, "action");
  assert.equal(request.action, "check_updates");

  context.window.__ytmFeatures.receive(request.id, { ok: true });
  await pending;
});

test("late feature responses cannot release the active request slot", async () => {
  const context = createRuntime();
  const first = context.window.__ytmFeatures.getSettings();
  const firstTitle = context.document.title;
  const firstRequest = JSON.parse(firstTitle.slice("YTMFEATURE:".length));
  const second = context.window.__ytmFeatures.triggerAction("check_updates");

  context.window.__ytmFeatures.receive(firstRequest.id + 100, { ok: true });
  context.runImmediateTimers();
  assert.equal(context.document.title, firstTitle);

  context.window.__ytmFeatures.receive(firstRequest.id, { ok: true });
  context.runImmediateTimers();
  const secondRequest = JSON.parse(context.document.title.slice("YTMFEATURE:".length));
  context.window.__ytmFeatures.receive(secondRequest.id, { ok: true });
  await Promise.all([first, second]);
});

test("player bar track metadata allows text selection without default I-beam cursor and suppresses click after drag selection", () => {
  let winHandlers = {};
  let docHandlers = {};
  const injectedStyles = [];
  let selectedText = "Milky";
  const trackNode = {
    closest(selector) {
      return selector.includes(".content-info-wrapper") || selector.includes("ytMusicMiniPlayerTrackInfo") ? {} : null;
    },
  };
  const context = {
    clearTimeout() {},
    console,
    Date,
    document: {
      addEventListener(event, handler) {
        docHandlers[event] = handler;
      },
      documentElement: {
        appendChild(node) {
          injectedStyles.push(node);
        },
      },
      head: null,
      querySelector() {
        return null;
      },
      createElement() {
        return { id: "", textContent: "", appendChild() {} };
      },
      getElementById() {
        return null;
      },
      readyState: "complete",
      title: "YouTube Music",
    },
    getSelection() {
      return selectedText;
    },
    setTimeout() {
      return 1;
    },
    addEventListener(event, handler) {
      winHandlers[event] = handler;
    },
  };
  context.window = context;
  vm.runInNewContext(source, context);

  let ctxPrevented = false;
  let ctxStopped = false;
  winHandlers.contextmenu({
    target: trackNode,
    preventDefault() { ctxPrevented = true; },
    stopPropagation() { ctxStopped = true; },
    stopImmediatePropagation() {},
  });
  assert.equal(ctxPrevented, false);
  assert.equal(ctxStopped, true);

  const css = injectedStyles.find((node) => node.id === "ytm-guard-style")?.textContent || "";
  assert.match(css, /ytmusic-player-bar \.content-info-wrapper[\s\S]*user-select:\s*text !important/);
  assert.match(css, /cursor:\s*default !important/);
  assert.doesNotMatch(css, /content-info-wrapper[\s\S]*cursor:\s*text/);

  let selectPrevented = false;
  docHandlers.selectstart({ target: trackNode, preventDefault() { selectPrevented = true; } });
  assert.equal(selectPrevented, false);

  docHandlers.mousedown({ button: 0, clientX: 10, clientY: 10, target: trackNode });
  docHandlers.mousemove({ clientX: 40, clientY: 10 });
  docHandlers.mouseup();

  let clickPrevented = false;
  let clickStopped = false;
  docHandlers.click({
    target: trackNode,
    preventDefault() { clickPrevented = true; },
    stopPropagation() { clickStopped = true; },
    stopImmediatePropagation() {},
  });
  assert.equal(clickPrevented, true);
  assert.equal(clickStopped, true);

  selectedText = "";
  let plainClickPrevented = false;
  docHandlers.mousedown({ button: 0, clientX: 10, clientY: 10, target: trackNode });
  docHandlers.mouseup();
  docHandlers.click({
    target: trackNode,
    preventDefault() { plainClickPrevented = true; },
    stopPropagation() {},
  });
  assert.equal(plainClickPrevented, false);
});

test("rejected setting writes restore the previous runtime value", async () => {
  const context = createRuntime();
  context.window.__ytmFeatures.configure({ synced_lyrics: true });
  const pending = context.window.__ytmFeatures.setSetting("synced_lyrics", false);
  const request = JSON.parse(context.document.title.slice("YTMFEATURE:".length));

  context.window.__ytmFeatures.receive(request.id, { error: "invalid setting value" });

  await assert.rejects(pending, /invalid setting value/);
  assert.equal(context.window.__ytmFeatures.config.synced_lyrics, true);
});

test("clearOrphanedInert removes inert attribute when no dialog is visible", () => {
  let inertRemoved = false;
  let pointerDownHandler = null;
  const layout = {
    removeAttribute(attr) {
      if (attr === "inert") inertRemoved = true;
    },
  };
  const context = {
    clearTimeout() {},
    console,
    Date,
    document: {
      addEventListener() {},
      documentElement: {},
      querySelector(selector) {
        if (selector === "ytmusic-app-layout[inert]") return layout;
        return null;
      },
      querySelectorAll() {
        return [];
      },
      createElement() {
        return { id: "", textContent: "", appendChild() {} };
      },
      getElementById() {
        return null;
      },
      readyState: "complete",
      title: "YouTube Music",
    },
    setTimeout() {
      return 1;
    },
    setInterval() {
      return 1;
    },
    addEventListener(event, handler) {
      if (event === "pointerdown") pointerDownHandler = handler;
    },
  };
  context.window = context;
  vm.runInNewContext(source, context);

  assert.equal(typeof pointerDownHandler, "function");
  pointerDownHandler();
  assert.equal(inertRemoved, true);
});


test("updater status is read without starting an update", async () => {
  const context = createRuntime();
  const pending = context.window.__ytmFeatures.getUpdateStatus();
  const request = JSON.parse(context.document.title.slice("YTMFEATURE:".length));
  assert.equal(request.kind, "get_update_status");
  assert.equal(request.action, undefined);
  context.window.__ytmFeatures.receive(request.id, { ok: true, body: JSON.stringify({ stage: "downloading", downloaded: 5, total: 10 }) });
  assert.equal(JSON.parse((await pending).body).stage, "downloading");
  assert.equal(context.document.title, "YouTube Music");
});

test("acknowledged slow requests free the bridge for queued settings", async () => {
  const context = createRuntime();
  const features = context.window.__ytmFeatures;
  const lyrics = features.lyrics({ title: "Song", artist: "Artist" });
  const lyricsRequest = JSON.parse(context.document.title.slice("YTMFEATURE:".length));
  assert.equal(lyricsRequest.kind, "lyrics");
  assert.equal(lyricsRequest.value.title, "Song");

  const setting = features.setSetting("synced_lyrics", true);
  features.ack(lyricsRequest.id);
  context.runImmediateTimers();
  const settingRequest = JSON.parse(context.document.title.slice("YTMFEATURE:".length));
  assert.equal(settingRequest.kind, "set_setting");

  features.receive(lyricsRequest.id, { ok: true, body: "{}" });
  assert.equal(context.document.title.startsWith("YTMFEATURE:"), true);
  features.receive(settingRequest.id, { ok: true });
  await Promise.all([lyrics, setting]);
  assert.equal(context.document.title, "YouTube Music");
});

test("translate emits translate request with target and lines", async () => {
  const context = createRuntime();
  const pending = context.window.__ytmFeatures.translate("ru", ["First", "Second"]);
  const request = JSON.parse(context.document.title.slice("YTMFEATURE:".length));
  assert.equal(request.kind, "translate");
  assert.equal(request.value.target, "ru");
  assert.deepEqual(request.value.lines, ["First", "Second"]);
  context.window.__ytmFeatures.receive(request.id, {
    ok: true,
    body: JSON.stringify({ translations: ["Первая", "Вторая"] }),
  });
  const res = await pending;
  assert.deepEqual(JSON.parse(res.body).translations, ["Первая", "Вторая"]);
});

test("lyrics requests preempt queued background translate requests", async () => {
  const context = createRuntime();
  const features = context.window.__ytmFeatures;
  const activeReq = features.request("https://lrclib.net/api/search");
  const req1 = JSON.parse(context.document.title.slice("YTMFEATURE:".length));

  const tr = features.translate("ru", ["Line 1"]);
  const lyr = features.lyrics({ title: "Partners in Crime", artist: "Set It Off", duration: 212 });

  features.ack(req1.id);
  context.runImmediateTimers();
  const nextReq = JSON.parse(context.document.title.slice("YTMFEATURE:".length));
  assert.equal(nextReq.kind, "lyrics");

  features.ack(nextReq.id);
  context.runImmediateTimers();
  const afterReq = JSON.parse(context.document.title.slice("YTMFEATURE:".length));
  assert.equal(afterReq.kind, "translate");

  features.receive(req1.id, { ok: true });
  features.receive(nextReq.id, { ok: true, body: "{}" });
  features.receive(afterReq.id, { ok: true, body: JSON.stringify({ translations: ["Строка 1"] }) });
  await Promise.all([activeReq, tr, lyr]);
});
