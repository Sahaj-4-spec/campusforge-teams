/*!
 * Power Intro 1.0
 * A start-up animation for websites. No dependencies.
 *
 *   PowerIntro.start({ brand: 'ACME', exit: 'split' });
 *
 * or, with no code, add data attributes to the script tag:
 *
 *   <script src="js/power-intro.js" defer data-auto data-brand="ACME"></script>
 */
(function (global, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory(global);
  else global.PowerIntro = factory(global);
})(typeof window !== 'undefined' ? window : this, function (window) {
  'use strict';

  var document = window.document;
  var scriptEl = document && document.currentScript;

  var DEFAULTS = {
    brand: '',                 // wordmark shown under the switch
    tagline: '',               // small line under the wordmark
    theme: 'red',              // red | blue | green | violet
    exit: 'split',             // split | zoom | fade
    once: false,               // false | true / 'session' | 'visit'
    storageKey: 'powerIntroSeen',
    sound: false,              // synthesized sound (browsers may wait for a tap)
    soundButton: true,         // show the sound on/off button
    skippable: true,           // show the Skip button, Esc also skips
    speed: 1,                  // 1 = about 5 seconds, 1.5 = faster, 0.75 = slower
    lockScroll: true,          // stop the page scrolling under the intro
    respectReducedMotion: true,
    zIndex: 99999,
    labels: { standby: 'Standby', unlock: 'Unlocking', charge: 'Power up', online: 'Online' },
    onStart: null,
    onProgress: null,          // function (progress 0..1)
    onIgnite: null,
    onDone: null               // function (reason: 'complete' | 'skipped')
  };

  var GLOW = {
    red: [255, 106, 61], blue: [79, 180, 255], green: [93, 255, 160], violet: [180, 140, 255]
  };
  var EXIT_MS = { split: 1100, zoom: 950, fade: 800 };
  var SEGMENTS = 24;

  var MARKUP =
    '<div class="pi-curtain pi-top"></div>' +
    '<div class="pi-curtain pi-bot"></div>' +
    '<div class="pi-bg"></div>' +
    '<div class="pi-scan"></div>' +
    '<canvas class="pi-fx"></canvas>' +
    '<div class="pi-vignette"></div>' +
    '<div class="pi-flash"></div>' +
    '<div class="pi-seam"></div>' +
    '<div class="pi-stage">' +
      '<div class="pi-pillwrap">' +
        '<div class="pi-readout" aria-hidden="true"><span class="pi-status"></span><span class="pi-pct">000%</span></div>' +
        '<div class="pi-pill">' +
          '<i class="pi-screw t l"></i><i class="pi-screw t m"></i><i class="pi-screw t r"></i>' +
          '<i class="pi-screw b l"></i><i class="pi-screw b m"></i><i class="pi-screw b r"></i>' +
          '<div class="pi-ring"><div class="pi-bay">' +
            '<div class="pi-light"></div>' +
            '<div class="pi-plate top"></div><div class="pi-plate bot"></div>' +
            '<div class="pi-sheen"></div>' +
            '<div class="pi-core">' +
              '<svg viewBox="0 0 200 200" aria-hidden="true">' +
                '<defs>' +
                  '<linearGradient id="pi-brass" x1="0" y1="0" x2="1" y2="1">' +
                    '<stop offset="0" stop-color="#f3d27a"/><stop offset=".5" stop-color="#b98a2e"/><stop offset="1" stop-color="#6e4d14"/>' +
                  '</linearGradient>' +
                  '<radialGradient id="pi-orb" cx=".4" cy=".35" r=".8">' +
                    '<stop offset="0" style="stop-color:var(--pi-glow-hi)"/><stop offset=".45" style="stop-color:var(--pi-glow)"/><stop offset="1" style="stop-color:var(--pi-lo)"/>' +
                  '</radialGradient>' +
                  '<path id="pi-vane" d="M92 28 H108 L103 64 H97 Z"/>' +
                  '<g id="pi-vanes">' + vaneUses() + '</g>' +
                '</defs>' +
                '<circle cx="100" cy="100" r="100" fill="#120a0c"/>' +
                '<circle cx="100" cy="100" r="95" fill="#160c0e" stroke="url(#pi-brass)" stroke-width="8"/>' +
                '<circle class="pi-ticks" cx="100" cy="100" r="82" fill="none" stroke="#6e4d14" stroke-width="5" stroke-dasharray="2 6.2"/>' +
                '<circle class="pi-counter" cx="100" cy="100" r="72" fill="none" stroke="#3b2a0e" stroke-width="2" stroke-dasharray="10 4"/>' +
                '<g class="pi-spin">' +
                  '<use href="#pi-vanes" fill="#5b4017"/>' +
                  '<use class="pi-lit" href="#pi-vanes" style="fill:var(--pi-glow)"/>' +
                '</g>' +
                '<circle cx="100" cy="100" r="33" fill="#0b0607" stroke="url(#pi-brass)" stroke-width="5"/>' +
                '<circle class="pi-orb" cx="100" cy="100" r="22" fill="url(#pi-orb)"/>' +
                '<circle cx="100" cy="100" r="107" fill="none" stroke="rgba(255,255,255,.10)" stroke-width="2.5"/>' +
                '<circle class="pi-arc" cx="100" cy="100" r="107" fill="none" stroke-width="3.5" stroke-linecap="round" pathLength="100" stroke-dasharray="100 100" stroke-dashoffset="100" style="stroke:var(--pi-glow)"/>' +
              '</svg>' +
            '</div>' +
          '</div></div>' +
        '</div>' +
        '<div class="pi-bar" aria-hidden="true">' + new Array(SEGMENTS + 1).join('<i></i>') + '</div>' +
      '</div>' +
      '<div class="pi-brand"><span class="pi-brand-text"></span><span class="pi-tag"></span></div>' +
    '</div>' +
    '<button type="button" class="pi-sound"></button>' +
    '<button type="button" class="pi-skip">Skip</button>';

  function vaneUses() {
    var s = '<use href="#pi-vane"/>';
    for (var i = 1; i < 12; i++) s += '<use href="#pi-vane" transform="rotate(' + (i * 30) + ' 100 100)"/>';
    return s;
  }

  function merge(a, b) {
    var r = {}, k;
    for (k in a) r[k] = a[k];
    for (k in b) {
      if (b[k] === undefined) continue;
      r[k] = (k === 'labels' && b[k] && typeof b[k] === 'object') ? merge(a.labels, b[k]) : b[k];
    }
    return r;
  }
  function easeCubic(t) {
    t = Math.min(1, Math.max(0, t));
    return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
  }
  function easeSine(t) {
    t = Math.min(1, Math.max(0, t));
    return -(Math.cos(Math.PI * t) - 1) / 2;
  }

  /* ---------- "show once" memory ---------- */
  function store(o) {
    return o.once === 'visit' ? window.localStorage : window.sessionStorage;
  }
  function seen(o) {
    try {
      if (/[?&]intro(=|&|$)/.test(window.location.search)) return false; // ?intro forces a replay
      return !!(o.once && store(o).getItem(o.storageKey));
    } catch (e) { return false; }
  }
  function mark(o) {
    try { if (o.once) store(o).setItem(o.storageKey, '1'); } catch (e) { /* storage blocked */ }
  }

  var current = null;

  /* ---------- main ---------- */
  function start(userOpts) {
    var o = merge(DEFAULTS, userOpts || {});
    var L = o.labels;
    var resolveFn;
    var api = {
      skip: function () {},
      destroy: function () {},
      replay: function (more) { return start(merge(userOpts || {}, more || {})); },
      finished: new Promise(function (res) { resolveFn = res; })
    };

    var reduced = !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
    if ((o.once && seen(o)) || (reduced && o.respectReducedMotion)) {
      mark(o);
      resolveFn('skipped');
      return api;
    }
    if (current) current.destroy();
    if (!document.body) { // script ran too early
      document.addEventListener('DOMContentLoaded', function () { start(userOpts); });
      return api;
    }

    var el = document.createElement('div');
    el.className = 'pi-root pi-theme-' + (GLOW[o.theme] ? o.theme : 'red');
    el.style.zIndex = String(o.zIndex);
    el.setAttribute('role', 'dialog');
    el.setAttribute('aria-modal', 'true');
    el.setAttribute('aria-label', 'Intro animation');
    el.innerHTML = MARKUP;
    document.body.appendChild(el);

    var $ = function (s) { return el.querySelector(s); };
    var statusEl = $('.pi-status'), pctEl = $('.pi-pct'), arc = $('.pi-arc');
    var core = $('.pi-core'), canvas = $('.pi-fx'), ctx = canvas.getContext('2d');
    var skipBtn = $('.pi-skip'), soundBtn = $('.pi-sound');
    var segs = el.querySelectorAll('.pi-bar i');
    var glow = GLOW[o.theme] || GLOW.red;

    if (o.brand) $('.pi-brand-text').textContent = o.brand;
    if (o.tagline) $('.pi-tag').textContent = o.tagline;
    if (!o.skippable) skipBtn.parentNode.removeChild(skipBtn);

    /* page lock, focus, inert background */
    var inerted = [], prevFocus = document.activeElement;
    if (o.lockScroll) document.documentElement.classList.add('pi-lock');
    Array.prototype.forEach.call(document.body.children, function (c) {
      if (c !== el && c.tagName !== 'SCRIPT' && 'inert' in c && !c.inert) { c.inert = true; inerted.push(c); }
    });
    if (o.skippable) skipBtn.focus({ preventScroll: true });

    /* ---------- sound (all synthesized, no files) ---------- */
    var AC = window.AudioContext || window.webkitAudioContext;
    var audio = null, hum = null, humGain = null, humFilter = null, master = null;
    var soundOn = !!o.sound && !!AC;
    if (!AC || !o.soundButton) soundBtn.parentNode.removeChild(soundBtn);
    function paintSoundBtn() { if (soundBtn.parentNode) soundBtn.textContent = soundOn ? 'Sound on' : 'Sound off'; }
    paintSoundBtn();

    function audioStart() {
      if (!AC) return false;
      try {
        if (!audio) {
          audio = new AC();
          master = audio.createGain(); master.gain.value = 0.55; master.connect(audio.destination);
          humFilter = audio.createBiquadFilter(); humFilter.type = 'lowpass'; humFilter.frequency.value = 220;
          humGain = audio.createGain(); humGain.gain.value = 0;
          hum = audio.createOscillator(); hum.type = 'sawtooth'; hum.frequency.value = 48;
          var sub = audio.createOscillator(); sub.type = 'sine'; sub.frequency.value = 24;
          hum.connect(humFilter); sub.connect(humFilter); humFilter.connect(humGain); humGain.connect(master);
          hum.start(); sub.start();
          hum._sub = sub;
        }
        if (audio.state === 'suspended') audio.resume();
        return true;
      } catch (e) { return false; }
    }
    function noiseBurst(dur, freq, q, vol) {
      var n = Math.floor(audio.sampleRate * dur), buf = audio.createBuffer(1, n, audio.sampleRate), d = buf.getChannelData(0);
      for (var i = 0; i < n; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / n);
      var src = audio.createBufferSource(); src.buffer = buf;
      var f = audio.createBiquadFilter(); f.type = 'bandpass'; f.frequency.value = freq; f.Q.value = q;
      var g = audio.createGain(); g.gain.value = vol;
      src.connect(f); f.connect(g); g.connect(master); src.start();
    }
    function sfxUnlock() {
      if (!soundOn || !audio) return;
      noiseBurst(0.18, 900, 2, 0.5);
      var t = audio.currentTime, osc = audio.createOscillator(), g = audio.createGain();
      osc.type = 'square'; osc.frequency.setValueAtTime(180, t); osc.frequency.exponentialRampToValueAtTime(60, t + 0.15);
      g.gain.setValueAtTime(0.18, t); g.gain.exponentialRampToValueAtTime(0.001, t + 0.2);
      osc.connect(g); g.connect(master); osc.start(t); osc.stop(t + 0.22);
    }
    function sfxIgnite() {
      if (!soundOn || !audio) return;
      var t = audio.currentTime, osc = audio.createOscillator(), g = audio.createGain();
      osc.type = 'sine'; osc.frequency.setValueAtTime(150, t); osc.frequency.exponentialRampToValueAtTime(34, t + 0.9);
      g.gain.setValueAtTime(0.9, t); g.gain.exponentialRampToValueAtTime(0.001, t + 1.1);
      osc.connect(g); g.connect(master); osc.start(t); osc.stop(t + 1.2);
      noiseBurst(0.9, 1800, 0.6, 0.35);
    }
    function audioFrame(p, charging) {
      if (!soundOn || !audio) return;
      var t = audio.currentTime;
      hum.frequency.setTargetAtTime(46 + p * 150, t, 0.06);
      hum._sub.frequency.setTargetAtTime(23 + p * 75, t, 0.06);
      humFilter.frequency.setTargetAtTime(160 + p * 900, t, 0.08);
      humGain.gain.setTargetAtTime(charging ? 0.04 + p * 0.16 : 0, t, 0.08);
    }
    soundBtn.addEventListener('click', function () {
      soundOn = !soundOn;
      if (soundOn) { audioStart(); } else if (humGain) { humGain.gain.setTargetAtTime(0, audio.currentTime, 0.05); }
      paintSoundBtn();
    });
    function wakeAudio() { if (soundOn) audioStart(); }
    if (soundOn) {
      audioStart();
      document.addEventListener('pointerdown', wakeAudio, { once: true });
      document.addEventListener('keydown', wakeAudio, { once: true });
    }

    /* ---------- sparks ---------- */
    var parts = [], W = 0, H = 0, dpr = 1, acc = 0, emberAcc = 0;
    function resize() {
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      W = window.innerWidth; H = window.innerHeight;
      canvas.width = Math.round(W * dpr); canvas.height = Math.round(H * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    resize();
    window.addEventListener('resize', resize);

    function spawn(x, y, vx, vy, life, size, a) {
      if (parts.length > 700) return;
      parts.push({ x: x, y: y, vx: vx, vy: vy, t: 0, life: life, size: size, a: a });
    }
    function burst(x, y, n) {
      for (var i = 0; i < n; i++) {
        var ang = Math.random() * Math.PI * 2, sp = 120 + Math.random() * 480;
        spawn(x, y, Math.cos(ang) * sp, Math.sin(ang) * sp, 0.6 + Math.random() * 0.8, 1 + Math.random() * 2.2, 1);
      }
    }
    function stepParts(dt) {
      ctx.clearRect(0, 0, W, H);
      ctx.globalCompositeOperation = 'lighter';
      var c = glow[0] + ',' + glow[1] + ',' + glow[2];
      for (var i = parts.length - 1; i >= 0; i--) {
        var q = parts[i];
        q.t += dt;
        if (q.t >= q.life) { parts.splice(i, 1); continue; }
        q.vy += 140 * dt;           // a little gravity
        q.vx *= 1 - 0.8 * dt;       // drag
        q.x += q.vx * dt; q.y += q.vy * dt;
        var k = 1 - q.t / q.life, al = q.a * k * k;
        ctx.fillStyle = 'rgba(' + c + ',' + (al * 0.14).toFixed(3) + ')';
        ctx.beginPath(); ctx.arc(q.x, q.y, q.size * 3.4, 0, 6.2832); ctx.fill();
        ctx.fillStyle = 'rgba(255,' + Math.min(255, glow[1] + 90) + ',' + Math.min(255, glow[2] + 110) + ',' + al.toFixed(3) + ')';
        ctx.beginPath(); ctx.arc(q.x, q.y, q.size, 0, 6.2832); ctx.fill();
      }
      ctx.globalCompositeOperation = 'source-over';
    }

    /* ---------- timeline (milliseconds, scaled by speed) ---------- */
    var T = { unlock: 400, charge: 1100, ignite: 3300, brand: 3450, exit: 4300 };
    var e = 0, last = performance.now(), raf = 0, rot = 0;
    var lit = -1, phase = 0, ignited = false, brandIn = false, exiting = false, dead = false, unlocked = false, exitTimer = 0;

    function render(p, open, fl, label) {
      el.style.setProperty('--p', p.toFixed(4));
      el.style.setProperty('--open', open.toFixed(4));
      el.style.setProperty('--rot', rot.toFixed(1));
      el.style.setProperty('--fl', fl.toFixed(2));
      arc.style.strokeDashoffset = String(100 - p * 100);
      var n = Math.round(p * 100);
      pctEl.textContent = (n < 10 ? '00' : n < 100 ? '0' : '') + n + '%';
      if (statusEl.textContent !== label) statusEl.textContent = label;
      var count = Math.floor(p * SEGMENTS + 0.0001);
      if (count !== lit) {
        for (var i = 0; i < SEGMENTS; i++) segs[i].className = i < count ? 'on' : '';
        lit = count;
      }
    }

    function beginExit(skipped) {
      if (exiting) return;
      exiting = true;
      var mode = EXIT_MS[o.exit] ? o.exit : 'split';
      if (!brandIn && !skipped) el.classList.add('pi-brand-in');
      el.classList.add('pi-exit', 'pi-exit-' + mode);
      exitTimer = setTimeout(function () { finish(skipped ? 'skipped' : 'complete'); }, EXIT_MS[mode] + 80);
    }

    function frame(now) {
      if (dead) return;
      var dt = Math.min(50, now - last); last = now;
      if (!exiting) e += dt * o.speed;
      var p = 0, open = 0, fl = 0, label = L.standby;

      if (exiting && ignited) { p = 1; open = 1; label = L.online; }
      else if (exiting) { p = 1; open = 1; label = L.online; }
      else {
        if (e < T.unlock) {
          fl = Math.random() < 0.35 ? 0.3 + Math.random() * 0.7 : 0;           // boot flicker
        } else if (e < T.charge) {
          label = L.unlock;
          fl = Math.random() < 0.12 ? Math.random() : 0;
        } else {
          label = L.charge;
        }
        open = easeCubic((e - T.unlock) / (T.charge - T.unlock));
        p = easeSine((e - T.charge) / (T.ignite - T.charge));
        if (p >= 1) label = L.online;
      }

      if (!unlocked && e >= T.unlock) { unlocked = true; sfxUnlock(); }

      rot += dt * (0.03 + p * 0.45);

      /* sparks follow the core */
      var r = core.getBoundingClientRect(), cx = r.left + r.width / 2, cy = r.top + r.height / 2;
      var dts = dt / 1000;
      if (!exiting && p > 0 && p < 1) {
        acc += dts * (40 + 190 * p);
        while (acc >= 1) {
          acc -= 1;
          spawn(cx - r.width * 0.3, cy + (Math.random() - 0.5) * r.height * 0.6,
                -(40 + Math.random() * 140), (Math.random() - 0.5) * 240 - 30,
                0.5 + Math.random() * 0.6, 0.8 + Math.random() * 1.6, 0.9);
        }
      }
      if (!exiting) {
        emberAcc += dts * 14;
        while (emberAcc >= 1) {
          emberAcc -= 1;
          spawn(Math.random() * W, H + 6, (Math.random() - 0.5) * 20, -(30 + Math.random() * 70), 2.5 + Math.random() * 2, 0.7 + Math.random() * 1.1, 0.45);
        }
      }

      if (!exiting && !ignited && e >= T.ignite) {
        ignited = true;
        el.classList.add('pi-on');
        burst(cx, cy, 150);
        sfxIgnite();
        if (o.onIgnite) try { o.onIgnite(); } catch (err) { /* ignore */ }
      }
      if (!exiting && !brandIn && e >= T.brand) { brandIn = true; el.classList.add('pi-brand-in'); }

      render(p, open, fl, label);
      audioFrame(p, !exiting && p < 1 && p > 0);
      if (o.onProgress && !exiting) try { o.onProgress(p); } catch (err) { /* ignore */ }

      stepParts(dts);
      if (!exiting && e >= T.exit) beginExit(false);
      raf = requestAnimationFrame(frame);
    }

    function finish(reason) {
      if (dead) return;
      dead = true;
      clearTimeout(exitTimer);
      cancelAnimationFrame(raf);
      window.removeEventListener('resize', resize);
      document.removeEventListener('keydown', onKey, true);
      document.removeEventListener('pointerdown', wakeAudio);
      document.removeEventListener('keydown', wakeAudio);
      if (humGain) { try { humGain.gain.setTargetAtTime(0, audio.currentTime, 0.05); } catch (err) { /* ignore */ } }
      if (audio) setTimeout(function () { try { audio.close(); } catch (err) { /* ignore */ } }, 1500);
      if (el.parentNode) el.parentNode.removeChild(el);
      inerted.forEach(function (c) { c.inert = false; });
      if (o.lockScroll) document.documentElement.classList.remove('pi-lock');
      if (prevFocus && prevFocus.focus) { try { prevFocus.focus({ preventScroll: true }); } catch (err) { /* ignore */ } }
      if (current === api) current = null;
      mark(o);
      resolveFn(reason);
      try { document.dispatchEvent(new CustomEvent('powerintro:done', { detail: { reason: reason } })); } catch (err) { /* old browser */ }
      if (o.onDone) try { o.onDone(reason); } catch (err) { /* ignore */ }
    }

    function onKey(ev) {
      if (ev.key === 'Escape' && o.skippable) { ev.preventDefault(); api.skip(); }
      else if (ev.key === 'Tab') { // keep focus inside the intro
        var targets = el.querySelectorAll('button');
        if (!targets.length) { ev.preventDefault(); return; }
        var first = targets[0], lastBtn = targets[targets.length - 1];
        if (ev.shiftKey && document.activeElement === first) { ev.preventDefault(); lastBtn.focus(); }
        else if (!ev.shiftKey && document.activeElement === lastBtn) { ev.preventDefault(); first.focus(); }
      }
    }
    document.addEventListener('keydown', onKey, true);

    api.skip = function () {
      if (dead || exiting) return;
      ignited = true;
      beginExit(true);
    };
    api.destroy = function () { finish('destroyed'); };
    if (o.skippable) skipBtn.addEventListener('click', function () { api.skip(); });

    current = api;
    render(0, 0, 0, L.standby);
    if (o.onStart) try { o.onStart(); } catch (err) { /* ignore */ }
    raf = requestAnimationFrame(function (now) { last = now; frame(now); });
    return api;
  }

  /* ---------- script-tag mode: <script ... data-auto data-brand="..."> ---------- */
  function readAttrs(s) {
    var out = {}, map = { speed: Number, sound: bool, skippable: bool, lockScroll: bool, soundButton: bool };
    function bool(v) { return v !== 'false' && v !== '0'; }
    ['brand', 'tagline', 'theme', 'exit', 'once', 'storageKey', 'speed', 'sound', 'skippable', 'lockScroll', 'soundButton'].forEach(function (k) {
      var attr = 'data-' + k.replace(/[A-Z]/g, function (m) { return '-' + m.toLowerCase(); });
      if (s.hasAttribute(attr)) {
        var v = s.getAttribute(attr);
        out[k] = map[k] ? map[k](v) : v;
      }
    });
    if (out.once === 'true' || out.once === '') out.once = true;
    if (out.once === 'false') out.once = false;
    return out;
  }
  if (scriptEl && scriptEl.hasAttribute && scriptEl.hasAttribute('data-auto')) {
    var go = function () { start(readAttrs(scriptEl)); };
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', go);
    else go();
  }

  return { start: start, version: '1.0.0' };
});
