# 호환성 매트릭스

이 문서는 카카오톡/디스코드 전송 직후 감지 가능성과 민감 입력 제외 근거를 기록하는 1급 산출물입니다. MVP 구현 중 플랫폼 스파이크 결과를 이 표에 계속 갱신합니다.

## 상태 범례

| 상태 | 의미 |
|---|---|
| `planned` | 아직 스파이크 전 |
| `pass` | 재현 가능한 전송 직후 피드백 경로 확인 |
| `partial` | 이벤트 또는 텍스트 일부만 확인 |
| `blocked` | OS/app 제약으로 현재 접근 불가 |
| `disabled` | 민감 입력 fail-closed 미충족으로 비활성화 |

## MVP 대상 매트릭스

| OS | App | OS/App version | Permissions | Input method | Send signal | Text acquisition method | Sensitive-exclusion result | Status | Evidence notes |
|---|---|---|---|---|---|---|---|---|---|
| macOS | Discord | TBD | Input Monitoring + Accessibility 필요 | Korean IME | Enter | Accessibility/AX focused text 우선 | TBD | planned | G004 platform spike에서 기록 |
| macOS | KakaoTalk | TBD | Input Monitoring + Accessibility 필요 | Korean IME | Enter 또는 send button | Accessibility/AX focused text 우선 | TBD | planned | G004 platform spike에서 기록 |
| Windows | Discord | TBD | WH_KEYBOARD_LL + UI Automation 필요 | Korean IME | Enter | UIA TextPattern/ValuePattern 우선 | TBD | planned | G004 platform spike에서 기록 |
| Windows | KakaoTalk | TBD | WH_KEYBOARD_LL + UI Automation 필요 | Korean IME | Enter 또는 send button | UIA TextPattern/ValuePattern 우선 | TBD | planned | G004 platform spike에서 기록 |

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

## Go/No-Go 기록

플랫폼 스파이크 후 아래 항목을 채웁니다.

- Decision: `go` / `narrow` / `no-go`
- Rationale:
- Passing OS/app pair(s):
- Blocked pair(s):
- Sensitive exclusion evidence:
- Next scope:
