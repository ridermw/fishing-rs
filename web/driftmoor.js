// Small miniquad plugin: save data in localStorage, URL query access, and a JSON status
// snapshot on window.__dm that automated play-tests can read.
miniquad_add_plugin({
  name: "driftmoor",
  version: 1,
  register_plugin: function (importObject) {
    const KEY = "driftmoor-cove-rs-v1";
    const write = (ptr, cap, str) => {
      const bytes = new TextEncoder().encode(str);
      if (bytes.length > cap) return 0;
      new Uint8Array(wasm_memory.buffer, ptr, bytes.length).set(bytes);
      return bytes.length;
    };
    // miniquad requests unsized DEPTH_COMPONENT textures (WebGL1 style); WebGL2 needs a sized format.
    const texImage2D = importObject.env.glTexImage2D;
    importObject.env.glTexImage2D = function (target, level, internalFormat, w, h, border, format, type, pixels) {
      const webgl2 = typeof WebGL2RenderingContext !== "undefined" && gl instanceof WebGL2RenderingContext;
      if (webgl2 && internalFormat === 0x1902) {
        return gl.texImage2D(target, level, 0x81A6 /* DEPTH_COMPONENT24 */, w, h, border, 0x1902, 0x1405 /* UNSIGNED_INT */, null);
      }
      return texImage2D(target, level, internalFormat, w, h, border, format, type, pixels);
    };
    importObject.env.dm_storage_get = (ptr, cap) => {
      try { const s = localStorage.getItem(KEY); return s ? write(ptr, cap, s) : 0; } catch (e) { return 0; }
    };
    importObject.env.dm_storage_set = (ptr, len) => {
      try { localStorage.setItem(KEY, UTF8ToString(ptr, len)); } catch (e) { /* storage unavailable */ }
    };
    importObject.env.dm_query = (ptr, cap) => write(ptr, cap, location.search);
    importObject.env.dm_report = (ptr, len) => {
      try { window.__dm = JSON.parse(UTF8ToString(ptr, len)); } catch (e) { /* ignore */ }
    };
  },
});
