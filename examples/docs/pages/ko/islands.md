# WASM 아일랜드

Zeyn은 서버에서 페이지를 렌더링합니다. 캔버스 편집기, 다이어그램, 무거운 차트처럼 이 방식으로는 상호작용이 너무 많은 영역도 있습니다. **아일랜드**는 WebAssembly로 컴파일된 Rust 컴포넌트이며, 서버가 그 외에는 서버에서 렌더링된 페이지에 배치합니다. 그 주변의 모든 것은 일반 HTML로 남습니다.

아일랜드는 `zeyn`의 `islands` Cargo 기능 뒤에 있습니다. 이를 켜지 않은 앱은 이 중 아무것도 컴파일하지 않으며, 아일랜드가 없는 페이지는 켜져 있어도 아무 비용도 치르지 않습니다(아래 성능 참고).

## 1. 구성 요소

| 구성 요소 | 위치 | 역할 |
|---|---|---|
| `zeyn::ui::Island` | 서버 | 자리 표시자 노드: 속성, 대체 콘텐츠, 바인딩된 action |
| `App::islands(dir)` | 서버 | 빌드된 파일과 로더를 제공합니다 |
| `zeyn-island` | 공유 및 브라우저 | `Remote`(호출 가능한 action의 계약), `Island::call`(클라이언트) |
| 공유 크레이트 | 양쪽 | 속성, 메시지, remote를 일반 serde 타입으로 |
| 아일랜드 크레이트 | 브라우저 | `mount(root)`와 선택적으로 `unmount(root)`를 내보내는 `cdylib` |
| `zeyn build-islands` | CLI | 아일랜드 크레이트를 해시된 파일과 매니페스트로 빌드합니다 |

`examples/showcase`와 같은 전형적인 레이아웃은 다음과 같습니다.

```
examples/showcase/
  shared/              showcase-shared: props, Count, SaveCount (serde + zeyn-island only)
  islands/counter/     the island crate, [lib] name = "counter", crate-type = ["cdylib", "rlib"]
  src/islands.rs       the screen and the action
  dist/islands/        output of zeyn build-islands (not committed)
```

## 2. 공유 타입

공유 크레이트는 `serde`와 기능이 없는 `zeyn-island`에 의존하므로 서버는 브라우저 코드를 컴파일하지 않습니다. remote는 아일랜드가 호출할 수 있는 action의 이름을 지정하고 그 타입을 고정합니다.

```rust
#[derive(Serialize, Deserialize)]
pub struct Count { pub value: i64 }

pub struct SaveCount;

impl zeyn_island::Remote for SaveCount {
    const NAME: &'static str = "save_count";
    type Input = Count;
    type Output = Count;
}
```

## 3. 서버 측

```rust
#[action(permission = "showcase.islands.write")]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> { ... }

Island::new("counter")
    .props(&CounterProps { start, texts })
    .label(t!("islands.counter"))
    .fallback(Alert::info(t!("islands.fallback")))
    .action::<SaveCount, _>(save_count)
```

`action::<R, A>`는 action의 입력이 `R::Input`이고 출력이 `Result<R::Output>`일 때만 컴파일되므로 양쪽이 어긋날 수 없습니다. 아일랜드는 자신의 노드가 바인딩한 action만 호출할 수 있습니다.

`fallback`은 모든 노드를 받습니다. 텍스트 요약이나 읽기 전용 그리드가 그 예입니다. 아일랜드가 로드되기 전, JavaScript가 꺼져 있을 때, 로드에 실패했을 때 표시됩니다. 그 안의 그리드와 다른 컴포넌트는 평소처럼 동작합니다.

로드: `.on_interaction()`은 자리 표시자에 대한 첫 포인터, 터치, 포커스를 기다리고, `.on_idle()`은 브라우저가 유휴 상태일 때 로드하며, 기본값은 자리 표시자가 뷰포트에서 200 px 이내로 들어올 때 로드합니다. 로드는 페이지를 차단하지 않습니다.

출력 폴더를 한 번 등록하십시오. `App::islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"))`. 그곳에 매니페스트가 없으면 서버는 경고를 기록하고 아일랜드는 대체 콘텐츠를 표시합니다.

## 4. 아일랜드 측

```rust
#[wasm_bindgen]
pub fn mount(root: web_sys::Element) -> Result<(), JsValue> {
    let island = zeyn_island::Island::new(root.clone());
    let props: CounterProps = island.props().map_err(|e| JsValue::from_str(&e.message))?;
    // build DOM under root, attach listeners
    zeyn_island::spawn(async move {
        match island.call::<SaveCount>(&Count { value: 3 }).await {
            Ok(saved) => { /* ... */ }
            Err(e) => { /* e.message is localized by the server */ }
        }
    });
    Ok(())
}
```

`Island::call`은 아일랜드의 Screen, 매개변수, 노드 경로, `key`로서의 remote 이름, 입력과 함께 `POST /_zeyn/data`로 전송합니다. 페이지의 CSRF 토큰을 `x-zeyn-csrf`로, 세션 쿠키와 함께 보냅니다. 서버는 모든 컴포넌트와 마찬가지로 CSRF, Screen의 접근 권한, Feature의 가드, action의 권한을 검사한 다음 인터셉터와 action을 시간 제한과 함께 실행합니다. 다른 진입 경로는 없습니다. 아일랜드는 자신의 노드가 바인딩하지 않은 action에 접근할 수 없으며, 아일랜드와 관련된 어떤 것도 검사를 우회하지 않습니다.

오류는 `CallError { status, code, message }`로 돌아옵니다. `status`는 로컬 실패(`network`, `props`, `input`, `answer`)에서 `0`입니다.

아일랜드가 타이머, 애니메이션 프레임, 소켓을 보유하면 `unmount(root)`를 내보내십시오. 로더는 자리 표시자가 문서를 떠날 때(탐색, 닫힌 창) 이를 호출합니다.

번역: 아일랜드가 표시하는 텍스트는 서버가 이미 현지화하여 속성으로 전달하십시오. 그러면 아일랜드가 카탈로그를 포함하지 않습니다.

## 5. 빌드

```
cd examples/showcase
zeyn build-islands            # every crate in islands/, output in dist/islands
zeyn build-islands --out public/islands islands/counter
```

각 크레이트에 대해 다음을 실행합니다.

1. `--config`로 전달한 크기 설정(`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, 디버그 정보 없음)과 함께 `cargo build --lib --release --target wasm32-unknown-unknown`을 실행하므로 앱 자체의 릴리스 프로필은 건드리지 않습니다.
2. `wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section`(`wasm-bindgen-cli` 버전은 `wasm-bindgen` 크레이트 `0.2.130`과 일치해야 합니다)을 실행합니다.
3. binaryen이 `PATH`에 있으면 `wasm-opt -Oz`를 실행합니다.
4. `{name}.{hash}.js`와 `{name}_bg.{hash}.wasm`(콘텐츠의 64비트 FNV 해시)을 기록하고 `manifest.json`을 갱신합니다.

`zeyn dev`는 앱에 `islands/` 폴더가 있으면 앱을 빌드하기 전에 매번 같은 일을 하며, `dist/`는 감시하지 않습니다.

서버는 매니페스트에 나열된 파일 이름만 `/_zeyn/islands/{file}`에서 `application/wasm` 또는 `text/javascript`와 `Cache-Control: public, max-age=31536000, immutable`로 제공합니다. 파일은 요청마다 디스크에서 읽으므로 서버 메모리를 사용하지 않습니다.

## 6. Content Security Policy

WebAssembly를 컴파일하려면 `script-src`에 `'wasm-unsafe-eval'`이 필요합니다. Zeyn은 이를 Screen에 아일랜드가 있는 페이지에만 추가하며, 다른 모든 페이지는 `docs/runtime-protocol.md`의 엄격한 정책을 유지합니다. `'wasm-unsafe-eval'`은 `WebAssembly.compile`과 `instantiate`만 허용하며 그 밖의 것은 허용하지 않습니다. `eval`, `new Function`, 인라인 스크립트는 계속 차단되고, 모듈은 여전히 앱의 출처에서만 옵니다.

정책은 문서에 속하며 나중에 완화할 수 없습니다. 따라서 `nav.js`가 아일랜드가 있는 Screen(부분 응답에 `x-zeyn-islands: 1`이 있음)을 이를 허용하지 않고 로드된 문서에 불러올 때는 전체 페이지 로드로 대체합니다. 문서가 아일랜드 정책을 가지고 나면 아일랜드가 있는 Screen과 일반 Screen 사이의 이동은 그 자리에서 이루어집니다.

서버는 `data-zeyn-island="` 속성으로 아일랜드를 찾습니다. 이스케이프된 사용자 데이터는 그 따옴표를 만들 수 없으므로 사용자가 입력한 텍스트로 정책을 켤 수 없습니다.

## 7. 캔버스 표면

`zeyn::ui::Canvas::new(name, description)`은 그리기를 위한 아일랜드입니다. 설명은 필수입니다. 캔버스는 보조 기술에 보이지 않으므로 페이지는 캔버스가 무엇을 보여 주고 어떤 키로 조작하는지 알려야 합니다. 서버는 이를 아일랜드 위의 문단으로 렌더링하고 자리 표시자의 `aria-describedby`가 이를 가리키게 합니다.

아일랜드 측에서 `canvas` 기능을 사용하는 `zeyn-island`는 `canvas::Surface`를 제공합니다.

```rust
let surface = Surface::new(&stage, Editor::new(..), Prefer::TwoD, &texts.canvas, &described_by, 420)?;
surface.update(|editor| editor.add_box());   // change the scene, draw on the next frame
```

| 관심사 | 표면이 하는 일 |
|---|---|
| 그래픽 | `Prefer::Gl`은 WebGL 2를 시도하고 2D로 대체합니다. `webgpu_available()`은 `surface.canvas()`에서 WebGPU를 직접 설정하는 장면을 위해 `navigator.gpu`가 있는지 알려 줍니다 |
| 고해상도 | 백킹 스토어는 `devicePixelRatio`와 요소 크기(`ResizeObserver`)를 따릅니다. 2D 그리기는 미리 스케일되므로 장면은 CSS 픽셀로 동작합니다 |
| 입력 | 포인터 누름, 이동, 해제(포인터 캡처 포함), 휠(줄과 페이지 델타를 픽셀로 변환), 키를 CSS 픽셀 단위 `Input` 값으로 전달합니다 |
| 프레임 | 장면이 사용한 입력, `update`, 크기 조정 후, 또는 `Scene::animating`이 `true`를 반환하는 동안에만 `requestAnimationFrame`을 사용합니다. 정지된 그림은 CPU를 쓰지 않습니다 |
| 기본 동작 | 장면이 사용하는 입력은 기본 동작이 막힙니다(화살표가 페이지를 스크롤하지 않음). 포커스가 떠날 수 있도록 Tab에는 `false`를 반환하십시오 |
| 정리 | `destroy()`는 리스너, 옵저버, 캔버스를 제거합니다. `unmount`에서 호출하십시오 |

캔버스에는 `tabindex="0"`, `role="application"`, 레이블, `aria-describedby`가 붙습니다.

쇼케이스 다이어그램 편집기(`examples/showcase/islands/diagram`)와 같은 접근성 패턴은 다음과 같습니다.

- **캔버스의 키보드:** `]`와 `[`는 도형을 선택하고, 화살표는 이동하고, Delete는 제거하며, `+`, `-`, `0`은 확대와 축소를 하고, Escape는 선택을 해제합니다. 키는 캔버스에 포커스가 있는 동안에만 동작합니다.
- **일반 컨트롤:** 버튼 툴바가 캔버스 없이 같은 일을 합니다.
- **텍스트 요약:** 캔버스 아래의 목록이 모든 도형을 설명하고 변경마다 따라가며, `role="status"` 줄이 선택과 저장 결과를 안내합니다.
- **대체 콘텐츠:** 서버가 같은 요약을 렌더링하므로 WebAssembly가 없는 사용자도 다이어그램을 읽을 수 있습니다.

편집기는 `SaveDiagram`으로 저장하며, 아일랜드는 절대 신뢰하지 않으므로 서버는 받은 것을 제한합니다(`Diagram::clamped`: 최대 200개 도형, 고유한 id, 40자의 레이블, 제한된 좌표).

## 8. 성능

| 항목 | 수치 |
|---|---|
| 일반 페이지 HTML, 요청, 정책 | 변경 없음: meta도, 로더도, 추가 요청도 없음 |
| `nav.js` | +281바이트 (전체 로드 대체) |
| `/_zeyn/islands.js` 로더, 아일랜드 페이지에만 | 3.9 KB, gzip 1.7 KB |
| 카운터 아일랜드 `.wasm` (wasm-opt 없음) | 127.6 KB, gzip 52.8 KB |
| 카운터 아일랜드 글루 `.js` | 18.0 KB, gzip 4.2 KB |
| 다이어그램 편집기 `.wasm` (캔버스 표면, wasm-opt 없음) | 169.4 KB, gzip 67.8 KB |
| 실시간 편집이 있는 다이어그램 편집기 (`yrs` 추가, `docs/realtime.md` 참고) | 408.4 KB, gzip 144.3 KB |
| 다이어그램 편집기 글루 `.js` | 30.3 KB, gzip 6.1 KB |
| 아일랜드당 서버 메모리 | 없음. 파일은 디스크에서 스트리밍되고 속성은 요청마다 렌더링됩니다 |

카운터 크기의 대부분은 `serde_json`과 `core`의 서식 코드입니다. 이를 공유하는 아일랜드는 각각 한 번씩 비용을 치르며, wasm-opt는 보통 10에서 20퍼센트를 더 줄입니다.

## 9. 제한

- 창(`open_window`) 안의 아일랜드는 여는 페이지가 아일랜드 정책을 가지고 있어야 합니다. 아일랜드가 있는 창은 아일랜드 페이지에 배치하거나 Screen으로 여십시오.
- `Closure::forget`으로 연결한 클로저는 페이지가 유지되는 동안 살아 있습니다. 아일랜드가 여러 번 마운트될 수 있다면 `unmount`에서 해제하십시오.
- 아일랜드 자체는 서버에서 아무것도 렌더링하지 않습니다. 검색 엔진, 로드 전의 스크린 리더, JavaScript가 없는 사용자가 받는 것은 대체 콘텐츠입니다. 이를 의미 있게 만드십시오.
