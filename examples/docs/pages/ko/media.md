# 미디어

Zeyn은 HTTP 범위 요청으로 미디어 파일을 제공하고 `Video`와 `Audio` 컴포넌트에서 재생합니다. 디코딩은 기본 `<video>`와 `<audio>` 요소가 하며, Zeyn은 그 위에 접근 가능한 자체 컨트롤을 그립니다.

## 파일 제공

권한과 함께 `MediaSource`를 Feature에 등록합니다.

```rust
Feature::new("library")
    .media("films", "library.watch", FilmFolder::new())
    .media("music", "library.listen", SongTable::new())
```

브라우저는 `GET /_zeyn/media/{source}/{id}`를 요청합니다. 소스가 무엇이든 보기 전에 서버는 다음을 수행합니다.

1. `A-Z a-z 0-9 - _ .` 문자로 1자에서 128자가 아니거나, 점으로 시작하거나, `..`를 포함하는 id를 거부합니다(`404`).
2. 소스의 권한(`403`)과 Feature의 가드를 검사합니다.
3. 인터셉터를 실행합니다(`CallKind::Media`이며, 소스 이름은 `screen`에 들어갑니다).

`open(ctx, id)`는 앱 자체 카탈로그에서 id를 찾아 `Media`를 반환합니다. 디스크의 파일에는 `Media::file(path)`(타입은 확장자에서, 엔티티 태그는 크기와 수정 시각에서 결정), 메모리의 데이터에는 `Media::bytes(data, type)`, 모든 `Read + Seek`에는 `Media::new(reader, len, type)`을 사용합니다. id는 이름이지 경로가 아닙니다. 경로는 id가 아니라 조회 결과로 만드십시오.

응답은 다음과 같습니다.

| 요청 | 응답 |
|---|---|
| `Range`가 없거나, 다른 단위이거나, 범위가 여러 개 | 전체 파일과 함께 `200` |
| `bytes=a-b`, `bytes=a-`, `bytes=-n` | `Content-Range: bytes a-b/len`과 함께 `206` |
| 형식이 잘못되었거나 끝을 넘어서 시작하는 바이트 범위 | `Content-Range: bytes */len`과 함께 `416` |
| 엔티티 태그와 일치하지 않는 `If-Range` | 전체 파일과 함께 `200` |
| 엔티티 태그와 일치하는 `If-None-Match` | `304` |

모든 응답에는 `Accept-Ranges: bytes`, `Cache-Control: private, no-cache`, `X-Content-Type-Options: nosniff`가 붙습니다. 오디오, 비디오, 래스터 이미지, WebVTT, 스트리밍 재생 목록만 인라인으로 제공되며, 그 밖의 타입(HTML, SVG, 스크립트)은 `Content-Disposition: attachment`와 함께 `application/octet-stream`으로 전송되므로 파일이 앱의 페이지로 실행될 수 없습니다. 파일은 블로킹 풀에서 64 KiB 조각으로 읽으며 대기열에 최대 네 조각이 있으므로, 파일 크기와 상관없이 메모리가 일정하게 유지되고 클라이언트가 떠나면 읽기가 멈춥니다. 속도 제한은 다른 요청과 똑같이 적용되며, 미디어 읽기는 쓰기가 아닙니다.

콘텐츠 보안 정책은 변경할 필요가 없습니다. `default-src 'self'`가 이미 앱의 미디어를 허용합니다. Media Source Extensions를 사용하는 적응형 스트리밍 라이브러리에는 `media-src`에 `blob:`이 필요하며, Zeyn은 이를 추가하지 않습니다.

## 공개 파일: 이미지, 로고, 다운로드

로그인 여부와 상관없이 누구나 볼 수 있는 파일은 공개 소스에 둡니다.

```rust
Feature::new("catalog").public_media("images", 86_400, ImageFolder::new("data/images"))
```

`GET /_zeyn/media/images/{id}`는 권한 검사를 건너뛰고(id, 가드, 인터셉터는 여전히 실행됩니다), 쿠키를 설정하지 않으며, `Cache-Control: public, max-age=86400`(최대 `MAX_PUBLIC_MEDIA_CACHE`, 1년)으로 응답하므로 브라우저와 앱 앞단의 CDN이 이를 보관합니다. 엔티티 태그로 `304`와 함께 재검증할 수도 있습니다. 경로를 `Image`의 `src`로 사용하십시오. 큰 이미지가 많다면 CDN 뒤의 별도 이미지 호스트가 여전히 더 저렴한 선택이며, `App::csp(|c| c.img_src([...]))`로 허용합니다.

- `open`은 모든 방문자에게 같은 바이트를 반환해야 합니다. 공개 파일을 사용자, 역할, 테넌트, 언어에 따라 달리하지 마십시오. CDN이나 프록시는 첫 번째 응답을 보관하여 모두에게 전달합니다. 사용자마다 다른 파일은 `Feature::media`로 등록한 소스에 두어야 합니다.
- `max_age`가 길 때는 콘텐츠 해시(`poster-3f9a2c.jpg`)처럼 콘텐츠와 함께 바뀌는 id를 부여하십시오. 그러면 파일이 바뀔 때 새 주소를 받게 되어, 만료될 때까지 오래된 사본이 캐시에 남지 않습니다.
- 공개 소스의 읽기(`GET`과 `HEAD`)는 `App::rate_limit`에 포함되지 않습니다. 한 페이지가 포스터 수십 개를 불러올 수 있고, 많은 모바일 방문자가 통신사 NAT 뒤에서 하나의 주소를 공유하기 때문입니다. 비공개 소스는 여전히 계산에 포함되며, IP 허용 및 거부 규칙은 둘 다에 여전히 적용됩니다. 대역폭 남용을 제한하는 것은 앱 앞단의 CDN이나 리버스 프록시의 역할입니다.

## 플레이어

```rust
Video::new("films", t!("films.harbor"))
    .subtitle(t!("films.harbor_sub"))
    .file("harbor.webm")
    .file("harbor.mp4")
    .hls("harbor.m3u8")
    .poster("harbor.jpg")
    .captions("harbor.en.vtt", "en", "English")
    .captions("harbor.tr.vtt", "tr", "Türkçe")

Audio::new("music", t!("songs.dawn")).subtitle(t!("songs.artist")).file("dawn.mp3")
```

컨트롤: 재생과 일시 정지, 탐색 슬라이더, 시간, 음소거, 볼륨 슬라이더, 재생 속도 메뉴, 자막 버튼, (자막 파일이 둘 이상이면) 언어 메뉴, 그리고 비디오에는 (브라우저가 지원하는 경우) 화면 속 화면과 전체 화면이 있습니다. 슬라이더는 WAI-ARIA 슬라이더 패턴을 따릅니다. 화살표는 5초(또는 볼륨의 5퍼센트)씩 이동하고, Page Up과 Page Down은 10분의 1씩 이동하며, Home과 End는 양 끝으로 이동합니다. 메뉴는 메뉴 버튼 패턴을 따릅니다. 포커스가 플레이어 안에 있는 동안 `k` 또는 Space는 재생과 일시 정지를, `j`와 `l`은 10초 이동을, `m`은 음소거를, `c`는 자막 전환을, `f`는 전체 화면 전환을 합니다. 이 키는 페이지의 다른 곳에서는 아무 동작도 하지 않습니다.

자막은 같은 소스가 제공하는 WebVTT 파일입니다. 플레이어는 활성 큐를 일반 텍스트로 그립니다(큐 마크업은 제거됨). 비디오에서는 화면 위에, 오디오에서는 제목 아래에 표시하므로 자막은 모든 브라우저와 전체 화면에서 똑같이 보입니다.

## 이어 보기

플레이어는 사용자가 멈춘 위치를 저장합니다. 재생 중에는 15초마다, 일시 정지할 때, 페이지가 숨겨지거나 닫힐 때, 미디어가 끝나면 `0`을 저장합니다. 저장된 지점이 양쪽 끝에서 5초보다 멀 때 이어서 재생합니다.

- 소스가 `position`과 `save_position`을 구현하면 위치는 서버로 갑니다. `GET /_zeyn/media/position?source=&id=`는 `{"at": seconds, "kept": true}`로 응답하고, `{source, id, at}`와 함께 `POST /_zeyn/media/position`은 저장합니다(`204`). 둘 다 id, 권한, 가드를 검사하며 로그인한 사용자가 필요합니다. POST에는 CSRF 헤더도 필요하며 속도 제한에서 쓰기로 계산됩니다. `MAX_POSITION_SECONDS`를 넘거나 0보다 작은 위치는 `422`입니다.
- 그렇지 않으면(소스의 기본 메서드이거나 익명 사용자) 읽기는 `{"at": null, "kept": false}`로 응답하고 플레이어는 위치를 브라우저의 로컬 저장소에 `zeyn.media.{source}/{id}`로 보관합니다. 저장소 오류(비공개 창, 차단된 저장소)는 무시됩니다.

로그인한 사용자에게는 서버 위치가 더 나은 선택입니다. 사용자를 따라 모든 기기로 이동하며, 앱이 "이어서 보기" 행에 이를 읽을 수 있습니다. 로컬 저장소는 코드가 필요 없지만 한 브라우저에만 남으며 그 브라우저를 사용하는 모든 사람이 공유합니다.

## HLS와 DASH

`hls(id)`와 `dash(id)`는 적응형 스트림을 추가합니다. Safari는 HLS를 기본으로 재생하므로 재생 목록을 사용합니다. 다른 브라우저에는 JavaScript 라이브러리(hls.js나 dash.js 등)가 필요하며 Zeyn은 이를 제공하지 않습니다. 프로그레시브 파일로 대체하기 전에 플레이어는 루트에서 취소 가능한 `zeyn:media-adaptive` 이벤트를 발생시킵니다.

```js
document.addEventListener('zeyn:media-adaptive', (e) => {
  const { media, kind, url } = e.detail;
  if (kind !== 'hls' || !window.Hls?.isSupported()) return;
  e.preventDefault();
  const hls = new window.Hls();
  hls.loadSource(url);
  hls.attachMedia(media);
});
```

그 스크립트는 앱의 파일에서 불러오십시오(인라인 스크립트는 콘텐츠 보안 정책에 의해 차단됩니다). 어떤 리스너도 `preventDefault()`를 호출하지 않으면 `file` 항목이 순서대로 재생됩니다. 재생 목록과 세그먼트 파일은 같은 소스가 제공하므로, id가 그 안에 나열되어 있는 한 상대 URL이 동작합니다.

## 업로드된 파일

`Form::file`로 업로드된 파일은 스토어의 권한, Feature의 가드, 테넌트 검사 이후 `GET /_zeyn/files/{store}/{id}`로 미디어 파일과 같은 응답(범위, 엔티티 태그, `nosniff`, 래스터 이미지, 오디오, 비디오만 인라인)으로 제공됩니다. `examples/docs/pages/forms.md`의 파일 업로드 섹션을 참고하십시오.
