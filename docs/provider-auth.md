# 제공자 인증 설계

없는말은 LLM을 사용해 한국어 맞춤법 교정과 비꼼 멘트를 생성합니다. 인증 경로는 **공식 API 키 우선**, **실험적 사용자 소유 BYO OAuth는 별도 옵트인**으로 분리합니다.

## 제공자 우선순위

| 경로 | 기본값 | 목적 | MVP 정책 |
|---|---:|---|---|
| Mock provider | 켜짐(개발/테스트) | 테스트와 로컬 UI 검증 | secret 필요 없음 |
| Official API key provider | 사용자가 설정 | 실제 LLM 호출 | 기본 production 경로 |
| Experimental BYO OAuth connector | 꺼짐 | 사용자가 직접 소유한 계정 연결 실험 | feature flag + 명시적 경고 필요 |

## 공식 API 키

- 사용자가 직접 발급한 API 키를 입력합니다.
- 키는 OS 보안 저장소에 저장하는 것을 목표로 합니다.
  - macOS: Keychain Services
  - Windows: Credential Manager
- WebView에는 키 원문을 넘기지 않습니다.
- 로그, crash report, diagnostics에는 키 전체 또는 일부를 남기지 않습니다.

## Experimental BYO OAuth boundary

OpenCode류 도구에서 볼 수 있는 “사용자 계정 기반 인증 흐름” 아이디어는 다음 경계 안에서만 다룹니다.

- 사용자가 직접 소유한 계정으로 명시적으로 연결합니다.
- 기본값은 꺼짐입니다.
- “무료 우회”, “공유 토큰”, “다중 사용자 프록시”, “구독 재판매” 기능으로 제공하지 않습니다.
- provider client secret을 저장소에 번들하지 않습니다.
- 브라우저/앱에서 토큰을 훔치거나 scraping하지 않습니다.
- 공식 API 키 경로와 secret namespace를 분리합니다.
- 실험 기능임을 UI와 README에 명확히 표시합니다.

## Secret namespace 계획

| Secret | Namespace 예시 | 노출 대상 |
|---|---|---|
| Official API key | `eomneunmal.provider.openai.api_key` | Rust provider layer only |
| BYO OAuth access token | `eomneunmal.provider.experimental.oauth.access` | Rust provider layer only |
| BYO OAuth refresh token | `eomneunmal.provider.experimental.oauth.refresh` | Rust provider layer only |

## Provider request 최소화

`CritiqueRequest`에는 다음 필드만 허용합니다.

- `message`: 민감 입력이 아니라고 판단된 후보 메시지
- `spelling_strength`: `weak | medium | strong`
- `sarcasm_strength`: `weak | medium | strong`
- `locale`: `ko-KR`

요청에 앱 이름, 창 제목 원문, 사용자 계정 정보, 주변 대화 전체를 넣지 않습니다.

## 테스트 요구사항

- mock secure store가 bearer credential을 제공하는지 검증합니다.
- official API key provider가 secret을 로그로 남기지 않는지 검증합니다.
- BYO OAuth connector는 feature flag off 상태에서 생성/호출되지 않아야 합니다.
- provider timeout/error는 overlay의 사용자 표시 상태로 매핑되어야 합니다.
