# 없는말 아키텍처

없는말 MVP는 **Tauri 2 + Rust core + OS-native adapter** 구조를 기본으로 합니다. UI는 얇게 유지하고, 입력 감지·민감 정보 판단·제공자 인증·원문 수명 관리는 Rust/native 계층에 둡니다.

## 상위 데이터 흐름

```text
KakaoTalk/Discord 입력창
  └─ OS adapter
      ├─ send-like event 감지(Enter/전송 후보)
      ├─ foreground app/window/focused element 메타데이터
      └─ candidate text 추출 또는 짧은 in-memory buffer
          ↓
Core privacy pipeline
  ├─ sensitive classifier: allow / exclude / manual-review
  ├─ candidate lifecycle: timeout, 완료, 앱 변경, 비활성화 시 즉시 삭제
  ├─ prompt builder: 맞춤법 강도 + 비꼼 강도
  └─ provider abstraction: mock / official API key / experimental BYO OAuth boundary
          ↓
Tauri command boundary
          ↓
Overlay UI
  ├─ loading: 전송 감지 후 300ms 이내 로컬 표시 목표
  ├─ result: 교정/설명/비꼼 멘트
  └─ dismiss: 수동 닫기 또는 자동 닫기
```

## 비목표와 경계

- 보낸 메시지를 수정, 삭제, 회수, 차단하지 않습니다.
- WebView/프론트엔드는 광범위한 입력 권한, API 키, OAuth 토큰, 원문 로그를 직접 다루지 않습니다.
- 키코드만으로 한국어 IME 조합 문자열을 복원하는 방식을 주 경로로 삼지 않습니다.
- 카카오톡/디스코드 서비스 API 봇이 아니라 로컬 데스크톱 오버레이입니다.

## 모듈 계획

| 영역 | 예정 경로 | 책임 |
|---|---|---|
| Tauri 앱 | `src-tauri/` | 앱 셸, commands, capabilities, native 연결 |
| Core | `src-tauri/src/core/` | 후보 텍스트 생명주기, 민감 입력 분류, 프롬프트, 제공자 추상화 |
| macOS adapter | `src-tauri/src/platform/macos/` | Input Monitoring/CGEventTap, Accessibility/AXUIElement probe |
| Windows adapter | `src-tauri/src/platform/windows/` | WH_KEYBOARD_LL, UI Automation TextPattern/ValuePattern/IsPassword probe |
| UI | `src/` | 설정 화면, 캐릭터 오버레이, 권한 안내 |
| Fixtures | `tests/fixtures/` | 원문 없는 redacted 메타데이터 fixture |
| Docs | `docs/` | 프라이버시, 인증, 호환성 매트릭스 |

## Platform adapter contract

각 OS adapter는 공통 인터페이스 뒤에 숨깁니다.

| 함수 | 반환/역할 |
|---|---|
| `permission_state()` | `ready`, `setup_required`, `blocked` |
| `foreground_context()` | 앱 id, 창 제목 hash/category, 포커스 요소 메타데이터 |
| `candidate_text()` | 접근성/UIA 기반 텍스트 또는 허용된 in-memory 후보 |
| `sensitivity_decision()` | `allow`, `exclude`, `manual_review`와 이유 |
| `send_event_stream()` | non-blocking Enter/send-like 이벤트 스트림 |

## 민감 정보 경계

1. OS가 password/protected 필드라고 알려주면 즉시 제외합니다.
2. label, placeholder, role, control type, URL/app context, 사용자 denylist가 ID/결제/보안코드 가능성을 보이면 제외 또는 수동 검토로 둡니다.
3. `manual_review` 또는 알 수 없음은 MVP에서 LLM 전송 금지로 처리합니다.
4. 진단 로그는 앱 id, 권한 상태, adapter 결정, 시간 정보처럼 redacted metadata만 남깁니다.

## 제공자 경계

- `Provider` 인터페이스는 `CritiqueRequest`를 받아 `CritiqueResult`를 반환합니다.
- 공식 API 키 제공자가 기본 구현입니다.
- 실험적 BYO OAuth connector는 별도 feature flag와 secret namespace를 사용합니다.
- 모든 secret은 OS 보안 저장소 경계를 통해 읽고 WebView로 전달하지 않습니다.

## Overlay 성능 목표

- 전송 후보 이벤트 timestamp부터 overlay shell/render까지 **300ms 이하**를 목표로 합니다.
- LLM 응답은 비동기 업데이트로 다루며, provider timeout과 취소 경로를 별도로 둡니다.

## Go/No-Go checkpoint

플랫폼 스파이크 이후 다음 조건을 만족해야 broad UX/provider polish로 넘어갑니다.

- 최소 하나의 OS/app 조합에서 카카오톡 또는 디스코드 전송 직후 후보 텍스트 획득 재현
- 해당 조합에서 민감 입력 제외 fail-closed 확인
- 네 가지 OS/app 조합을 `docs/compatibility-matrix.md`에 evidence와 함께 기록
