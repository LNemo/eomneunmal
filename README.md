# 없는말

**없는말**은 macOS와 Windows에서 카카오톡/디스코드 메시지를 보낸 직후, 캐릭터가 튀어나와 한국어 맞춤법·띄어쓰기·표기 오류를 비꼬듯 지적하는 경험을 목표로 하는 데스크톱 앱 프로토타입입니다. 현재 저장소는 macOS KakaoTalk 나와의 채팅에서 전송 직후 오버레이 표시를 `pass`로 검증했으며, Discord와 Windows 지원은 아직 검증 전입니다.

> ⚠️ **욕설 포함 주의**  
> 비꼼 강도를 높이면 욕설, 직접적인 사용자 공격, 모욕적인 농담이 표시될 수 있습니다. 기본값은 약한 농담 톤이며, 강한 모드는 사용자가 직접 켰을 때만 동작해야 합니다.

## MVP 목표

없는말 MVP는 “모든 앱을 완벽히 지원하는 범용 키보드 앱”이 아니라 다음 루프를 먼저 증명합니다.

1. 카카오톡 또는 디스코드에서 한국어 메시지를 입력한다.
2. 메시지를 실제로 보낸 직후 앱이 전송 이벤트와 후보 텍스트를 감지한다.
3. 민감 입력이 아니라고 판단될 때만 LLM에 맞춤법 검사를 요청한다.
4. 보낸 메시지는 수정·회수·차단하지 않고, 별도 오버레이 캐릭터가 교정과 비꼼 코멘트를 표시한다.

## 핵심 원칙

- **전송 후 피드백만 제공**: 보낸 메시지를 수정, 삭제, 회수, 차단하지 않습니다.
- **민감 입력은 실패 시 차단**: 아이디, 비밀번호, 결제/카드, 보안코드처럼 민감할 수 있는 입력은 자동 제외합니다. 민감 여부를 확신할 수 없으면 캡처와 LLM 전송을 하지 않습니다.
- **원문 장기 저장 금지**: 기본 설정에서 원문 메시지를 파일, 로그, 진단 번들에 저장하지 않습니다.
- **공식 API 키 우선**: 공식 LLM API 키 사용을 기본 경로로 둡니다.
- **실험적 BYO OAuth는 별도 옵트인**: OpenCode 스타일의 사용자 소유 계정 연결 아이디어는 실험 기능으로만 다루며, 기본 무료 우회 경로·토큰 재판매·공유 프록시로 만들지 않습니다.

## 지원 대상 우선순위

| 우선순위 | 범위 | 상태 |
|---|---|---|
| 1 | macOS + Discord 전송 직후 피드백 | `partial`: 설치/AX inventory + simulated adapter→pipeline→overlay proof, 실제 전송 pass 없음 |
| 2 | macOS + KakaoTalk 전송 직후 피드백 | `pass`: 나와의 채팅 safe self-chat에서 실제 전송 직후 오버레이 표시 확인 |
| 3 | Windows + Discord 전송 직후 피드백 | `blocked`: 현재 호스트가 Windows가 아님 |
| 4 | Windows + KakaoTalk 전송 직후 피드백 | `blocked`: 현재 호스트가 Windows가 아님 |

자세한 기록은 [`docs/compatibility-matrix.md`](docs/compatibility-matrix.md)에 남깁니다.

## 권한 안내

없는말은 OS 수준 입력/접근성 API를 사용해야 하므로 설치 후 권한 안내가 필요합니다.

### macOS

- **손쉬운 사용(Accessibility)**: 현재 포커스된 텍스트 요소와 앱/창 메타데이터 확인에 필요합니다.
- **입력 모니터링(Input Monitoring)**: Enter 키 이벤트 tap 기반 감지 정확도를 높일 때 필요할 수 있습니다. 현재 KakaoTalk live path는 Accessibility로 실제 메시지 입력 필드(`AXDescription=메시지 입력`)와 채팅 로그 구조 변화, 또는 Enter key-down 신호를 함께 확인합니다.

### Windows

- **저수준 키보드 이벤트(WH_KEYBOARD_LL)**: Enter 같은 전송 후보 이벤트 감지에 사용합니다.
- **UI Automation**: 포커스된 텍스트 요소, `IsPassword` 같은 보호 필드 신호, 앱/창 메타데이터 확인에 사용합니다.

## 맞춤법/비꼼 설정

| 설정 | 약함 | 보통 | 강함 |
|---|---|---|---|
| 맞춤법 강도 | 꼭 틀리면 안 되는 오류 위주 | 일반 맞춤법/띄어쓰기 | 띄어쓰기, 외래어 표기 등 가능한 한 엄격하게 |
| 비꼼 강도 | 가벼운 농담 | 선명한 놀림 | 욕설/직접 모욕 포함 가능 |

## LLM 제공자와 인증

- 기본 경로는 공식 API 키입니다.
- API 키와 토큰은 OS 보안 저장소(예: macOS Keychain, Windows Credential Manager)에 저장하는 구조를 목표로 합니다.
- 실험적 BYO OAuth는 사용자가 직접 소유한 계정을 연결하는 옵트인 기능으로만 설계합니다. 기본값은 꺼짐이며, 번들 클라이언트 시크릿·토큰 스크래핑·공유 토큰 프록시를 포함하지 않습니다.

자세한 설계는 [`docs/provider-auth.md`](docs/provider-auth.md)를 참고하세요.


## 현재 호환성 상태

2026-06-27 기준 이 저장소는 macOS KakaoTalk 나와의 채팅에서 실제 전송 직후 오버레이 표시를 `pass`로 기록했습니다.

- macOS Discord는 simulated adapter decision이 Rust pipeline과 overlay까지 연결되는 것을 테스트로 증명했습니다.
- macOS KakaoTalk는 direct Accessibility snapshot, 실제 메시지 입력 필드 판별, Enter/채팅 로그 구조 변화 기반 post-send detector, mock provider, overlay 표시까지 safe self-chat에서 확인했습니다.
- 실제 Discord 메시지 전송, secure-field live exclusion, Windows host 검증은 아직 수동 검증 전입니다.
- 검증된 범위 밖에서는 README/릴리스 노트/앱 UI에서 “지원 완료”라고 표현하지 않습니다.

자세한 evidence는 [`docs/compatibility-matrix.md`](docs/compatibility-matrix.md)에 남깁니다.

## 개발 상태

현재 저장소는 **Tauri 2 runtime + Rust privacy core + macOS KakaoTalk live watcher + overlay local proof** 단계입니다.

구현된 주요 항목:

1. Tauri 2 앱 셸, 설정 UI, Rust settings bridge
2. 민감 입력 제외, 후보 텍스트 생명주기, 프롬프트/제공자 추상화
3. non-focusable overlay window와 300ms shell render target 테스트
4. macOS direct AX focused-text snapshot + KakaoTalk Enter/채팅 로그 구조 변화 기반 live detector
5. macOS Discord/KakaoTalk adapter contract + in-memory fallback tests
6. simulated/live adapter → mock provider → overlay 통합 테스트

남은 핵심 작업:

1. 안전한 테스트 채널에서 실제 Discord post-send pass row 확보
2. macOS secure-field live 검증
3. Windows host에서 WH_KEYBOARD_LL + UI Automation 검증
4. official LLM provider 네트워크 smoke test와 패키징/서명

## 문서

- [아키텍처](docs/architecture.md)
- [프라이버시 정책/설계](docs/privacy.md)
- [제공자 인증 설계](docs/provider-auth.md)
- [호환성 매트릭스](docs/compatibility-matrix.md)

## 공개 저장소 주의

이 저장소에는 API 키, OAuth 토큰, 원문 메시지 로그, 실제 결제/비밀번호/개인 대화 샘플을 커밋하지 않습니다.
