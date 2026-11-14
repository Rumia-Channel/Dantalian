import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

const source = await readFile(new URL("../../static/js/player/engine.js", import.meta.url), "utf8");
const settle = () => new Promise(setImmediate);

// Model the browser boundary, not PlayerEngine: volume can echo writes without
// attenuating sound, Web Audio requires a gesture, and opaque media is silent.
function playerPage({
  userAgent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15",
  platform = "MacIntel",
  maxTouchPoints = 5,
  elementVolumeWorks = false,
  volumeReadback = true,
} = {}) {
  const activation = { isActive: false };
  const contexts = [];

  class Audio extends EventTarget {
    constructor() {
      super();
      this._volume = 1;
      this._src = "";
      this.muted = false;
      this.paused = true;
      this.currentTime = 0;
      this.duration = 30;
      this.readyState = 4;
      this.buffered = { length: 1, start: () => 0, end: () => 30 };
    }
    get volume() { return volumeReadback ? this._volume : 1; }
    set volume(value) { this._volume = value; }
    get src() { return this._src; }
    set src(value) {
      this._src = value;
      this.requestCrossOrigin = this.crossOrigin;
    }
    load() {}
    removeAttribute(name) { if (name === "src") this._src = ""; }
    pause() { this.paused = true; }
    play() {
      this.paused = false;
      // Native play events are dispatched after the gesture handler returns.
      queueMicrotask(() => this.dispatchEvent(new Event("play")));
      return Promise.resolve();
    }
  }

  class AudioNode {
    connect(destination) { this.destination = destination; return destination; }
  }
  class AudioContext {
    constructor() {
      this.state = "suspended";
      this.destination = new AudioNode();
      contexts.push(this);
    }
    createMediaElementSource(audio) {
      const node = new AudioNode();
      node.context = this;
      audio.sourceNode = node;
      return node;
    }
    createGain() {
      const node = new AudioNode();
      node.gain = { value: 1 };
      return node;
    }
    resume() {
      if (activation.isActive) this.state = "running";
      return Promise.resolve();
    }
  }

  const sandbox = vm.createContext({
    Audio, URL, setTimeout,
    window: { AudioContext, location: { href: "https://library.test/music/" } },
    navigator: { userAgent, platform, maxTouchPoints, userActivation: activation },
  });
  vm.runInContext(source, sandbox);
  const engine = vm.runInContext("new PlayerEngine()", sandbox);
  return {
    engine,
    contexts,
    gesture(callback) {
      activation.isActive = true;
      try { return callback(); } finally { activation.isActive = false; }
    },
    audibleLevel() {
      const audio = engine.audio;
      if (audio.paused || audio.muted || !audio.src) return 0;
      let level = elementVolumeWorks ? audio._volume : 1;
      let node = audio.sourceNode;
      if (!node) return level;
      const context = node.context;
      // Like Worker playback, the same-origin media URL redirects to storage.
      if (context.state !== "running" || audio.requestCrossOrigin !== "anonymous") return 0;
      while (node && node !== context.destination) {
        if (node.gain) level *= node.gain.value;
        node = node.destination;
      }
      return node === context.destination ? level : 0;
    },
  };
}

const tracks = [
  { id: 1, file_hash: "first.wav" },
  { id: 2, file_hash: "second.wav" },
];

for (const [name, options] of [
  ["desktop-mode iPadOS with writable volume", {}],
  ["iPhone with writable volume", { userAgent: "Mozilla/5.0 (iPhone) AppleWebKit/605.1.15", platform: "iPhone", maxTouchPoints: 5 }],
  ["older iPad with read-only volume", { userAgent: "Mozilla/5.0 (iPad) AppleWebKit/605.1.15", platform: "iPad", volumeReadback: false }],
  ["desktop Safari", { maxTouchPoints: 0, elementVolumeWorks: true }],
]) {
  test(`${name}: saved volume, adjustment and mute affect audible output`, async () => {
    const page = playerPage(options);
    const { engine } = page;
    engine.setVolume(0.4);
    engine.loadQueue(tracks);
    // Start while source resolution is still pending, as the album UI does.
    page.gesture(() => engine.play());
    await settle();
    assert.equal(page.audibleLevel(), 0.4, "saved volume must apply to the first audible frame");

    engine.setVolume(0.2);
    assert.equal(page.audibleLevel(), 0.2, "slider movement must attenuate the sound");
    engine.toggleMute();
    engine.setVolume(0.6);
    assert.equal(page.audibleLevel(), 0, "changing volume must not defeat mute");
    engine.toggleMute();
    assert.equal(page.audibleLevel(), 0.6, "unmute must restore the selected level");

    page.gesture(() => engine.next());
    await settle();
    assert.equal(page.audibleLevel(), 0.6, "track changes must keep the selected level");
  });
}

for (const state of ["suspended", "interrupted"]) {
  test(`iPadOS: user playback recovers sound from an AudioContext that is ${state}`, async () => {
    const page = playerPage();
    const { engine } = page;
    engine.setVolume(0.3);
    engine.loadQueue(tracks);
    page.gesture(() => engine.play());
    await settle();
    assert.equal(page.audibleLevel(), 0.3);

    engine.pause();
    page.contexts[0].state = state;
    page.gesture(() => engine.play());
    await settle();
    assert.equal(page.audibleLevel(), 0.3, "resume must run inside the play gesture");

    page.contexts[0].state = state;
    page.gesture(() => engine.playQueueIndex(1));
    await settle();
    assert.equal(page.audibleLevel(), 0.3, "track selection must resume before async source loading");
  });
}
