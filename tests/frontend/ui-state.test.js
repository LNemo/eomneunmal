import test from 'node:test';
import assert from 'node:assert/strict';
import {
  createOverlayState,
  createSettingsState,
  validateSettings,
  onSendDetected,
  markOverlayRendered,
  applyProviderResult
} from '../../src/ui-state.js';

test('default settings preserve privacy and enable MVP targets', () => {
  const settings = createSettingsState();
  assert.equal(settings.spellingStrength, 'medium');
  assert.equal(settings.sarcasmStrength, 'weak');
  assert.equal(settings.provider, 'mock');
  assert.equal(settings.experimentalByoOAuthEnabled, false);
  assert.equal(settings.appTargeting.discord, true);
  assert.equal(settings.appTargeting.kakaotalk, true);
  assert.deepEqual(validateSettings(settings), { ok: true, errors: [] });
});

test('experimental BYO OAuth requires explicit opt-in', () => {
  const settings = createSettingsState({ provider: 'experimental-byo-oauth' });
  const result = validateSettings(settings);
  assert.equal(result.ok, false);
  assert.match(result.errors.join('\n'), /옵트인/);
});

test('unknown contexts and raw persistence cannot be enabled', () => {
  const settings = createSettingsState({
    privacy: { failClosedUnknownContexts: false, persistRawText: true }
  });
  const result = validateSettings(settings);
  assert.equal(result.ok, false);
  assert.match(result.errors.join('\n'), /알 수 없는 입력/);
  assert.match(result.errors.join('\n'), /원문 메시지/);
});

test('overlay shell render target is measured at 300ms', () => {
  let state = createOverlayState(0);
  state = onSendDetected(state, 1000);
  state = markOverlayRendered(state, 1299);
  assert.equal(state.phase, 'rendered-loading');
  assert.equal(state.lastRenderWithinTarget, true);

  state = onSendDetected(state, 2000);
  state = markOverlayRendered(state, 2301);
  assert.equal(state.lastRenderWithinTarget, false);
});

test('provider result updates asynchronously after local shell render', () => {
  let state = createOverlayState(0);
  state = onSendDetected(state, 10);
  state = markOverlayRendered(state, 15);
  state = applyProviderResult(state, { corrected: '돼요', roast: '그 정도는 외우자.' }, 800);
  assert.equal(state.phase, 'result');
  assert.equal(state.result.corrected, '돼요');
});
