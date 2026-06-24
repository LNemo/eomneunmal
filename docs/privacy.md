# 프라이버시 설계

없는말은 넓은 입력 감지 권한을 요구할 수 있으므로, MVP부터 **최소 수집·짧은 보관·실패 시 차단**을 기본 원칙으로 둡니다.

## 수집하는 정보

| 종류 | 용도 | 기본 보관 |
|---|---|---|
| 후보 메시지 텍스트 | 전송 직후 맞춤법 검사 요청 | 메모리에서만 보관, 처리 후 삭제 |
| 앱/창/포커스 메타데이터 | 카카오톡/디스코드 여부 및 민감 입력 판단 | redacted/hash 형태 진단 가능 |
| 권한 상태 | 설정 안내와 문제 해결 | 원문 텍스트 없음 |
| provider 상태/오류 | 네트워크/인증 문제 표시 | 원문 텍스트 없음 |

## 수집하지 않는 정보

- 비밀번호, 카드번호, CVC, OTP, 보안질문, 결제 폼 내용
- 원문 대화 로그 파일
- API 키 또는 OAuth 토큰 plaintext 설정 파일
- unrelated foreground text 전체 덤프

## 민감 입력 제외 규칙

다음 신호 중 하나라도 있으면 캡처와 LLM 전송을 중단합니다.

- macOS secure text field 또는 접근성 password/protected 유사 신호
- Windows UI Automation `IsPassword` 또는 보호 필드 신호
- label/placeholder/title에 다음 의미가 포함됨: 비밀번호, 암호, password, passcode, 카드, card, cvc, cvv, 결제, payment, 주민, 계좌, OTP, 보안코드, 로그인 ID 등
- 앱/창/URL context가 결제·로그인·계정 복구 흐름으로 보임
- adapter가 포커스 요소의 성격을 판별하지 못함

MVP에서 `manual_review`나 `unknown`은 **exclude**와 동일하게 처리합니다.

## 후보 텍스트 생명주기

- 후보 텍스트는 전송 이벤트 처리에 필요한 짧은 시간 동안만 메모리에 둡니다.
- 처리 완료, provider timeout, 앱 전환, 설정 비활성화, 민감 판단 발생 시 즉시 삭제합니다.
- crash report, debug log, telemetry, persisted setting에 원문을 쓰지 않습니다.

## LLM 전송 정책

LLM 요청에는 다음만 포함합니다.

- 사용자가 방금 보낸 후보 메시지
- 맞춤법 강도
- 비꼼 강도
- 한국어 교정에 필요한 최소 지시문

다음은 보내지 않습니다.

- 주변 대화 전체
- 창 제목 원문
- 계정 식별자
- API 키/OAuth 토큰
- OS 사용자명/로컬 파일 경로

## 진단 번들

향후 “진단 번들 복사” 기능은 다음만 포함해야 합니다.

- OS/app 버전
- 권한 상태
- adapter 결정 코드
- send event timestamp와 overlay timestamp
- provider status/error class
- window title hash/category

원문 메시지와 secret은 포함하지 않습니다.

## 사용자 제어

- 앱별 허용/차단 목록
- 맞춤법 검사 일시 중지
- provider 비활성화
- 강한 비꼼 모드 끄기
- 로컬 진단 삭제

## 공개 저장소 원칙

테스트 fixture는 실제 사용자 대화가 아니라 redacted metadata만 사용합니다. 실제 API 키, OAuth 토큰, 결제/비밀번호 예시는 커밋하지 않습니다.
