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

function renderSettingsStatus() {
  const result = validateSettings(readSettings());
  $('settingsStatus').textContent = result.ok ? '설정이 MVP privacy boundary를 만족합니다.' : result.errors.join(' / ');
  $('settingsStatus').className = result.ok ? 'ok' : 'error';
}

function renderOverlay(title, body, phase) {
  $('overlayTitle').textContent = title;
  $('overlayBody').textContent = body;
  $('overlay').className = `overlay ${phase}`;
}

function simulateSend() {
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

for (const id of ['spellingStrength', 'sarcasmStrength', 'provider', 'discordEnabled', 'kakaoEnabled', 'failClosed', 'redactDiagnostics', 'byoOptIn']) {
  $(id).addEventListener('change', renderSettingsStatus);
}
$('simulateSend').addEventListener('click', simulateSend);
renderSettingsStatus();
