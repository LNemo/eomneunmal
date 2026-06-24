export const DEFAULT_SETTINGS = Object.freeze({
  spellingStrength: 'medium',
  sarcasmStrength: 'weak',
  provider: 'mock',
  experimentalByoOAuthEnabled: false,
  appTargeting: Object.freeze({ discord: true, kakaotalk: true }),
  privacy: Object.freeze({ failClosedUnknownContexts: true, redactDiagnostics: true, persistRawText: false }),
  overlayAutoDismissMs: 8000
});

export function createSettingsState(overrides = {}) {
  return {
    ...DEFAULT_SETTINGS,
    ...overrides,
    appTargeting: { ...DEFAULT_SETTINGS.appTargeting, ...(overrides.appTargeting ?? {}) },
    privacy: { ...DEFAULT_SETTINGS.privacy, ...(overrides.privacy ?? {}) }
  };
}

export function validateSettings(settings) {
  const errors = [];
  if (settings.provider === 'experimental-byo-oauth' && !settings.experimentalByoOAuthEnabled) {
    errors.push('실험적 BYO OAuth는 직접 옵트인이 필요합니다.');
  }
  if (!settings.privacy.failClosedUnknownContexts) {
    errors.push('알 수 없는 입력은 반드시 제외해야 합니다.');
  }
  if (!settings.privacy.redactDiagnostics) {
    errors.push('진단 정보 redaction은 기본적으로 켜져 있어야 합니다.');
  }
  if (settings.privacy.persistRawText) {
    errors.push('MVP에서는 원문 메시지 저장을 허용하지 않습니다.');
  }
  if (!settings.appTargeting.discord && !settings.appTargeting.kakaotalk) {
    errors.push('최소 하나의 MVP 대상 앱을 켜야 합니다.');
  }
  return { ok: errors.length === 0, errors };
}

export function createOverlayState(now = performance.now()) {
  return {
    phase: 'idle',
    detectedAt: null,
    renderedAt: null,
    renderDeadlineMs: 300,
    lastRenderWithinTarget: null,
    createdAt: now
  };
}

export function onSendDetected(state, detectedAt = performance.now()) {
  return {
    ...state,
    phase: 'loading',
    detectedAt,
    renderedAt: null,
    lastRenderWithinTarget: null
  };
}

export function markOverlayRendered(state, renderedAt = performance.now()) {
  if (state.phase !== 'loading' || state.detectedAt === null) {
    return { ...state, lastRenderWithinTarget: false };
  }
  const within = renderedAt - state.detectedAt <= state.renderDeadlineMs;
  return {
    ...state,
    phase: 'rendered-loading',
    renderedAt,
    lastRenderWithinTarget: within
  };
}

export function applyProviderResult(state, result, renderedAt = performance.now()) {
  return {
    ...state,
    phase: 'result',
    renderedAt,
    result
  };
}
