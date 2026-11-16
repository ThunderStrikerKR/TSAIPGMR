# 초기 구현 작업 원장

기준일 2026-11-17 KST. 같은 인계 파일로 재개할 때는 이 원장과 `git status --short --branch`, `git log -3 --oneline`, `git remote -v`를 대조한다. GitHub 저장소는 `ThunderStrikerKR/TSAIPGMR`, 작업 브랜치는 `init/rust-mcp`다. 기존 `main`의 초기 커밋을 기준으로 검증된 로컬 `rust-ai-workspace`의 구현과 증거를 반영했다.

## 계약과 근거

| 상위 폴더 원본 | 확인 범위 | SHA-256 |
|---|---|---|
| CODEX_HANDOFF.md | §1–4. Codex가 담당하는 초기 구현의 계약과 수용 기준 | `844CE783BCD8058A55DA8CF657E3C4AA0A69F0DDC68931EC8067F8AD507B2726` |
| Rust 단일 언어 범용 AI 소프트웨어 개발 환경 - 전체 설계지침.md | R02 §2·6·7·9의 작업, MCP, 증거 요구사항과 단계 00·01 및 02·05 일부 | `DF473E7D6AF7E8447AD99385AFB2B8D2066D686310329C8A87D00DA4B12DAC73` |
| 근거와 미확보 원문 목록.md | §3. A-02의 실제 ChatGPT MCP 왕복과 C17의 범위 한정 원문 저장·재조회 | `2050DBFB940C9715594FCC77A600E1912B27DBBB75B55FBFAC9F3C5DC3492BCA` |

상위 문서 원본은 읽기 전용으로 참조했다. 초기 구현에서 Codex 사용은 `CODEX_HANDOFF.md` §1을 따른다. 새 실행 코드와 제품 시험은 Rust로 작성한다. 현재 범위는 신규 Git·Rust 기반, 합성 MCP 결과의 프로젝트/역할별 저장과 회수, 재개·CI 설정이다. A-02와 C17은 선행 근거로 인용하며 여기서 검증하는 서버는 `synthetic-mcp-server 0.1.0`이다. 제품의 자체 UI, 숨김 브라우저의 일반 응답 전문 회수, 원문 수집·편찬과 독립 의미감사는 후속 단계에 있다.

| 작업 ID | 현재 상태 | 증거와 재개 지점 |
|---|---|---|
| INIT-01 | 로컬 완료 | Windows x86_64 MSVC. Git 2.56.0.windows.4, rustc 1.99.1, Cargo 1.99.1, rustfmt 1.10.0 실행. Rustup으로 고정 1.99.1 도구 체인을 설치했다. GitHub 연결에서 인증 계정과 접근 가능한 저장소 URL을 조회했다. 새 프로젝트의 공개 원격은 INIT-05에 기록했다. |
| INIT-02 | 로컬 완료 | Cargo 작업 공간, 잠금 파일, 도구 체인 고정과 README. 새 복제본에서 포맷·빌드·시험을 통과한 구현 커밋은 `d2e26f44bacf9dfdeb7e62fbdbaf49bb7c856dfc`다. |
| INIT-03 | 로컬 PASS | Rust SDK, Streamable HTTP, 루프백, 실제 서버 바이너리와 임시 SQLite. 시험에서 초기화, 도구 탐색, 저장, 멱등성, 충돌, 권한, 재시작을 대조했다. |
| INIT-04A | 원격 MCP `NOT_RUN` | 지정된 외부 시험 호스트·HTTPS 주소가 없다. README의 프록시 구성 후 원격 Rust 시험을 실행한다. |
| INIT-04B | ChatGPT MCP `BLOCKED` | 실제 ChatGPT 웹의 플러그인 추가 메뉴, 사용자 지정 MCP 양식과 인증 선택지를 확인했다. HTTPS 주소 또는 지정된 워크스페이스의 Secure MCP Tunnel과 본인 승인을 준비한 뒤 도구 발견부터 재개한다. 서버 호출과 저장·조회 영수증은 미수집이다. |
| INIT-05 | 공개 GitHub 작업 브랜치 및 CI | Windows·Ubuntu CI가 포맷, 잠금 빌드, 테스트를 실행한다. [작업 브랜치의 Actions 실행](https://github.com/ThunderStrikerKR/TSAIPGMR/actions/workflows/ci.yml?query=branch%3Ainit%2Frust-mcp)에서 실행 결과를 확인한다. |

## 확인한 로컬 결과

`cargo test --workspace --locked --test mcp_http local_mcp_round_trip_and_restart -- --nocapture`로 재현한다. 검증 실행 로그와 마지막 확인 구현 커밋은 [로컬 실행 증거](evidence/INIT-local.md)에 기록한다.

다음 작업은 INIT-04A의 외부 시험 경로 확정이다. 시험 endpoint가 생기면 INIT-04A에서 초기화·탐색·저장·재조회를 수행하고, INIT-04B에서 별도 요청 ID로 ChatGPT 왕복을 실행한다. 원격 시험에 실패하면 실행 명령, 마지막 정상 단계, 오류, 영향 항목과 재개 위치를 여기에 남긴다.
