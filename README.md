# Rust 단일 언어 AI 개발 환경

R02의 00·01 단계와 02·05 단계 일부를 구현하는 초기 작업 공간이다. 현재 실행 진입점은 합성 결과 전용 Rust MCP 서버다. [작업 상태와 설계 근거](WORKLOG.md)를 먼저 읽으면 동일한 인계 요청에서 작업을 이어갈 수 있다.

## 빌드와 검증

Rustup과 Git이 필요하다. 고정 Rust 버전과 rustfmt는 `rust-toolchain.toml`에서 설치한다. Windows는 MSVC C 빌드 도구를, Linux는 C 컴파일러와 테스트 클라이언트용 OpenSSL 개발 헤더를 사용한다. SQLite는 서버 바이너리에 포함된다.

```text
cargo fmt --all -- --check
cargo build --workspace --locked
cargo test --workspace --locked
cargo test --workspace --locked --test mcp_http local_mcp_round_trip_and_restart -- --nocapture
```

마지막 명령은 임시 SQLite 파일과 임의의 루프백 포트에서 실제 서버 프로세스를 띄운다. Rust MCP SDK로 연결·도구 탐색·호출을 수행하고 서버를 종료한 뒤 새 프로세스에서 동일 영수증을 재조회한다. 시험이 생성한 자격은 프로세스 환경에만 전달한다.

## 서버 실행

`MCP_COLLECTOR_TOKEN`과 `MCP_READER_TOKEN`을 호스트 비밀 저장소에서 실행 환경으로 주입한 뒤 실행한다. 두 값은 각각 32바이트 무작위 값을 64자리 16진수로 인코딩해 따로 생성한다.

```text
cargo run --locked -p synthetic-mcp-server
```

기본 MCP 주소는 `http://127.0.0.1:8787/mcp`다. 서버는 준비되면 실제 수신 주소를 stdout에 쓰고, 저장·멱등 재호출·조회 이벤트의 프로젝트 ID, 요청 ID와 영수증 ID를 JSON 한 줄씩 stderr에 쓴다.

| 환경 변수 | 용도 |
|---|---|
| `MCP_COLLECTOR_TOKEN` | 필수. 해당 시험 프로젝트의 저장·조회 Bearer 토큰 |
| `MCP_READER_TOKEN` | 필수. 해당 시험 프로젝트의 조회 Bearer 토큰 |
| `MCP_BIND` | 서버의 루프백 주소와 포트. 기본 `127.0.0.1:8787` |
| `MCP_PROJECT` | 두 자격의 프로젝트 범위. 기본 `synthetic-init` |
| `MCP_DB_PATH` | SQLite 파일. 기본 `data/results.sqlite3` |
| `MCP_ALLOWED_HOST` | 리버스 프록시가 보존하는 추가 Host 이름. 로컬 시험의 기본 허용 목록은 `localhost`, `127.0.0.1`, `::1` |
| `MCP_TEST_URL` | 원격 Rust 클라이언트 시험에 사용할 HTTPS MCP URL |

## 호출 계약

Streamable HTTP의 `/mcp`에 `Authorization: Bearer <token>`을 보낸다. 로컬 시험에서 MCP 2025-11-25 초기화와 2026-07-28 발견 절차를 검증했다. 각 HTTP 요청의 자격으로 프로젝트와 역할을 결정한다. 브라우저가 보낸 `Origin` 헤더는 거부한다. 채팅 연결은 서버 간 요청으로 처리한다.

| 도구 | 입력 | 권한 | 성공 응답 |
|---|---|---|---|
| `server_status` | 빈 객체 | Collector, Reader | `status=ready`, `project_id`, `role` |
| `record_result` | `project_id`, `request_id`, `result` | Collector | `status=stored\|replayed`, `receipt` |
| `get_result` | `project_id`, `request_id` | Collector, Reader | `status=found`, `receipt` |

프로젝트 ID와 요청 ID는 1–64자의 영문자, 숫자, 하이픈, 밑줄을 사용한다. 결과는 `[SYNTHETIC]`으로 시작하는 UTF-8 문자열이며 최대 4,096바이트다. 클라이언트가 업무 요청 ID를 최초 호출 전에 만들고, 같은 작업을 재시도할 때 그대로 사용한다. HTTP의 JSON-RPC 호출 ID는 전송 상관관계에 사용한다. 영수증에는 프로젝트 ID, 업무 요청 ID, 서버가 생성한 `receipt_id`, 저장한 문자열이 있다. 두 역할에 동일한 도구 목록을 제공하며 실행 시 자격을 확인한다.

| 오류 | 조건 |
|---|---|
| HTTP 401 | Bearer 자격 누락 또는 불일치 |
| HTTP 403 | 허용 범위를 벗어난 Host 또는 Origin |
| MCP 인자 오류 | 필수 필드 누락, 타입 불일치, 미정의 필드 등 SDK 계약 위반 |
| `FORBIDDEN` | 다른 프로젝트에 접근하거나 Reader로 저장 시도 |
| `INVALID_INPUT` | 요청 ID 형식 오류, 합성 접두어 누락, 결과 크기 초과 |
| `NOT_FOUND` | 프로젝트·요청 키에 해당하는 결과 없음 |
| `CONFLICT` | 이미 저장된 프로젝트·요청 키에 다른 결과 제출 |
| `STORAGE_ERROR` | SQLite 작업 또는 저장 작업 실행 실패. 같은 키로 재조회한 뒤 재시도한다. |

## 외부 시험과 ChatGPT 연결

**INIT-04A** 지정된 시험 호스트에서 서버를 루프백으로 실행하고 HTTPS 프록시로 `/mcp`를 연결한다. 원격 SDK 시험 경로는 Bearer 헤더를 서버에 전달해야 한다. 프록시가 외부 Host를 보존할 경우 `MCP_ALLOWED_HOST`에 그 호스트 이름을 지정한다. 다른 컴퓨터에서 `MCP_TEST_URL`과 두 시험 자격을 설정한 뒤 다음 명령을 실행한다.

```text
cargo test --workspace --locked --test mcp_http remote_mcp_round_trip -- --ignored --nocapture
```

**INIT-04B** ChatGPT 웹의 실제 연결 양식에서 OAuth와 인증 없음 옵션을 확인했다. 이 Rust 서버는 정적 Bearer 자격을 받으므로, 공개 HTTPS로 연결할 경우 선택한 호스트에 ChatGPT용 OAuth 인증 경계를 추가한다. 그 경계가 ChatGPT의 토큰을 검증한 뒤 허용된 프로젝트의 Collector 자격으로 전달해야 한다. Secure MCP Tunnel을 사용할 경우 선택한 워크스페이스의 터널 접근 권한을 확인하고, 터널 전용 내부 경로에서 해당 자격을 주입한다. 호스트 쪽 프록시 및 터널 구성은 INIT-04의 남은 작업이다.

1. ChatGPT 웹의 **플러그인 → 추가 → 사용자 지정 MCP 서버 추가**로 이동한다.
2. HTTPS의 전체 `/mcp` 주소 또는 사용 가능한 Secure MCP Tunnel을 선택한다. 호스트 구성에 맞는 인증을 선택한다. 사용자 계정에서 필요한 OAuth 로그인, 연결 확인과 쓰기 승인을 직접 완료한다.
3. 도구 세 개의 발견을 확인한다. 시험 채팅에서 플러그인을 선택해 `server_status`를 호출한다. 해당 프로젝트로 고유한 `request_id`와 `[SYNTHETIC] ChatGPT 왕복 시험`을 사용해 `record_result`를 호출한 뒤 같은 ID를 `get_result`로 조회한다.
4. 채팅 도구 입력의 요청 ID, 서버의 `stored` 및 `found` 이벤트, 도구 응답의 영수증 ID와 본문을 대조한다. 결과와 승인 상태를 작업 원장에 추가한다.

[OpenAI 연결·시험 가이드](https://developers.openai.com/plugins/deploy/connect-chatgpt) · [ChatGPT 워크스페이스 권한 및 연결](https://help.openai.com/en/articles/12584461-connecting-custom-mcp-servers-in-chatgpt) · [Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels)
