# 表单

`Form::of(value)` 用于编辑一个 model 或 entity。字段来自字段引用，标签来自语言文件，验证来自 `#[rule]` 属性，因此浏览器和服务器检查的是同样的规则。

## 一个表单

```rust
Form::of(Contact { quantity: 1, ..Contact::default() })
    .row(|r| r.field(Contact::name).field(Contact::email))
    .row(|r| r.field(Contact::quantity).field(Contact::size))
    .submit(t!("docs.live.send"), send_contact)
    .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
```

@live form

把姓名留空，或输入错误的邮箱，然后按 *Send*：浏览器会立即显示错误，而服务器也会以同样的消息拒绝同样的输入。

## 布局

| 调用 | 结果 |
|---|---|
| `.field(M::f)` | 单独一行的一个字段 |
| `.row(\|r\| r.field(a).field(b))` | 并排的字段；在窄屏幕上会堆叠 |
| `.section(title)` | 其后各字段的带标题分组 |
| `.step(title)` | 向导的一个步骤；表单一次显示一个步骤 |
| `.range(M::from, M::to)` | 两个日期合为一个范围选择器 |
| `Form::auto(value)` | 所有字段，按声明顺序 |

表单最宽 720 px，只包含一个表单的卡片或面板与表单同宽，因此卡片不会有一半空着。标签位于 36 px 输入框上方 6 px 处；字段之间相距 20 px。获得焦点的输入框显示焦点色边框和一圈柔和的 3 px 光环；无效的输入框显示危险色边框。单位、货币和 `%` 以弱化样式绘制在输入框内部，数字字段把减号和加号按钮（28 px）放在字段末端内部；方向键仍可步进。

## 保存

`.submit(label, action)` 指定负责保存的 action。该 action 必须接收该表单 model 的 `Valid<M>`；其他类型无法编译。运行时发送这些值，服务器运行每条规则，然后运行 action。

- 来自规则或 action 中 `ValidationErrors` 的字段错误会显示在字段旁边。
- `Error::Conflict`（旧的 `#[version]`）会显示一条消息，说明有人修改了该记录。
- `.on_success(|result, ui| ...)` 在保存成功后运行 effect（参见 [Effects 与窗口](effects.md)）。

## 控件

每种字段类型都有默认控件：文本输入、数字输入、带币种的金额、日期和时间选择器、`bool` 的复选框以及选项的下拉框。`.control(M::f, Control::...)` 选择另一种控件，`.lookup(M::f, action)` 在服务器上搜索记录。参见[控件与选择器](controls.md)。

## 依赖的 lookup

`.depends_on(field, source)` 让 lookup `field` 的选项依赖于另一个字段，例如某个国家的城市。每次搜索都会在 `LookupQuery::parent` 中发送 `source` 的值（`q.parent_id()` 把它读作 id），`source` 改变时会清空 `field`，使其选项按新值重新加载。

```rust
#[action(permission = "geo.read")]
pub async fn find_cities(q: LookupQuery, store: Cities) -> Result<Page<LookupItem>> {
    Ok(q.apply(store.of_country(q.parent_id())))
}

Form::of(address)
    .lookup(Address::country, find_countries)
    .lookup(Address::city, find_cities)
    .depends_on(Address::city, Address::country)
    .submit(t!("save"), save_address)
```

这一对值不能交给浏览器信任，因此提交时服务器会带着提交的国家，以该 action 的权限再次向 lookup 的 action 查询所选城市，对它不返回的城市给出字段错误并拒绝。按 id 查询时也要按父值筛选，就像筛选后再调用 `q.apply` 那样。`App::check_page` 会报告字段不是表单 lookup、或来源不是表单另一个字段的 `depends_on`。

## 筛选表单

没有提交按钮的表单可以筛选界面：`.drives(topic)` 把它的字段变成界面的 params。字段变化时（选择框、日期范围、按 Enter 的文本框），其值进入同名的界面 param，地址保持同步，`.listens(topic)` 的节点就地刷新并从自己的 params 中读取该值（见 [主题](effects.md#topics)）。数字和布尔值按原样发送，空字段发送为 `null`；服务器把 params 读作界面的类型化 params，不合适的值会被拒绝。

```rust
#[params]
pub struct SalesParams {
    pub region: Option<Region>,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

#[screen(route = "/sales", permission = "sales.read")]
pub fn sales(p: SalesParams, store: Sales) -> Screen {
    Screen::new(t!("sales.title"))
        .body(
            Form::of(SalesFilter { region: p.region, from: p.from, to: p.to })
                .field(SalesFilter::region)
                .range(SalesFilter::from, SalesFilter::to)
                .drives("sales"),
        )
        .body(Kpi::new(t!("sales.total"), store.total(&p)).listens("sales"))
}
```

带 `submit` 的表单不能驱动主题；请改为在保存后于 `on_success` 中用 `ui.refresh(topic)` 刷新。

## 只读字段

`#[readonly]` 字段会在表单中显示但不能编辑，例如只由库存变动来改变的库存量。

## 文件上传

`File`、`Option<File>` 和 `Vec<File>` 字段保存由 `FileStore` 管理的文件。把规则声明为一个 `const`，在 Feature 上连同权限注册 store，并在表单上使用同样的规则：

```rust
pub const ATTACHMENTS: FileRules = FileRules::new("attachments")
    .max_bytes(5 * 1024 * 1024)
    .max_files(5)
    .accept(&[FileType::Pdf, FileType::Png, FileType::Jpeg]);

Feature::new("support")
    .files(ATTACHMENTS, "support.ticket.files", DiskStore::new("data/attachments"))
    .scan_files(MyVirusScanner)

Form::of(ticket).field(Ticket::title).file(Ticket::attachments, ATTACHMENTS)
```

一个权限名让同一批用户既能上传也能下载。若要让读者打开他们无权添加的文件，请将其拆分：`.files(ATTACHMENTS, FileAccess::read("support.ticket.read").write("support.ticket.files"), store)`。

该控件是放在拖放区域内的原生文件输入。每个文件独立上传并带进度条，上传过程中可以取消，失败后可以重试，完成后可以移除。图片会显示由所选文件的对象 URL 生成的预览。进度和结果通过一个 polite 实时区域播报。showcase 在 `/uploads` 有一个可运行的示例；本文档站点没有已登录的用户，因此不显示实时上传。

拖放的文件夹会加入其中及子文件夹里的文件，每次最多读取 100 个条目，直到读完整个文件夹；字段不接受的类型的文件会被跳过，并在实时区域中报告数量。字段获得焦点时粘贴的文件也会上传。扩展名或类型不在 `accept` 中的文件会在发送任何字节之前被浏览器拒绝，服务器还会再次检查内容。一个字段最多同时上传三个文件，其余文件带着忙碌进度条排队等待。

| 按键 | 作用 |
|---|---|
| Tab | 移到文件输入，然后依次移到每个文件的下载链接及其取消、重试或移除按钮 |
| 在文件输入上按 Enter 或 Space | 打开系统文件选择器（拖放的键盘替代方式） |
| 在文件输入上按 Ctrl+V 或 Cmd+V | 上传剪贴板中的文件，例如截图 |
| 在取消、重试、移除上按 Enter 或 Space | 执行该操作；取消后焦点移到重试，移除后移到下一个文件或回到输入框 |

服务器对 `POST /_zeyn/files/{store}` 所做的处理：

1. 检查 CSRF 请求头、store 的权限和 Feature 的 guard，并运行 interceptor（`CallKind::Upload`，以新文件 id 进行审计）；上传计入写入限制；
2. 拒绝声明的 `Content-Length` 超出限制的请求（`413`），然后读取前 4 KiB 并根据内容判断类型；扩展名必须属于同一类型，且该类型必须在 `accept` 中，否则返回 `415`。HTML、SVG、XML 和脚本永远不会被接受。浏览器给出的内容类型会被忽略；
3. 把其余部分以最多 64 KiB 的块、通过容量为四的通道流式写入 store，并统计字节数；超出限制、连接中断或 30 秒没有数据都会停止上传，store 会删除已写入的内容；
4. 运行 Feature 的 `FileScanner`；被拒绝的文件，或扫描器失败时，文件会被删除，并返回 `422`；
5. 返回 `201` 和 `{id, name, size, type}`。id 是服务器生成的 128 位随机数；文件名会被清除路径和控制字符。

启用 `image` 功能后，`FileRules::clean_images()` 会在存储保存文件后增加一步：PNG、JPEG 和 WebP 上传会被解码并以相同格式重新编码，因此 EXIF（含 GPS）、XMP、ICC 配置文件、文本块以及藏在图像数据之后的字节都不会留在服务器上。EXIF 方向会先应用到像素上；JPEG 以质量 90 写出，PNG 和 WebP 为无损。解码在单边 `MAX_IMAGE_SIDE`（12,000）像素和 `MAX_IMAGE_MEMORY`（256 MiB）处停止；无法解码的图像会被删除，响应为 `422`。两个上限都会在解码任何像素之前按图像头中的尺寸检查，因此声称是巨大图像的小文件会在不占用那部分内存的情况下被拒绝；动画图像只保留第一帧。清理后的文件以新的 id 替换原文件，随后扫描器检查清理后的文件。没有该功能时这个方法不存在，因此存储不会在没有清理时误以为已经清理。

下载（`GET /_zeyn/files/{store}/{id}`）会检查 id 的形态、权限、guard 和租户，并通过媒体服务（参见 `docs/media.md`）处理：范围请求、实体标签和 `nosniff`。只有位图图片、音频和视频以内联方式提供；其余一律以 `application/octet-stream` 和 `Content-Disposition: attachment` 发送，附带带引号的 ASCII 名称和 UTF-8 名称，并带有 `Content-Security-Policy: sandbox`。

`DiskStore` 在一个文件夹中保存 `{id}.bin` 和 `{id}.json`，并先把上传内容写到 `{id}.part`。`MemoryStore` 把文件保存在内存中并设有字节上限，用于测试和演示。表单值只是引用文件的名称：必须确认文件存在的保存 action 应调用 `store.meta(id)`，showcase 就是这样做的。

`FileRules::quota(bytes, window)` 限制单个用户在每个时间窗口内向某个存储上传的量，并通过应用的 rate store 计数；超过限额的那次上传仍会保存，下一次上传返回 `429`。`DiskStore` 最多每小时一次删除超过一小时的 `.part` 文件（因崩溃而中断的上传）。`ZipGuard` 是针对基于 ZIP 的上传（ZIP、Word、Excel、PowerPoint）的扫描器：它会拒绝中央目录声明超过 10,000 个条目、总计超过 1 GiB 或超过文件大小 200 倍的归档，以及 ZIP64 归档；解压已接受归档的代码仍须在这些限制处停止。

预算：上传 256 MiB 的文件会使服务器堆增长约 0.4 MiB（`tests/files_memory.rs`）；该控件渲染 10 个 DOM 节点，每个文件再加 5 个；`files.js` 增加 17 KB 原始大小和 6 KB gzip 大小，`files.css` 增加 3 KB 原始大小和 1 KB gzip 大小。

## File drop

`FileDrop::new(label, rules)` 是同一个上传控件脱离表单单独使用，适用于文件就是全部任务的界面，例如文档收件箱。它像表单字段一样接受文件夹、粘贴的文件和 accept 列表，并上传到 `rules` 的存储，该存储由 `Feature::files` 连同权限一起注册。`.single()` 每次只接受一个文件。`.on_upload(action)` 对每个已保存的 `File` 运行一个动作；服务器先检查 id 是否是它生成的格式，再次清理名称，并只保留规则中的内容类型，因此动作不会收到浏览器编造的任何内容。动作拒绝时，条目会显示原因。区域内的文件选择器是键盘和单指针的操作路径。

```rust
#[action(permission = "support.ticket.files")]
pub async fn attach(file: File, tickets: Tickets) -> Result<()> {
    tickets.attach(file).await
}

FileDrop::new(t!("ticket.attachments"), ATTACHMENTS).on_upload(attach)
```
