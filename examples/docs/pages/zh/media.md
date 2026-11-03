# 媒体

Zeyn 通过 HTTP Range 请求提供媒体文件，并在 `Video` 和 `Audio` 组件中播放它们。解码由原生的 `<video>` 和 `<audio>` 元素完成；Zeyn 在其上绘制自己的无障碍控件。

## 提供文件

在 feature 上注册一个带权限的 `MediaSource`：

```rust
Feature::new("library")
    .media("films", "library.watch", FilmFolder::new())
    .media("music", "library.listen", SongTable::new())
```

浏览器获取 `GET /_zeyn/media/{source}/{id}`。在 source 看到任何内容之前，服务器会：

1. 拒绝不是由 1 到 128 个 `A-Z a-z 0-9 - _ .` 组成、以点开头或包含 `..` 的 id（`404`）；
2. 检查 source 的权限（`403`）和 feature 的守卫；
3. 运行拦截器（`CallKind::Media`，source 名称放在 `screen` 中）。

`open(ctx, id)` 在应用自己的目录中查找该 id 并返回 `Media`：对磁盘上的文件使用 `Media::file(path)`（类型来自扩展名，实体标签来自大小和修改时间），对内存中的数据使用 `Media::bytes(data, type)`，对任何 `Read + Seek` 使用 `Media::new(reader, len, type)`。id 是名称，绝不是路径：请根据你的查找结果构建路径，而不是根据 id。

响应：

| 请求 | 响应 |
|---|---|
| 没有 `Range`、其他单位或多个范围 | `200`，返回整个文件 |
| `bytes=a-b`、`bytes=a-`、`bytes=-n` | `206`，带 `Content-Range: bytes a-b/len` |
| 格式错误或起点超出末尾的字节范围 | `416`，带 `Content-Range: bytes */len` |
| 与实体标签不匹配的 `If-Range` | `200`，返回整个文件 |
| 与实体标签匹配的 `If-None-Match` | `304` |

每个响应都带有 `Accept-Ranges: bytes`、`Cache-Control: private, no-cache` 和 `X-Content-Type-Options: nosniff`。只有音频、视频、位图图像、WebVTT 和流式播放列表以内联方式提供；任何其他类型（HTML、SVG、脚本）都会以 `application/octet-stream` 加 `Content-Disposition: attachment` 发送，因此文件永远不会作为应用的页面运行。文件在阻塞线程池上以 64 KiB 的块读取，最多排队四个块，因此无论文件多大内存都保持平稳，并且离开的客户端会停止读取。速率限制与其他请求一样适用；媒体读取不算写入。

内容安全策略无需改动：`default-src 'self'` 已经允许来自应用的媒体。使用 Media Source Extensions 的自适应流媒体库需要在 `media-src` 中加入 `blob:`，Zeyn 不会添加它。

## 公开文件：图片、徽标、下载

任何人（无论是否登录）都可以查看的文件放在公开 source 中：

```rust
Feature::new("catalog").public_media("images", 86_400, ImageFolder::new("data/images"))
```

`GET /_zeyn/media/images/{id}` 跳过权限检查（id、守卫和拦截器仍会运行），不设置 cookie，并以 `Cache-Control: public, max-age=86400` 响应（最大为 `MAX_PUBLIC_MEDIA_CACHE`，一年），因此浏览器和应用前面的 CDN 会缓存它。实体标签仍允许它们通过 `304` 重新验证。把该路径用作 `Image` 的 `src`。对于大量大图片，位于 CDN 之后的独立图片主机仍是成本更低的选择；请用 `App::csp(|c| c.img_src([...]))` 允许它。

- `open` 必须对每个访问者返回相同的字节。绝不要按用户、角色、租户或语言区分公开文件：CDN 或代理会保留第一个响应并把它交给所有人。因人而异的文件应放在用 `Feature::media` 注册的 source 中。
- 使用较长的 `max_age` 时，请给出随内容变化的 id，例如内容哈希（`poster-3f9a2c.jpg`）。这样文件改变后会获得新的地址，而不是让旧副本在缓存中一直存活到过期。
- 对公开 source 的读取（`GET` 和 `HEAD`）不计入 `App::rate_limit`：一个页面可能加载几十张海报，而且许多移动端访问者在运营商 NAT 之后共用一个地址。私有 source 仍然计数，IP 允许和拒绝规则仍对两者生效。限制带宽滥用是应用前面的 CDN 或反向代理的职责。

## 播放器

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

控件：播放和暂停、进度滑块、时间、静音、音量滑块、播放速度菜单、字幕按钮，以及（有多个字幕文件时）语言菜单；对于视频还有画中画（在浏览器支持时）和全屏。滑块遵循 WAI-ARIA slider 模式：方向键移动 5 秒（或音量的 5%），Page Up 和 Page Down 移动十分之一，Home 和 End 跳到两端。菜单遵循 menu button 模式。当焦点在播放器内时，`k` 或空格播放和暂停，`j` 和 `l` 跳转 10 秒，`m` 静音，`c` 切换字幕，`f` 切换全屏；这些键在页面其他位置不起作用。

字幕是由同一个 source 提供的 WebVTT 文件。播放器把当前字幕条以纯文本绘制（会丢弃字幕标记），视频叠在画面上，音频放在标题下方，因此字幕在所有浏览器和全屏模式下看起来都一样。

## 续播

播放器会保存用户停下的位置：播放期间每 15 秒一次，暂停时，页面被隐藏或关闭时，以及媒体播放结束时保存 `0`。当保存的位置距离任一端超过 5 秒时才会续播。

- 当 source 实现了 `position` 和 `save_position` 时，位置会发送到服务器：`GET /_zeyn/media/position?source=&id=` 返回 `{"at": seconds, "kept": true}`，而带有 `{source, id, at}` 的 `POST /_zeyn/media/position` 用于保存（`204`）。两者都会检查 id、权限和守卫，并需要已登录的用户；POST 还需要 CSRF 请求头，并按速率限制中的写入计数。超过 `MAX_POSITION_SECONDS` 或小于零的位置返回 `422`。
- 否则（source 使用默认方法，或用户是匿名的），读取返回 `{"at": null, "kept": false}`，播放器会把位置保存在浏览器的本地存储中，键为 `zeyn.media.{source}/{id}`。存储错误（隐私窗口、被阻止的存储）会被忽略。

对已登录用户而言，服务器端位置是更好的选择：它们会跟随用户到每台设备，应用也可以读取它们来做“继续观看”行。本地存储无需代码，但只留在一个浏览器中，并由使用它的所有人共享。

## HLS 和 DASH

`hls(id)` 和 `dash(id)` 添加自适应流。Safari 原生播放 HLS，因此它使用播放列表。其他浏览器需要 JavaScript 库（如 hls.js 或 dash.js），Zeyn 不附带它们。在回退到渐进式文件之前，播放器会在其根元素上派发一个可取消的 `zeyn:media-adaptive` 事件：

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

请从应用自己的文件中加载该脚本（内联脚本会被内容安全策略阻止）。当没有监听器调用 `preventDefault()` 时，`file` 条目会按顺序播放。播放列表和分段文件由同一个 source 提供，因此只要它们的 id 列在其中，它们的相对 URL 就能正常工作。

## 上传的文件

通过 `Form::file` 上传的文件由 `GET /_zeyn/files/{store}/{id}` 提供，响应与媒体文件相同（范围请求、实体标签、`nosniff`，仅位图图像、音频和视频以内联方式提供），并在 store 的权限、feature 的守卫和租户检查之后进行。请参见 `examples/docs/pages/forms.md` 中关于文件上传的部分。
