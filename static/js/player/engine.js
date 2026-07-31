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

        this.audio.addEventListener("timeupdate", () => this._emit("time", this.getPosition()));
        this.audio.addEventListener("loadedmetadata", () => this._emit("duration", this.getDuration()));
        this.audio.addEventListener("ended", () => this._emit("ended", this.current()));
        this.audio.addEventListener("play", () => this._emit("playstate", true));
        this.audio.addEventListener("pause", () => this._emit("playstate", false));
        this.audio.addEventListener("error", () => {
            if (this.index >= 0) this._emit("error", this.current());
        });
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
        return `/audio/${track.file_hash}`;
    }

    _loadCurrent(autoplay) {
        const entry = this.currentEntry();
        if (!entry) {
            this.audio.removeAttribute("src");
            this._emit("empty", true);
            return;
        }
        this.audio.src = this._url(entry.track);
        this._emit("trackchange", entry.track);
        if (autoplay) this.audio.play().catch(() => this._emit("playstate", false));
    }

    play() {
        if (this.index < 0 && this.queue.length > 0) {
            this.index = 0;
            this._rebuildPlayOrder(this.index);
        }
        if (this.current() && !this.audio.src) this._loadCurrent(false);
        this.audio.play().catch(() => {});
    }

    pause() {
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
        this.audio.pause();
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
