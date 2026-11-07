# 社交

用于社交页面的组件。每个组件都在服务端渲染，并且只在使用它的页面上加载自己的样式表和脚本。

## StoryRing

`StoryRing` 是带环的头像，用来表示某人的动态对查看者来说是否是新的。`StoryState::Unseen` 绘制品牌色的粗环，`StoryState::Seen` 绘制中性色的细环。状态来自服务端，取决于应用保存的关于查看者的数据；浏览器从不修改它。`StoryRow` 把多个环排成一行，可以像滑轨一样横向滚动。

```rust
StoryRow::new()
    .add(StoryRing::new(ada.name.clone(), StoryState::Unseen)
        .image(ada.photo.clone())
        .link(stories, StoryParams { person: ada.id }))
    .add(StoryRing::new(alan.name.clone(), StoryState::Seen).image(alan.photo.clone()))
```

设置 `.link(screen, params)` 后，环就是指向显示动态的屏幕的链接。它的无障碍名称是此人的姓名加上状态，例如“Ada Lovelace，未读动态”；环下方显示的名称以同样的方式开头，因此语音控制用户可以直接说出它。两种状态不仅颜色不同，环的宽度也不同；在强制颜色模式下，已读环以虚线绘制。图片遵循 `Avatar` 的规则（应用的路径或允许的 `https` 地址）；没有图片时显示首字母。

环只用 CSS 绘制：没有脚本，也没有图片。`stories.css`（2.2 KB）只在包含环的页面上加载。一行最多容纳 `MAX_STORY_RINGS`（100）个环。触摸、触控板和 Shift+滚轮都可以滚动它，环会吸附到位，获得键盘焦点的环会滚动到可见区域。当一行中没有任何环带链接时，由这一行本身获得焦点，这样键盘仍然可以滚动它。

| 按键 | 作用 |
|---|---|
| Tab | 移到下一个环并将其滚动到可见区域 |
| Enter | 打开当前焦点环的动态 |
| Left, Right | 焦点在行本身时：滚动该行 |
