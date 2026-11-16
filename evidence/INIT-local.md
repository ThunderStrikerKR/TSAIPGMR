# INIT-01~05 초기 검증 증거

검증일 2026-11-17 KST. 검증 대상 구현 커밋은 `d2e26f44bacf9dfdeb7e62fbdbaf49bb7c856dfc`다. 환경은 Windows 10 Home 빌드 19045, x86_64-pc-windows-msvc, Git 2.56.0.windows.4, rustc/Cargo 1.99.1, rustfmt 1.10.0, RMCP 3.10.0이다.

## 로컬 MCP · INIT-03

```text
cargo fmt --all -- --check
cargo build --workspace --locked
cargo test --workspace --locked
cargo test --workspace --locked --test mcp_http -- --nocapture
```

작업 폴더에서 네 명령이 각각 종료 코드 0으로 끝났다. 자동 실행 결과는 로컬 통합 시험 1건 통과, 원격 시험 1건 제외다. Rust SDK 클라이언트가 Streamable HTTP로 실제 서버 프로세스에 연결했다. SQLite 파일과 자격은 이 시험에서 생성했다.

| 확인 항목 | 관측 결과 |
|---|---|
| 연결·도구 탐색 | MCP 2025-11-25 초기화 후 `server_status`, `record_result`, `get_result` 모두 발견. Collector와 Reader 역할 표시 확인 |
| 정상 저장·멱등성 | 같은 키와 본문을 동시에 제출해 `stored`, `replayed`가 한 건씩 반환. 요청 ID, 영수증, 한글과 혼합 개행이 포함된 본문 일치 |
| 충돌·입력 | 같은 키의 다른 본문은 `CONFLICT`. 합성 접두어 누락은 `INVALID_INPUT`. 없는 키는 `NOT_FOUND` |
| 권한 | 자격 누락·불일치 HTTP 401, 승인하지 않은 Origin HTTP 403. Reader의 저장 및 다른 프로젝트 접근은 `FORBIDDEN` |
| 재시작·프로젝트 격리 | 첫 서버 프로세스를 종료한 뒤 동일 SQLite 파일을 여는 새 프로세스에서 MCP 2026-07-28 발견 절차와 `found` 응답 확인. 같은 파일을 다른 프로젝트에서 열어 해당 프로젝트 키의 조회는 `NOT_FOUND`, 원래 프로젝트 키 접근은 `FORBIDDEN` 확인 |

로컬 Git 복제본과 빈 빌드 디렉터리를 사용해 같은 커밋을 다시 검사했다. 포맷 검사, 잠금 빌드, `cargo test --workspace --locked -- --nocapture`가 모두 종료 코드 0이었다. 시험은 로컬 1건 통과, 원격 1건 제외였고 검증 뒤 작업 트리는 깨끗했다. 아래는 새 복제본 시험의 로그다.

```jsonl
{"project_id":"synthetic-init","receipt_id":"88787630-2963-4be5-b2a1-4a990b4e16dc","request_id":"74b9cc3f-c611-4a8d-86d9-fed92fa2e592","status":"stored"}
{"project_id":"synthetic-init","receipt_id":"88787630-2963-4be5-b2a1-4a990b4e16dc","request_id":"74b9cc3f-c611-4a8d-86d9-fed92fa2e592","status":"replayed"}
{"project_id":"synthetic-init","receipt_id":"88787630-2963-4be5-b2a1-4a990b4e16dc","request_id":"74b9cc3f-c611-4a8d-86d9-fed92fa2e592","status":"found"}
{"project_id":"synthetic-init","receipt_id":"88787630-2963-4be5-b2a1-4a990b4e16dc","request_id":"74b9cc3f-c611-4a8d-86d9-fed92fa2e592","status":"found"}
```

마지막 `found`는 다른 서버 프로세스에서 수집했다. 개발 중 첫 `cargo build --workspace`에서 RMCP 가져오기 경로가 달라 E0432/E0603으로 INIT-03 빌드가 실패했다. SDK 소스에 맞춰 가져오기 경로를 수정한 뒤 위 명령으로 관련 빌드와 동작을 재검증했다.

## 원격과 호스트 · INIT-04/05

- GitHub 연결 도구로 인증 계정의 저장소 접근과 복제 URL 조회를 확인했다. 신규 프로젝트의 로컬 `git remote -v`는 빈 결과다. GitHub Actions 실행과 실행 URL은 `NOT_RUN`.
- 원격 MCP SDK는 `NOT_RUN`. 지정된 외부 시험 호스트 또는 주소가 생기면 README의 원격 시험 명령을 실행한다.
- 실제 ChatGPT 웹의 `플러그인 → 추가 → 사용자 지정 MCP 서버 추가` 양식에서 서버 URL, OAuth, 인증 없음, OAuth 또는 인증 없음 선택을 확인했다. 실제 ChatGPT의 도구 검색과 호출은 `BLOCKED`. 마지막 확인 단계는 연결 양식 열기다. 시험 주소 또는 Secure MCP Tunnel 및 워크스페이스 권한을 준비한 뒤 등록과 필요한 사용자 승인을 거쳐 도구 발견에서 이어간다.
