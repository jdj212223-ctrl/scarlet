// Runs inside the YouTube, YouTube Music and YouTube TV pages. Rust sets window.__scarlet = { adblock, sponsorblock }
// before this runs and updates it live when you flip a toggle in the top bar.
(() => {
  if (window.__scarletLoaded) return;
  window.__scarletLoaded = true;
  window.__scarlet = window.__scarlet || { adblock: true, sponsorblock: true };
  const on = k => !!(window.__scarlet && window.__scarlet[k]);

  // --- Ad skipping: hide ad slots, and when a video ad plays, skip it or jump to its end. ---
  const AD_CSS = `
    ytd-ad-slot-renderer, ytd-in-feed-ad-layout-renderer, ytd-banner-promo-renderer, ytd-statement-banner-renderer,
    ytd-promoted-sparkles-web-renderer, ytd-display-ad-renderer, ytd-companion-slot-renderer, #player-ads, #masthead-ad,
    .ytp-ad-overlay-container, ytmusic-mealbar-promo-renderer { display: none !important; }`;
  let style;
  const syncStyle = () => {
    if (on('adblock') && !style) { style = document.createElement('style'); style.textContent = AD_CSS; (document.head || document.documentElement).appendChild(style); }
    if (!on('adblock') && style) { style.remove(); style = null; }
  };
  const skipAd = () => {
    if (!on('adblock')) return;
    const player = document.querySelector('.html5-video-player');
    const skip = document.querySelector('.ytp-ad-skip-button, .ytp-ad-skip-button-modern, .ytp-skip-ad-button');
    if (skip) skip.click();
    if (player && player.classList.contains('ad-showing')) {
      const v = player.querySelector('video');
      if (v && isFinite(v.duration) && v.duration > 0) { v.muted = true; v.currentTime = v.duration; }
    }
  };

  // --- SponsorBlock: fetch community-marked segments for the current video and skip them. ---
  const CATS = ['sponsor', 'selfpromo', 'interaction', 'intro', 'outro', 'music_offtopic'];
  let segVideo = null, segments = [];
  const videoId = () => new URL(location.href).searchParams.get('v') || (location.pathname.match(/\/shorts\/([\w-]{11})/) || [])[1] || null;
  const loadSegments = async id => {
    segVideo = id; segments = [];
    if (!id) return;
    try {
      const r = await fetch(`https://sponsor.ajay.app/api/skipSegments?videoID=${id}&categories=${encodeURIComponent(JSON.stringify(CATS))}`);
      if (r.ok && segVideo === id) segments = (await r.json()).map(s => s.segment);
    } catch {}
  };
  let toast;
  const say = text => {
    if (!toast) {
      toast = document.createElement('div');
      toast.style.cssText = 'position:fixed;left:50%;bottom:90px;transform:translateX(-50%);z-index:999999;background:#e11d2a;color:#fff;' +
        'padding:8px 14px;border-radius:999px;font:600 13px system-ui,sans-serif;pointer-events:none;transition:opacity .3s;opacity:0';
      document.documentElement.appendChild(toast);
    }
    toast.textContent = text; toast.style.opacity = 1;
    clearTimeout(toast.t); toast.t = setTimeout(() => toast.style.opacity = 0, 1800);
  };
  const skipSponsor = () => {
    if (!on('sponsorblock')) return;
    const id = videoId();
    if (id !== segVideo) loadSegments(id);
    const v = document.querySelector('video');
    if (!v || !segments.length) return;
    for (const [a, b] of segments) {
      if (v.currentTime >= a && v.currentTime < b - 0.3) { v.currentTime = b; say('Skipped a sponsor segment'); break; }
    }
  };

  setInterval(() => { syncStyle(); skipAd(); skipSponsor(); }, 300);

  // --- Mini player (called from the top bar). ---
  window.__scarletPip = async () => {
    const v = [...document.querySelectorAll('video')].find(x => x.readyState > 0 && x.offsetWidth > 0) || document.querySelector('video');
    if (!v) { say('Play a video first'); return; }
    try {
      if (document.pictureInPictureElement) await document.exitPictureInPicture();
      else if (v.requestPictureInPicture) await v.requestPictureInPicture();
      else if (v.webkitSetPresentationMode) v.webkitSetPresentationMode(v.webkitPresentationMode === 'picture-in-picture' ? 'inline' : 'picture-in-picture');
    } catch (e) { say('Mini player not available here'); }
  };
})();
