# 호환성 매트릭스

이 문서는 카카오톡/디스코드 전송 직후 감지 가능성과 민감 입력 제외 근거를 기록하는 1급 산출물입니다. MVP 구현 중 플랫폼 스파이크 결과를 이 표에 계속 갱신합니다.

## 상태 범례

| 상태 | 의미 |
|---|---|
| `planned` | 아직 스파이크 전 |
| `pass` | 재현 가능한 전송 직후 피드백 경로 확인 |
| `partial` | 이벤트, 설치, 권한, 텍스트 획득 중 일부만 확인 |
| `blocked` | OS/app/test-environment 제약으로 현재 접근 불가 |
| `disabled` | 민감 입력 fail-closed 미충족으로 비활성화 |

## 2026-06-25 로컬 스파이크 결과

Evidence command:

```sh
(cd src-tauri && cargo run --bin probe_matrix > ../.omx/ultragoal/probe-matrix-20260625.md)
(cd src-tauri && cargo test)
npm test
npm run build
```

Current host:

- OS: macOS 27.0, build 26A5353q, arm64
- Discord: `/Applications/Discord.app`, bundle `com.hnc.Discord`, version `0.0.389`
- KakaoTalk: `/Applications/KakaoTalk.app`, bundle `com.kakao.KakaoTalkMac`, version `26.1.2`
- Accessibility probe: `System Events UI elements enabled = true`
- Input Monitoring: manual verification required; non-interactive probe cannot prove it
- Raw text capture: not performed
- External message send: not performed automatically because it would send a real message outside the repo/test harness

| OS | App | OS/App version | Permissions | Input method | Send signal | Text acquisition method | Sensitive-exclusion result | Status | Evidence notes |
|---|---|---|---|---|---|---|---|---|---|
| macOS | Discord | 27.0 / 0.0.389 | unknown | Korean IME | Enter/send-button candidate; actual external send not automated | Accessibility/AX focused text first; in-memory candidate fallback | core classifier harness pass; secure-field live probe pending | partial | bundle com.hnc.Discord installed; AX=ready; InputMonitoring=manual; no raw text captured; no external message sent |
| macOS | KakaoTalk | 27.0 / 26.1.2 | unknown | Korean IME | Enter/send-button candidate; actual external send not automated | Accessibility/AX focused text first; in-memory candidate fallback | core classifier harness pass; secure-field live probe pending | partial | bundle com.kakao.KakaoTalkMac installed; AX=ready; InputMonitoring=manual; no raw text captured; no external message sent |
| Windows | Discord | not-current-host / TBD on Windows host | not-current-host | Korean IME | WH_KEYBOARD_LL via SetWindowsHookEx | UI Automation TextPattern/ValuePattern with UI Automation IsPassword property exclusion | contract specified; live Windows secure-field probe pending | blocked | blocked in this session: current host is not Windows |
| Windows | KakaoTalk | not-current-host / TBD on Windows host | not-current-host | Korean IME | WH_KEYBOARD_LL via SetWindowsHookEx | UI Automation TextPattern/ValuePattern with UI Automation IsPassword property exclusion | contract specified; live Windows secure-field probe pending | blocked | blocked in this session: current host is not Windows |

### Simulated adapter integration evidence

이 항목은 실제 Discord 메시지를 보내지 않은 로컬 proof입니다. public support `pass`로 승격하지 않습니다.

| Field | Evidence |
|---|---|
| Adapter | macOS Discord `LivePostSendAdapter` contract |
| Send signal | synthetic/Enter-like event abstraction |
| Candidate source | in-memory fallback candidate, debug redacted |
| Pipeline | adapter decision → `CritiquePipeline<SpyProvider/MockProvider>` → `OverlayController` |
| Provider privacy | provider request contains candidate message only; app/window/channel metadata and secrets are not included |
| Sensitive fail-closed | protected Discord-like field returns excluded; provider and overlay are not called |
| Support claim | `SimulatedAdapterOnly`, not `pass` |
| Test evidence | `cargo test` includes simulated Discord adapter integration and provider-spy privacy tests |

## Formal MVP Go/No-Go 기록

- Decision: `narrow`
- Date: 2026-06-25
- Rationale: Discord/KakaoTalk are installed on the macOS host and Accessibility is observable as enabled. macOS Discord now has a simulated adapter→pipeline→overlay proof with provider-spy privacy tests, but Input Monitoring and actual post-send Korean text availability were not proven. Windows cannot be tested on the current host. Sending a real Discord/KakaoTalk message automatically would be an external side effect without a user-provided safe test channel, so this run must not claim `pass` for any OS/app pair.
- Passing OS/app pair(s): none yet
- Partial OS/app pair(s): macOS Discord, macOS KakaoTalk
- Blocked pair(s): Windows Discord, Windows KakaoTalk on this host
- Sensitive exclusion evidence: Rust core classifier tests pass for password/protected, payment/card, unknown metadata fail-closed, chat allow fixtures; simulated Discord protected-field adapter decision excludes before provider/overlay; live secure-field adapter probe remains pending
- Scoped fallback: continue only as prototype/simulated adapter integration over the stable adapter contract and mock provider. Do not claim public MVP support for actual KakaoTalk/Discord post-send behavior until a manual test channel run records at least one `pass` row.

## 스파이크 기록 템플릿

| Field | Value |
|---|---|
| Date | YYYY-MM-DD |
| Tester | local/dev |
| OS build | TBD |
| App version | TBD |
| Permissions granted | TBD |
| Input method | Korean IME / English / other |
| Message fixture | Redacted typo category only, no raw private text |
| Send signal observed | Enter / button / unknown |
| Text acquisition observed | focused text / candidate buffer / unavailable |
| Sensitive exclusion observed | pass / fail / unknown |
| Overlay shell latency | ms from send event to local render |
| Result | pass / partial / blocked / disabled |
| Evidence path or note | no raw text, redacted screenshot/log only |
