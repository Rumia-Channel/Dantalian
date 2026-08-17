const AUDIO_PREBUFFER_SECONDS = 3;
const AUDIO_PREBUFFER_TIMEOUT_MS = 15_000;
const AUDIO_PREBUFFER_POLL_MS = 200;

// 音声再生エンジン。
// キュー要素は { track, album } として保持し、複数CDの曲を混在させる。

class PlayerEngine {
    constructor() {
        this.audio = new Audio();
        this.audio.preload = "metadata";
        this.queue = [];
        this.index = -1;
        this.shuffle = false;
        this.repeatMode = "queue"; // queue | track | off
        this.playOrder = [];
        this.listeners = {};
        this._sourceCandidates = [];
        this._sourceIndex = 0;
        this._sourceSizes = [];
        this._playRequested = false;
        this._playRequestId = 0;
        this._cacheObjectUrl = null;
        this._cacheFormat = null;
        this._loadToken = 0;
        this._loading = false;

        this.audio.addEventListener("timeupdate", () => this._emit("time", this.getPosition()));
        this.audio.addEventListener("loadedmetadata", () => {
            this._emit("duration", this.getDuration());
            this._emit("sourcechange", {
                track: this.current(),
                format: this._sourceFormat(),
                sizeBytes: this._sourceSize(),
                url: this.audio.currentSrc || this._sourceCandidates[this._sourceIndex] || "",
            });
        });
        this.audio.addEventListener("ended", () => this._emit("ended", this.current()));
        this.audio.addEventListener("play", () => this._emit("playstate", true));
        this.audio.addEventListener("pause", () => this._emit("playstate", false));
        this.audio.addEventListener("error", () => this._handleSourceError());
    }

    on(event, fn) {
        (this.listeners[event] = this.listeners[event] || []).push(fn);
    }

    _emit(event, payload) {
        (this.listeners[event] || []).forEach((fn) => fn(payload));
    }

    _normalizeEntry(entry) {
        if (!entry) return null;
        const track = entry.track || entry;
        if (!track || !track.file_hash) return null;
        return { track, album: entry.track ? (entry.album || null) : null };
    }

    _shuffleIndexes(indexes, anchorIndex) {
        const result = [...indexes];
        for (let i = result.length - 1; i > 0; i--) {
            const j = Math.floor(Math.random() * (i + 1));
            [result[i], result[j]] = [result[j], result[i]];
        }
        const anchor = result.indexOf(anchorIndex);
        if (anchor >= 0) result.splice(anchor, 1);
        if (anchorIndex >= 0) result.unshift(anchorIndex);
        return result;
    }

    _rebuildPlayOrder(anchorIndex = this.index) {
        const indexes = this.queue.map((_, i) => i);
        this.playOrder = this.shuffle ? this._shuffleIndexes(indexes, anchorIndex) : indexes;
    }

    _syncPlayOrderAfterAppend(previousLength) {
        if (!this.shuffle) {
            this._rebuildPlayOrder(this.index);
            return;
        }
        const added = this.queue.map((_, i) => i).filter((i) => i >= previousLength);
        this.playOrder.push(...this._shuffleIndexes(added, -1));
    }

    loadQueue(entries, startIndex = 0) {
        this.audio.pause();
        this.queue = (entries || []).map((entry) => this._normalizeEntry(entry)).filter(Boolean);
        this.index = this.queue.length > 0
            ? Math.min(Math.max(Number(startIndex) || 0, 0), this.queue.length - 1)
            : -1;
        this._rebuildPlayOrder(this.index);
        this._emit("queuechange", this.queue);
        if (this.index >= 0) this._loadCurrent(false);
        else {
            this._loadToken += 1;
            this._loading = false;
            this._releaseCachedObjectUrl();
            this._sourceCandidates = [];
            this._sourceSizes = [];
            this.audio.removeAttribute("src");
            this._emit("empty", true);
        }
    }

    // 既存呼び出しとの互換用。CD情報を渡せばキュー要素へ変換する。
    loadTracks(tracks, startTrackId, album = null) {
        const sorted = [...(tracks || [])].sort(
            (a, b) => (a.disc_number - b.disc_number) || (a.track_number - b.track_number)
        );
        const entries = sorted.filter((track) => track.file_hash).map((track) => ({ track, album }));
        const startIndex = entries.findIndex((entry) => entry.track.id === startTrackId);
        this.loadQueue(entries, startIndex >= 0 ? startIndex : 0);
    }

    appendQueue(entries) {
        const additions = (entries || []).map((entry) => this._normalizeEntry(entry)).filter(Boolean);
        if (additions.length === 0) return 0;
        const previousLength = this.queue.length;
        if (previousLength === 0) {
            this.loadQueue(additions, 0);
            return additions.length;
        }
        this.queue.push(...additions);
        this._syncPlayOrderAfterAppend(previousLength);
        this._emit("queuechange", this.queue);
        return additions.length;
    }

    currentEntry() {
        return this.index >= 0 ? this.queue[this.index] || null : null;
    }

    current() {
        const entry = this.currentEntry();
        return entry ? entry.track : null;
    }

    _url(track) {
        if (typeof audioSourceSelection === "function") return audioSourceSelection(track);
        if (typeof audioSourceCandidates === "function") {
            return { urls: audioSourceCandidates(track), sizes: [] };
        }
        return { urls: [`/audio/${encodeURIComponent(track.file_hash)}`], sizes: [] };
    }

    _sourceSize() {
        const size = this._sourceSizes[this._sourceIndex];
        return Number.isFinite(Number(size)) && Number(size) > 0 ? Number(size) : null;
    }

    _sourceFormat() {
        const source = this.audio.currentSrc || this._sourceCandidates[this._sourceIndex] || "";
        if (this._cacheObjectUrl && source === this._cacheObjectUrl) {
            return this._cacheFormat || "original";
        }
        try {
            const parsed = new URL(source, window.location.href);
            const path = parsed.pathname;
            if (path.includes("/audio/encoded/opus/")) return "opus";
            if (path.includes("/audio/encoded/aac/")) return "aac";
            if (path.includes("/api/audio/stream/") && ["opus", "aac"].includes(parsed.searchParams.get("format"))) {
                return parsed.searchParams.get("format");
            }
        } catch {}
        return "original";
    }

    _releaseCachedObjectUrl() {
        if (!this._cacheObjectUrl) return;
        if (typeof releaseCachedAudioSource === "function") {
            releaseCachedAudioSource(this._cacheObjectUrl);
        }
        this._cacheObjectUrl = null;
        this._cacheFormat = null;
    }

    _bufferedAhead() {
        const position = Number(this.audio.currentTime) || 0;
        const ranges = this.audio.buffered;
        for (let index = 0; index < ranges.length; index += 1) {
            const start = ranges.start(index);
            const end = ranges.end(index);
            if (position + 0.25 >= start && position <= end + 0.25) {
                return Math.max(0, end - position);
            }
        }
        return 0;
    }

    _hasPlaybackBuffer() {
        if (this.audio.readyState < 3) return false;
        const duration = Number(this.audio.duration);
        if (!Number.isFinite(duration) || duration <= 0) {
            return this._bufferedAhead() > 0;
        }
        const remaining = Math.max(0, duration - (Number(this.audio.currentTime) || 0));
        const target = Math.min(AUDIO_PREBUFFER_SECONDS, remaining);
        return target <= 0.25 || this._bufferedAhead() >= target;
    }

    _preparePlaybackBuffer() {
        if (this.audio.preload === "auto") return;
        this.audio.preload = "auto";
        if (this.audio.src) this.audio.load();
    }

    async _playWhenBuffered(loadToken, playRequestId) {
        this._preparePlaybackBuffer();
        const deadline = Date.now() + AUDIO_PREBUFFER_TIMEOUT_MS;
        while (
            loadToken === this._loadToken
            && playRequestId === this._playRequestId
            && this._playRequested
            && !this.audio.error
            && !this._hasPlaybackBuffer()
            && Date.now() < deadline
        ) {
            await new Promise((resolve) => setTimeout(resolve, AUDIO_PREBUFFER_POLL_MS));
        }
        if (
            loadToken !== this._loadToken
            || playRequestId !== this._playRequestId
            || !this._playRequested
            || !this.audio.src
        ) {
            return;
        }
        this.audio.play().catch(() => {
            if (playRequestId === this._playRequestId) this._emit("playstate", false);
        });
    }

    _handleSourceError() {
        if (this._loading || this._sourceCandidates.length === 0) return;
        if (this._sourceIndex + 1 < this._sourceCandidates.length) {
            const shouldPlay = this._playRequested || this.isPlaying;
            this._sourceIndex += 1;
            this.audio.src = this._sourceCandidates[this._sourceIndex];
            this.audio.load();
            if (shouldPlay) this._playWhenBuffered(this._loadToken, this._playRequestId);
            return;
        }
        if (this.index >= 0) this._emit("error", this.current());
    }

    async _loadCurrent(autoplay) {
        const token = ++this._loadToken;
        this._playRequestId += 1;
        const entry = this.currentEntry();
        this._loading = true;
        this._playRequested = Boolean(autoplay);
        this.audio.pause();
        this._releaseCachedObjectUrl();
        this._sourceCandidates = [];
        this._sourceIndex = 0;
        this._sourceSizes = [];
        if (!entry) {
            this.audio.removeAttribute("src");
            this._loading = false;
            this._emit("empty", true);
            return;
        }

        const selection = await Promise.resolve(this._url(entry.track));
        const networkCandidates = Array.isArray(selection) ? selection : (selection?.urls || []);
        const networkSizes = Array.isArray(selection) ? [] : (selection?.sizes || []);
        let cachedSource = null;
        if (typeof getCachedAudioSource === "function") {
            cachedSource = await getCachedAudioSource(entry.track);
        }
        if (token !== this._loadToken || this.currentEntry() !== entry) {
            if (cachedSource && typeof releaseCachedAudioSource === "function") {
                releaseCachedAudioSource(cachedSource.url);
            }
            return;
        }

        this._sourceCandidates = cachedSource
            ? [cachedSource.url, ...networkCandidates]
            : networkCandidates;
        this._sourceSizes = cachedSource
            ? [cachedSource.size || null, ...networkSizes]
            : networkSizes;
        this._sourceIndex = 0;
        if (this._sourceCandidates.length === 0) {
            this._loading = false;
            this._emit("trackchange", entry.track);
            this._emit("error", entry.track);
            return;
        }
        const shouldPlay = this._playRequested || autoplay;
        this.audio.preload = shouldPlay ? "auto" : "metadata";
        this.audio.src = this._sourceCandidates[0] || "";
        this.audio.load();
        this._loading = false;
        this._emit("trackchange", entry.track);
        if (shouldPlay) this._playWhenBuffered(token, this._playRequestId);
    }

    play() {
        this._playRequested = true;
        const playRequestId = ++this._playRequestId;
        if (this.index < 0 && this.queue.length > 0) {
            this.index = 0;
            this._rebuildPlayOrder(this.index);
        }
        if (this._loading) return;
        if (this.current() && !this.audio.src) {
            this._loadCurrent(true);
            return;
        }
        this._playWhenBuffered(this._loadToken, playRequestId);
    }

    pause() {
        this._playRequested = false;
        this._playRequestId += 1;
        this.audio.pause();
    }

    toggle() {
        if (this.audio.paused) this.play();
        else this.pause();
    }

    get isPlaying() {
        return !this.audio.paused && !this.audio.ended;
    }

    _finishQueue() {
        this.audio.pause();
        this._emit("queueend", this.currentEntry());
    }

    next(fromEnd = false) {
        if (this.queue.length === 0) return false;
        const position = this.playOrder.indexOf(this.index);
        let nextPosition = position + 1;
        if (nextPosition >= this.playOrder.length) {
            if (fromEnd && this.repeatMode === "off") {
                this._finishQueue();
                return false;
            }
            if (this.repeatMode === "queue" || !fromEnd) {
                this._rebuildPlayOrder(this.index);
                nextPosition = this.playOrder.length > 1 ? 1 : 0;
            } else {
                this._finishQueue();
                return false;
            }
        }
        this.index = this.playOrder[nextPosition];
        this._loadCurrent(true);
        return true;
    }

    advanceAfterEnded() {
        if (this.repeatMode === "track") {
            this.audio.currentTime = 0;
            this.play();
            return true;
        }
        return this.next(true);
    }

    setShuffle(enabled) {
        this.shuffle = Boolean(enabled);
        this._rebuildPlayOrder(this.index);
        this._emit("shuffle", this.shuffle);
        this._emit("queuechange", this.queue);
    }

    toggleShuffle() {
        this.setShuffle(!this.shuffle);
        return this.shuffle;
    }

    setRepeatMode(mode) {
        if (!["queue", "track", "off"].includes(mode)) return this.repeatMode;
        this.repeatMode = mode;
        this._emit("repeat", this.repeatMode);
        return this.repeatMode;
    }

    toggleRepeatMode() {
        const next = { queue: "track", track: "off", off: "queue" }[this.repeatMode];
        return this.setRepeatMode(next);
    }

    prev() {
        if (this.queue.length === 0) return false;
        if (this.audio.currentTime > 3) {
            this.audio.currentTime = 0;
            return true;
        }
        const position = this.playOrder.indexOf(this.index);
        let previousPosition = position - 1;
        if (previousPosition < 0) {
            if (this.repeatMode === "queue") previousPosition = this.playOrder.length - 1;
            else previousPosition = 0;
        }
        this.index = this.playOrder[previousPosition];
        this._loadCurrent(true);
        return true;
    }

    playQueueIndex(index) {
        const numericIndex = Number(index);
        if (!Number.isInteger(numericIndex) || numericIndex < 0 || numericIndex >= this.queue.length) return false;
        this.index = numericIndex;
        this._rebuildPlayOrder(this.index);
        this._loadCurrent(true);
        return true;
    }

    playTrackById(trackId) {
        const i = this.queue.findIndex((entry) => entry.track.id === trackId);
        return i >= 0 ? this.playQueueIndex(i) : false;
    }

    removeQueueIndex(index) {
        const numericIndex = Number(index);
        if (!Number.isInteger(numericIndex) || numericIndex < 0 || numericIndex >= this.queue.length) return false;
        const wasCurrent = numericIndex === this.index;
        const wasPlaying = this.isPlaying;
        this.queue.splice(numericIndex, 1);
        if (this.queue.length === 0) {
            this.clearQueue();
            return true;
        }
        if (numericIndex < this.index) this.index -= 1;
        if (wasCurrent) {
            this.index = Math.min(numericIndex, this.queue.length - 1);
            this._rebuildPlayOrder(this.index);
            this._loadCurrent(wasPlaying);
        } else {
            this._rebuildPlayOrder(this.index);
        }
        this._emit("queuechange", this.queue);
        return true;
    }

    clearQueue() {
        this._loadToken += 1;
        this._loading = false;
        this.audio.pause();
        this._releaseCachedObjectUrl();
        this.queue = [];
        this.index = -1;
        this.playOrder = [];
        this.audio.removeAttribute("src");
        this._emit("queuechange", this.queue);
        this._emit("empty", true);
    }

    seek(fraction) {
        const dur = this.getDuration();
        if (!isFinite(dur) || dur <= 0) return;
        this.audio.currentTime = Math.min(Math.max(fraction, 0), 1) * dur;
    }

    setPosition(seconds) {
        const position = Number(seconds);
        if (!Number.isFinite(position) || position < 0) return false;
        const duration = this.getDuration();
        this.audio.currentTime = duration > 0 ? Math.min(position, duration) : position;
        this._emit("time", this.getPosition());
        return true;
    }

    getPosition() {
        return this.audio.currentTime || 0;
    }

    getDuration() {
        return this.audio.duration || 0;
    }

    setVolume(v) {
        this.audio.volume = Math.min(Math.max(v, 0), 1);
    }

    get volume() {
        return this.audio.volume;
    }

    get muted() {
        return this.audio.muted;
    }

    toggleMute() {
        this.audio.muted = !this.audio.muted;
        return this.audio.muted;
    }
}
