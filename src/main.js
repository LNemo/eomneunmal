import {
  createOverlayState,
  createSettingsState,
  validateSettings,
  onSendDetected,
  markOverlayRendered,
  applyProviderResult
} from './ui-state.js';

let overlayState = createOverlayState();

const $ = (id) => document.getElementById(id);
const tauriInvoke = () => globalThis.__TAURI__?.core?.invoke ?? null;

function readSettings() {
  return createSettingsState({
    spellingStrength: $('spellingStrength').value,
    sarcasmStrength: $('sarcasmStrength').value,
    provider: $('provider').value,
    experimentalByoOAuthEnabled: $('byoOptIn').checked,
    appTargeting: {
      discord: $('discordEnabled').checked,
      kakaotalk: $('kakaoEnabled').checked
    },
    privacy: {
      failClosedUnknownContexts: $('failClosed').checked,
      redactDiagnostics: $('redactDiagnostics').checked,
      persistRawText: false
    }
  });
}

function applySettingsToForm(settings) {
  $('spellingStrength').value = settings.spellingStrength;
  $('sarcasmStrength').value = settings.sarcasmStrength;
  $('provider').value = settings.provider;
  $('byoOptIn').checked = settings.experimentalByoOAuthEnabled;
  $('discordEnabled').checked = settings.appTargeting.discord;
  $('kakaoEnabled').checked = settings.appTargeting.kakaotalk;
  $('failClosed').checked = settings.privacy.failClosedUnknownContexts;
  $('redactDiagnostics').checked = settings.privacy.redactDiagnostics;
}

function renderProviderRuntimeNote(settings) {
  $('providerRuntimeNote').textContent = settings.provider === 'mock'
    ? '현재 local proof는 Mock provider만 실행합니다.'
    : '이 provider 설정은 저장·검증만 됩니다. 현재 local proof는 실제 provider 호출 없이 Mock provider만 실행합니다.';
}

async function validateWithRust(settings) {
  const invoke = tauriInvoke();
  if (!invoke) {
    return null;
  }
  return invoke('validate_settings', { settings });
}

async function saveWithRust(settings) {
  const invoke = tauriInvoke();
  if (!invoke) {
    return null;
  }
  return invoke('save_settings', { settings });
}

async function loadSettingsFromRust() {
  const invoke = tauriInvoke();
  if (!invoke) {
    return false;
  }
  const settings = await invoke('get_saved_settings');
  applySettingsToForm(createSettingsState(settings));
  return true;
}

async function renderSettingsStatus() {
  const settings = readSettings();
  const rustResult = await validateWithRust(settings);
  const result = rustResult ?? validateSettings(settings);
  const bridgeText = rustResult ? ' / Rust bridge active' : '';
  $('settingsStatus').textContent = result.ok ? `설정이 MVP privacy boundary를 만족합니다.${bridgeText}` : result.errors.join(' / ');
  $('settingsStatus').className = result.ok ? 'ok' : 'error';
  renderProviderRuntimeNote(settings);
  if (result.ok) {
    await saveWithRust(settings);
  }
}

function renderOverlay(title, body, phase) {
  $('overlayTitle').textContent = title;
  $('overlayBody').textContent = body;
  $('overlay').className = `overlay ${phase}`;
}

function renderOverlayView(view) {
  renderOverlay(view.title, view.body, view.phase);
}

function simulateBrowserPreview() {
  const detectedAt = performance.now();
  overlayState = onSendDetected(overlayState, detectedAt);
  renderOverlay('전송 감지', '없는말이 맞춤법을 씹을 준비 중...', 'loading');

  requestAnimationFrame(() => {
    overlayState = markOverlayRendered(overlayState, performance.now());
    const latencyText = overlayState.lastRenderWithinTarget ? '300ms 목표 안에 표시됨' : '300ms 목표 초과';
    renderOverlay('검사 중', latencyText, 'loading');

    window.setTimeout(() => {
      overlayState = applyProviderResult(overlayState, {
        corrected: '그렇게 하면 돼요.',
        explanation: '되요 → 돼요',
        roast: readSettings().sarcasmStrength === 'strong' ? '돼지가 아니라 되지겠지, 돼지야.' : '세종대왕님이 살짝 한숨 쉬셨다.'
      });
      renderOverlay('맞춤법 지적', `${overlayState.result.explanation} — ${overlayState.result.roast}`, 'result');
    }, 200);
  });
}

async function simulateSend() {
  const invoke = tauriInvoke();
  if (!invoke) {
    simulateBrowserPreview();
    return;
  }

  await renderSettingsStatus();
  try {
    const report = await invoke('simulate_mock_post_send');
    renderOverlayView(report.renderedLoadingView);
    if (report.resultView) {
      renderOverlayView(report.resultView);
    }
  } catch (error) {
    renderOverlay('실행 제외', String(error), 'result');
  }
}

for (const id of ['spellingStrength', 'sarcasmStrength', 'provider', 'discordEnabled', 'kakaoEnabled', 'failClosed', 'redactDiagnostics', 'byoOptIn']) {
  $(id).addEventListener('change', () => { void renderSettingsStatus(); });
}
$('simulateSend').addEventListener('click', simulateSend);
loadSettingsFromRust()
  .catch(() => false)
  .finally(() => { void renderSettingsStatus(); });
