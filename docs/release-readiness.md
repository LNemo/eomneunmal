# 릴리스 준비 상태

작성일: 2026-06-27

## 결론

현재 상태는 **Tauri runtime + privacy core + overlay local proof + macOS KakaoTalk safe self-chat live pass 완료**입니다. macOS KakaoTalk 나와의 채팅 범위는 `pass`로 주장할 수 있지만, Discord/Windows/secure-field live 검증은 아직 완료되지 않았습니다.

이 판단은 `docs/compatibility-matrix.md`의 formal decision `narrow-with-kakao-pass`를 따릅니다.

## 완료된 항목

- 한국어 README와 공개 저장소 안전 문서
- privacy-first Rust core pipeline
- 민감 입력 fail-closed 분류
- 후보 텍스트 메모리 생명주기와 redacted Debug
- 맞춤법 강도/비꼼 강도 prompt builder
- official API key provider blueprint와 secret-store boundary
- experimental BYO OAuth off-by-default boundary
- Tauri 2 설정/권한 스캐폴드
- Tauri 2 runtime entrypoint, settings bridge, redacted diagnostics command
- 설정 UI와 오버레이 미리보기
- non-focusable overlay window, Rust overlay controller, Tauri overlay event bridge
- 300ms overlay shell render target 측정 로직
- macOS/Windows platform probe contract
- macOS Discord live-adapter contract spike with in-memory fallback and protected-field fail-closed tests
- macOS KakaoTalk direct AX focused-text snapshot
- macOS KakaoTalk Enter/채팅 로그 구조 변화 기반 live watcher
- macOS KakaoTalk safe self-chat 실제 전송 직후 overlay 표시 pass (`message_input=true`, `chat_history_hash_present=true` redacted probe 확인)
- simulated macOS Discord adapter decision → mock/spy provider → overlay integration tests
- macOS Discord/KakaoTalk 설치/AX 상태 inventory
- Windows probe design rows and current-host blocker record
- harness/demo integration: post-send candidate → classifier → mock provider → overlay result

## 아직 공개 MVP로 주장하면 안 되는 항목

- 실제 Discord 메시지 전송 직후 텍스트 획득 `pass`
- Windows host에서 WH_KEYBOARD_LL + UI Automation live 검증
- 실제 secure text field에서 OS adapter fail-closed live 검증
- official LLM provider 실제 네트워크 호출 smoke test
- Tauri 앱 패키징/서명/배포

## Overclaim hard rule

- public README, 릴리스 노트, 앱 UI는 `docs/compatibility-matrix.md`에 있는 `pass` row 범위만 지원 완료로 표현합니다.
- simulated adapter/harness 결과는 `partial` 또는 prototype evidence로만 표기합니다.
- raw text, API key, OAuth token, private channel name은 diagnostics/evidence에 남기지 않습니다.

## 안전한 다음 수동 검증 절차

1. 테스트 전용 Discord 서버/채널 또는 KakaoTalk 나와의 채팅을 준비한다.
2. macOS에서 Accessibility 권한을 부여한다.
3. `cd src-tauri && cargo run --bin probe_matrix`로 inventory를 다시 기록한다.
4. 한국어 오타 fixture(예: `그렇게 하면 되요`)를 테스트 채널에 입력하고 전송한다.
5. raw text가 로그/diagnostics에 남지 않는지 확인한다.
6. `docs/compatibility-matrix.md`에 pass/partial/blocked evidence를 갱신한다.

## 릴리스 판단

- macOS KakaoTalk safe self-chat은 prototype `pass`로 표기할 수 있습니다.
- Discord/Windows/secure-field live 검증은 compatibility limitation을 유지해야 합니다.
- 사용자에게 검증 범위 밖의 실제 채팅 앱 지원을 약속하지 않습니다.
