# 소셜

소셜 페이지를 위한 컴포넌트입니다. 각 컴포넌트는 서버에서 렌더링되며, 해당 컴포넌트를 사용하는 페이지에서만 스타일시트와 스크립트를 불러옵니다.

## StoryRing

`StoryRing`은 어떤 사람의 스토리가 보는 사람에게 새로운지 알려 주는, 링 안의 아바타입니다. `StoryState::Unseen`은 브랜드 색의 두꺼운 링을, `StoryState::Seen`은 얇은 중립색 링을 그립니다. 상태는 서버에서 오며, 앱이 보는 사람에 대해 저장해 둔 값에서 정해집니다. 브라우저는 이를 바꾸지 않습니다. `StoryRow`는 링을 레일처럼 옆으로 스크롤되는 한 줄에 배치합니다.

```rust
StoryRow::new()
    .add(StoryRing::new(ada.name.clone(), StoryState::Unseen)
        .image(ada.photo.clone())
        .link(stories, StoryParams { person: ada.id }))
    .add(StoryRing::new(alan.name.clone(), StoryState::Seen).image(alan.photo.clone()))
```

`.link(screen, params)`를 지정하면 링은 스토리를 보여 주는 화면으로 가는 링크가 됩니다. 접근 가능한 이름은 사람의 이름과 상태이며, 예를 들어 "Ada Lovelace, 읽지 않은 스토리"와 같습니다. 링 아래에 보이는 이름도 같은 방식으로 시작하므로 음성 제어 사용자가 그대로 말할 수 있습니다. 두 상태는 색뿐 아니라 링의 두께로도 구분되며, 강제 색상 모드에서는 본 링이 점선으로 그려집니다. 이미지는 `Avatar` 규칙을 따릅니다(앱의 경로 또는 허용된 `https` 주소). 이미지가 없으면 이니셜이 표시됩니다.

링은 CSS만으로 그려집니다. 스크립트도 이미지도 없습니다. `stories.css`(2.2 KB)는 링이 있는 페이지에서만 불러옵니다. 한 줄에는 최대 `MAX_STORY_RINGS`(100)개의 링이 들어갑니다. 터치, 트랙패드, Shift+휠로 스크롤할 수 있고, 링은 제자리에 맞춰 멈추며, 키보드 포커스를 받은 링은 보이는 영역으로 스크롤됩니다. 한 줄의 링 중 링크가 있는 것이 없으면 줄 자체가 포커스를 받으므로 키보드로 계속 스크롤할 수 있습니다.

| 키 | 동작 |
|---|---|
| Tab | 다음 링으로 이동하고 보이는 영역으로 스크롤합니다 |
| Enter | 포커스된 링의 스토리를 엽니다 |
| Left, Right | 줄 자체에 포커스가 있을 때: 줄을 스크롤합니다 |
