const $ = (id) => document.getElementById(id);
const tauriEventListen = () => globalThis.__TAURI__?.event?.listen ?? null;
const tauriInvoke = () => globalThis.__TAURI__?.core?.invoke ?? null;

let dismissTimer = null;

function renderOverlayState(state) {
  $('overlayTitle').textContent = state.title;
  $('overlayBody').textContent = state.body;
  $('overlay').className = `overlay overlay-window ${state.phase}`;

  if (dismissTimer !== null) {
    window.clearTimeout(dismissTimer);
    dismissTimer = null;
  }

  if (state.phase === 'result' && state.autoDismissMs > 0) {
    dismissTimer = window.setTimeout(() => {
      const invoke = tauriInvoke();
      if (invoke) {
        void invoke('dismiss_overlay');
      }
    }, state.autoDismissMs);
  }
}

async function startOverlayListener() {
  const listen = tauriEventListen();
  if (!listen) {
    return;
  }
  await listen('overlay://state', (event) => {
    renderOverlayState(event.payload);
  });
}

void startOverlayListener();
