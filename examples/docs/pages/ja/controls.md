# コントロールとピッカー

フォームのフィールドは、その型に応じてコントロールを選びます。`.control(M::f, Control::...)` はそれを変更し、`.lookup(M::f, action)` はフィールドをサーバーに対する検索に変えます。すべてのコントロールは、内部ではネイティブなフォーム要素なので、ランタイムがあってもなくても値は同じように送信され、すべてのコントロールは、キーボードやスクリーンリーダーのユーザーのために ARIA パターンに従います。

```rust
Form::of(Contact { quantity: 2, size: Size::Medium, ..Contact::default() })
    .lookup(Contact::product, find_samples)
    .row(|r| r.field(Contact::delivery).field(Contact::size))
    .control(Contact::delivery, Control::Radio)
    .control(Contact::size, Control::Segmented)
    .field(Contact::newsletter)
    .control(Contact::newsletter, Control::Switch)
    .field(Contact::note)
    .submit(t!("docs.live.send"), send_contact)
```

@live controls

## 既定のコントロール

| フィールドの型 | コントロール |
|---|---|
| `String` | テキスト入力。`max_len` が 255 を超える場合は、文字数カウンター付きで伸びるテキストエリア |
| `i32`, `i64` | `min` と `max` のルール付きの数値入力 |
| `Money` | ユーザーの数値形式による金額と通貨 |
| `Decimal` | ユーザーの形式による正確な数値で、フィールドの `scale` に従います（[Decimal 入力](#decimal-and-percent) を参照） |
| `bool` | チェックボックス |
| `#[choice]` の列挙型 | セレクト。検索できるリストを開くボタン |
| `#[choice]` の列挙型の `Vec` | 複数選択。チップと、複数の選択肢を選べるリスト |
| `Date`, `Time`, `DateTime`, `LocalDateTime` | 日付と時刻のピッカー |
| モデルの `Vec<M>` | 明細エディタ（[明細エディタ](lines.md) を参照） |

## その他のコントロール

| コントロール | 用途 |
|---|---|
| `Control::Radio` | 選択肢が少なく、すべてを表示したい選択 |
| `Control::Segmented` | 短い選択肢が 2 つから 5 つの選択 |
| `Control::Listbox` | 入力して検索できる長い選択リスト |
| `Control::Switch` | 設定として即座に反映される `bool` |
| `Control::Percent` | パーセント記号付きで表示される `Decimal` または整数 |
| `Control::Password`, `Control::NewPassword` | パスワードとして入力されるテキストフィールド。表示と非表示のボタン付き |
| `Control::PasswordMeter` | 強度メーター付きの新しいパスワード |
| `Control::Rating` | 1 から N までの整数を星で表示します（[Rating](#rating) を参照） |

## Rating

`Control::Rating` は整数フィールドを 1 から N までの星として表示します。N はフィールドの `max` ルールです（ルールがなければ 5、最大 10）。凡例がフィールドのラベルである `fieldset` の中にあるネイティブのラジオボタンのグループなので、スクリプトなしで W3C APG のラジオグループパターンに従います。Tab はグループに一度だけ到達し、矢印キーで移動と選択を行い、各星はユーザーの言語で「3 / 5」のように名前が付きます。ホバーするとポインターの位置までの星が点灯します。サーバーは他の数値と同様にフィールドのルールで値を検証するので、フィールドに `min` と `max` を指定してください。`Option<i64>` に `required` がある場合、すべての星を空にすることは拒否されます。

```rust
#[model]
pub struct Feedback {
    #[rule(required, min = 1, max = 5)]
    pub stars: Option<i64>,
}

Form::of(Feedback::default())
    .field(Feedback::stars)
    .control(Feedback::stars, Control::Rating)
    .submit(t!("feedback.send"), send_feedback)
```

@live rating


## ルックアップ

ルックアップは、ユーザーが入力するたびにサーバー上でレコードを検索します。アクションは `LookupQuery` を受け取り、`Page<LookupItem>` を返します。フィールドが保存するのは ID です。

```rust
#[action(permission = "stock.product.read")]
pub async fn find_samples(q: LookupQuery, store: SampleStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|s| LookupItem::new(s.id.0, s.name).hint(s.code)).collect();
    Ok(q.apply(items))
}
```

`.lookup_many(M::f, action)` は、`Vec<i64>` フィールドに対して同じことを行い、選ばれたものを削除可能なタグとして表示します。

`.lookup_columns(M::f, action, [t!("code"), t!("name"), t!("city")])` は、リストに複数の列を表示します。検索は引き続きサーバー上のアクションに送られ、キー入力ごとの検索でその権限が検査されます。各項目は `LookupItem::cells` で、見出しと同じ順序で値を渡します。`LookupQuery::apply` はセルも検索します。スクリーンリーダーは、各選択肢を「Code: C-2, Name: Mert Kaya, City: Ankara」のように読み上げます。狭い画面では、列は各選択肢の中で縦に積まれます。

```rust
#[action(permission = "sales.customer.read")]
pub async fn find_customers(q: LookupQuery, store: CustomerStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|c| LookupItem::new(c.id.0, c.name.clone()).cells([c.code, c.name, c.city]));
    Ok(q.apply(items.collect()))
}
```

## 複数選択

`#[choice]` の列挙型の `Vec` は、複数選択で編集します。選ばれた選択肢は削除ボタン付きのチップとして表示され、コンボボックスが、選択肢のオンとオフを切り替えるリストを開きます（`aria-multiselectable` を持つ APG の listbox を、コンボボックスの中に置いたもの）。内部にはネイティブの `<select multiple>` があるため、ランタイムがなくてもフィールドは動作します。ランタイムは値をカンマでつないで送信し（`Email,Sms`）、サーバーはそれを 1 つずつ読み取り、未知の値を拒否し、重複を取り除きます。JSON（API、AI ツール、データベース）では、フィールドは選択肢の値の配列です。

```rust
#[model]
pub struct Contact {
    #[rule(required)]
    pub channels: Vec<Channel>,
}

Form::of(contact).field(Contact::channels)
```

@live multi

| キー | 動作 |
|---|---|
| 下矢印、上矢印、Enter、Space | 閉じているコンボボックスでリストを開きます |
| Home、End | 最初または最後の選択肢でリストを開きます |
| 下矢印、上矢印 | 開いているリスト内を移動します |
| Shift と下矢印または上矢印 | 移動し、次の選択肢のオンとオフを切り替えます |
| Space、Enter | アクティブな選択肢のオンとオフを切り替えます。リストは開いたままです |
| 文字キー | その文字で始まる次の選択肢へ移動します |
| Backspace、Delete | 閉じているコンボボックスで、最後に選んだ選択肢を削除します |
| Escape、Alt と上矢印 | リストを閉じます。フォーカスはコンボボックスに残ります |
| Tab | リストを閉じて、次へ移動します |

Ctrl+A（APG パターンの「すべて選択」）は、ショートカットのポリシーが求めるとおり、ブラウザに任せています。チップの削除ボタンはタブ順に含まれます。リスト、チップ、ルックアップの列には、約 1.7 KB（gzip）のスクリプトが必要で、リクエストは不要です。フィールドは 3 ノードに、選ばれた選択肢ごとに 1 ノードが加わります。

## Decimal とパーセント

`Decimal` フィールドは正確な数値を保持します。入力、サーバー、データベース、エクスポートのどこにも浮動小数点は使われません。入力欄は、入力中にユーザーの言語で数値を整形し（トルコ語では `1.234,5678`、英語では `1,234.5678`）、フィールドの `scale` の桁数までの小数を許可します。サーバーは同じ言語でそれを読み取り、`scale`、`precision`、`min`、`max` を再度検査します。

```rust
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
}

Form::of(terms).field(Terms::rate).field(Terms::discount).control(Terms::discount, Control::Percent)
```

`Control::Percent` は値をパーセントポイントで保持します。`12.5` は、英語では `12.5 %`、トルコ語では `%12,5` と表示され、`12.5` として保存されます。スクリーンリーダーには、入力欄の説明を通して「percent」という語が読み上げられます。

## マスク入力

`mask` ルールを持つテキストフィールドは、ユーザーの入力中にそのパターンを表示し、生の文字だけを送信します。パターンがルールそのものです。サーバーは入力されたリテラルを取り除き（`(532) 123 45 67` は `5321234567` になります）、`A` の位置を大文字にし、すべての位置を埋めていない値を拒否します。API の呼び出しと AI ツールは生の値を送り、同じ検査を受けます。

```rust
#[model]
pub struct Contact {
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
}
```

| パターンの文字 | 意味 |
|---|---|
| `9` | 数字 |
| `a` | 文字 |
| `A` | 文字。大文字で書かれます |
| `*` | 文字または数字 |
| バックスラッシュ | 次の文字はリテラルです。たとえば `\T` |
| それ以外 | リテラル。表示されますが、保存されません |

ランタイムがなければ、フィールドは生の値を持つ通常のテキスト入力になります。ユーザーはリテラルを付けても付けなくても入力できます。

## パスワード

`Control::Password`（サインイン）、`Control::NewPassword`（サインアップ、変更）、`Control::PasswordMeter`（強度メーター付きの新しいパスワード）は、サーバーから値が入力されることも、下書きに保存されることもないパスワード入力です。表示と非表示のボタンは、APG のトグルボタンのパターンに従います。名前は「Show password」のままで、`aria-pressed` がテキストが表示されているかどうかを伝えます。フォームは、送信時にパスワードを再び非表示に戻します。

メーターは、長さ、文字の種類、明らかなパターンからブラウザ内で計算されるヒントで、4 本のバーとテキスト（「Strength: good」）が丁寧なライブリージョンに表示されます。サーバーが何を受け付けるかは、フィールドのルール（`min_len`、モデルの `validate` 関数）によって決まり、メーターによって決まることはありません。

@live inputs

### キーボード

| キー | 場所 | 動作 |
|---|---|---|
| 上矢印、下矢印 | Decimal とパーセントの入力 | `min` と `max` の範囲内で整数 1 単位ずつ増減します |
| Page Up、Page Down | Decimal とパーセントの入力 | 10 ずつ増減します |
| Backspace | マスク入力 | キャレットの前の文字を削除します。リテラルの上では、リテラルの前の文字を削除します |
| Tab | パスワード入力 | 表示と非表示のボタンへ移動します |
| Enter、Space | 表示と非表示のボタン | パスワードを表示または非表示にします。フォーカスはボタンに残ります |

### 予算と検査

| | |
|---|---|
| 追加されるスクリプト | `components.js` にマスク、トグル、メーター用として約 2 KB（gzip）、生で 7 KB。Decimal とパーセントの入力は数値コントロールを使います |
| DOM ノード | マスク入力で 3、トグル付きのパスワードで 4、メーター付きで 11 |
| リクエスト | 入力中はありません |
| サーバーの検査 | `scale`、`precision`、`min`、`max`、`mask` は、ページ、API、AI ツールのいずれからでも、保存のたびに実行されます |

## 日付と時刻のピッカー

ピッカーは、ユーザーの形式による入力を受け付け、カレンダーまたは時刻のリストを開きます。`DateOptions` で制限を設定します。`.no_weekends()`、`.week_numbers()`、`.step_minutes(15)` です。`.range(from, to)` は、2 つの日付フィールドを、*今月* のようなプリセットを持つ 1 つの範囲ピッカーにまとめます。ライブ例は [時刻と言語](time-and-languages.md#pickers) にあります。
