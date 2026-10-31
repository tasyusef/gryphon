// Loads the Leptos app. Trunk's own inline loader is off because the CSP forbids inline
// scripts (Trunk.toml).
import init from './gryphon-ui.js'

// macOS WebKit (the Tauri build there) gets Toolbox's corrections in webkit/ for the ways it
// sets the bitmap faces differently from Chromium.
const ua = navigator.userAgent
if (/\bMacintosh\b/.test(ua) && /\bAppleWebKit\//.test(ua) && !/\b(Chrome|Chromium|Edg|CriOS|FxiOS)\//.test(ua)) {
  const link = Object.assign(document.createElement('link'), { rel: 'stylesheet', href: '/webkit/webkit.css' })
  await new Promise((done) => {
    link.onload = link.onerror = done
    document.head.append(link)
  })
}

await init({ module_or_path: new URL('./gryphon-ui_bg.wasm', import.meta.url) })
