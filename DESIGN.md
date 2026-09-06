# wit-java 设计文档 v2

> **项目已从 `wit2java` 更名为 `wit-java`，理由见 §11。** 本文档中所有
> `wit2java` 的旧称已统一替换。
>
> 相对 v1 的核心改动：定位从「填补 Java codegen 空白的独立工具」改为
> 「WIT → Java 的 **mapping specification**，附带参考实现」；mapping 章节从
> 十行对照表扩展成文档主体；新增 support package、identifier mangling、
> collision 规则、Javadoc、feature gates、determinism、role、命名与命名空间
> 等章节；async 类型明确移出 v1 范围。

---

## 1. 定位

一句话：

> **wit-java 定义 WIT → Java 的类型映射规范，并提供一个基于 Bytecode Alliance
> 官方 `wit-parser` 的参考实现，生成零运行时依赖、可直接 `javac` 的 Java 17 声明代码。**

请注意这句话里主语是**规范**，实现是附属品。

理由见 §2。简单说：一个 host-side WIT binding generator =
「Java 类型声明」+「canonical ABI lifting/lowering」。前者与 runtime 完全无关，
是纯粹可共享的部分；后者与 runtime 强绑定。如果 Endive、Chicory、wasmtime-java
各做一遍前者，Java 生态会出现三套互不兼容的 `Result<T, E>` —— 那比现在什么都没有更糟。

因此本项目的**成功判据不是 star 数，是「有第二个独立实现采纳了 mapping v1」**。
§11 的命名结构直接服务于这个判据。

### 1.1 明确的非目标

不做，且不打算做：

- WebAssembly Component Model runtime
- Canonical ABI lowering / lifting、memory layout、handle table
- component loader、host/guest glue
- TeaVM / JNI / Panama / 任何 native 依赖
- Java → Wasm 的 guest 编译路径

---

## 2. 背景：官方 Java backend 为什么烂尾

这一节不是历史考据，它直接决定本项目的设计边界，必须写在前面。

### 2.1 事实

- `wit-bindgen` 曾有 `wit-bindgen-teavm-java` backend。上游 README 已明确说明：
  TeaVM-WASI 支持长期无人维护、从未与其他 generator 达到 feature parity，因此被移除。
  最后一个带该支持的 commit 是 `86e8ae2`。
- crate 冻结在 **0.40.0**，而 `wit-bindgen` 主线目前在 **0.57.x**。
- 从 0.12.0 起，`tests/codegen.rs` 顶部就有一整块被 `// todo: implement resource
  support` 屏蔽掉的测试：`resources`、`resource_alias`、`resource_borrow_in_record`、
  `import_and_export_resource`、`same_names5` 等。**resource 支持从未实现过，
  一路 todo 到被删除。**
- 目前 `wit-bindgen` 主动维护的 guest 语言是 Rust、C、C++、C#、Go。Java 不在列。

参考：
- <https://github.com/bytecodealliance/wit-bindgen/blob/main/README.md>
- <https://github.com/bytecodealliance/wit-bindgen/issues/1010>

### 2.2 三个原因

**(a) 站错了边界（结构性）**

teavm-java 是 **guest** generator：Java → Wasm。但经 TeaVM 编译的 Java 不是 Java ——
没有完整反射、没有完整 class library、没有真正的线程。等于放弃 JVM 去换一个 component，
而 Java 生态的价值恰恰就是 JVM。真实需求一直在 **host** 侧：我有 JVM 应用，
我想安全地加载别人的 Wasm component。guest 侧的 Java 从来没有用户。

**(b) 没有 owner（组织性）**

外部贡献进来后无人认领。WIT 0.2 引入 resource 时每个 backend 都要重写，
Rust/C/C#/Go 都跟上了，Java 的 resource 测试被注释掉后再也没人回来解注释。

**(c) feature-parity treadmill（持续性）**

WIT 规范一直在动：resource 之后是 0.3 的 async / future / stream。
无人认领的 backend 永远追不上。

**这三条里没有一条是「生成 Java 声明太难」。**

### 2.3 这三条对本项目成立吗

| 原因 | 对 wit-java |
|---|---|
| (a) 站错边界 | **不成立**。本项目不碰 TeaVM，也不站 guest 侧 |
| (b) 无 owner | **成立，且更严重**（单人项目）。缓解手段见 §19 的 M0 gate 与 §11 的可移交命名结构 |
| (c) treadmill | **部分成立**，见下 |

关于 (c) 必须说准确：完整 bindgen backend 的工作量大头是 canonical ABI 的
lifting/lowering —— 每个类型的 flat 参数展开、内存布局、字符串编码、handle table。
WIT 规范的改动主要砸在这一层，本项目完全躲开。但**类型系统本身的演进躲不掉**：
0.3 的 `future<T>` / `stream<T>` / `error-context` 是新的类型构造，mapping 必须回答。

结论：抗 treadmill 能力是真实的，**前提是 v1 明确把 async 类型排除在外并且真的忍住**（§5.7）。

### 2.4 2026 年的需求证据

- Arcjet 在 2026-01 的工程博客中记录：他们需要在 JVM 上运行 Wasm component，
  但当时唯一对高层 Wasm 定义有支持的 JVM runtime 是 Chicory，且只提供 WASIp1 bindings；
  他们的剩余选项是照着自己的 WIT 定义手写 bindings。
  <https://blog.arcjet.com/running-wasm-on-the-jvm/>
- 2026-05，Bytecode Alliance 宣布 **Endive** —— Chicory 的 fork，由 BA 托管，
  增加 Cranelift 后端、零 native 依赖 —— 但**尚不支持 WASI P2 与 component model**。
  <https://wasmcloud.com/community/2026-05-27-community-meeting/>

即：BA 已把 JVM host 侧当作战略方向并投入了 runtime，而 component model 那一层还空着。
那一层的第一件事就是把 WIT 变成 Java 类型。

---

## 3. 核心设计原则

### 3.1 使用官方 wit-parser

不自行实现 WIT parser，直接依赖 `wit-parser::Resolve`，由它负责语法解析、
package/interface/world/type resolution 与依赖解析。

为降低上游 API 耦合，中间加一层很薄的 adapter：

```text
wit-parser::Resolve  →  WIT adapter  →  wit-java internal model
```

### 3.2 生成结果只依赖 Java 标准库

生成的代码必须能被 `javac --release 17` 直接编译，且 `-Xlint:all -Werror` 零 warning。
不要求 Wasmtime / Chicory / Endive / TeaVM / JNI / 任何第三方 JAR。

唯一例外是 support types（§5.8），它们由本工具自己生成或从已发布的 tiny artifact 提供。

### 3.3 declaration-only，但不是 runtime-blind

只生成声明，不生成 ABI glue。但**声明的形状必须为下游 lifting/lowering 留出扩展点**。
declaration-only 不等于可以不考虑 runtime —— 恰恰相反，如果 mapping v1 冻结的形状
让 host adapter 用不了（例如 resource 是纯 `interface`，没地方挂 native handle），
下游会自己重新生成一套，本项目当场归零。§19 的 M2 gate 就是为了防这个。

### 3.4 无法表达就报错，不猜

无法表达、存在歧义、命名冲突 → 一律 error，携带 WIT 源位置。详见 §5.10 与 §16。

---

## 4. 架构

```text
                WIT
                 │
                 ▼
      wit-parser::Resolve      (upstream)
                 │
                 ▼
           WIT adapter          (薄，隔离上游 API 变化)
                 │
                 ▼
           Java mapper          (核心，实现 mapping spec)
                 │
                 ▼
      Java declaration IR       (含 import resolve pass)
                 │
                 ▼
       Java source renderer     (deterministic)
                 │
                 ▼
              .java
```

---

## 5. Mapping specification（本项目的主体）

规范正文位于独立仓库 `wit-java-mapping`（§11），本章是它的骨架与关键决策记录。
**mapping spec 的稳定性优先于 generator 实现的稳定性**（§15）。

### 5.1 Primitive types

| WIT | Java | 说明 |
|---|---|---|
| `bool` | `boolean` | |
| `s8` | `byte` | |
| `s16` | `short` | |
| `s32` | `int` | |
| `s64` | `long` | |
| `u8` | `int` | Java `byte` 是 signed，不可用 |
| `u16` | `int` | Java `char` 虽是 unsigned 16 位但语义错误，不可用 |
| `u32` | `long` | |
| `u64` | `long` | **无符号语义丢失**，见下 |
| `f32` | `float` | |
| `f64` | `double` | |
| `char` | `int` | **不是 `char`**，见下 |
| `string` | `java.lang.String` | |

**`char` 必须映射到 `int`。** WIT 的 `char` 是 Unicode scalar value（可达 U+10FFFF，
排除 surrogate range）。Java 的 `char` 是 UTF-16 code unit，最多 U+FFFF，
映射过去会静默丢掉整个 supplementary plane。这是 correctness bug，不是风格问题。
生成的 Javadoc 必须注明取值域。

**`u64 → long` 的无符号语义丢失**是 mapping v1 唯一一处「能编译但语义会骗人」的映射。
处理方式：写进 mapping spec 的 known limitations，且每个涉及 `u64` 的声明处 emit
Javadoc，提示使用 `Long.compareUnsigned` / `Long.toUnsignedString` /
`Long.divideUnsigned`。提供 `--u64=long|BigInteger`，默认 `long`。

**`f32`/`f64`**：WIT 只有 canonical NaN，Java 的 NaN 比较语义不同。在 support
package 的 package-info 里说明一次即可，不逐处 emit。

### 5.2 List、tuple

| WIT | Java |
|---|---|
| `list<u8>` / `list<s8>` | `byte[]` |
| `list<T>` | `java.util.List<T>` |
| `tuple<A, B>` | `<support>.Tuple2<A, B>` |
| `tuple<A, ..., H>` | `<support>.Tuple3` … `Tuple8` |
| `tuple<>`（空） | `<support>.Unit` |
| `list<T, N>`（0.3 fixed-size） | **v1 不支持 → error** |

`list<u8> → byte[]` 是必须的特例，否则 `List<Integer>` 在真实 WASI 接口上是性能灾难。
其余 `list<primitive>` 不做数组特例（保持规则简单，且 `List<Long>` 在这些位置出现频率低）。

tuple 元数 > 8 → error（Java 无 variadic 泛型；真实 WIT 里 > 4 元的 tuple 极罕见，
出现基本意味着 WIT 侧应该用 record）。

### 5.3 `option<T>`

默认 `option<T> → java.util.Optional<T>`，**包括 record component 与方法参数位置**。

这违反 Java 惯例（`Optional` 官方立场是只作返回类型，且不 `Serializable`）。
接受这个代价，理由是替代方案更糟：`@Nullable T` 在 `option<option<T>>` 上会塌陷 ——
`some(none)` 与 `none` 无法区分。

提供 `--option-style=optional|nullable`：

- `optional`（默认）：一律 `Optional<T>`
- `nullable`：非返回位置用裸 `T` + `@Nullable`（support package 自有注解）；
  **遇到嵌套 `option` 直接 error**，指出嵌套位置，不猜

### 5.4 `result<T, E>`

`result` 的四种形态都要有确定映射。Java 泛型不接受 `void`，用 support package 的 `Unit`：

| WIT | Java |
|---|---|
| `result<T, E>` | `Result<T, E>` |
| `result<T>` | `Result<T, Unit>` |
| `result<_, E>` | `Result<Unit, E>` |
| `result` | `Result<Unit, Unit>` |

`Result<T, E>` 本身是 sealed interface + `Ok<T, E>` / `Err<T, E>` 两个 record，
位于 support package（§5.8）。**不映射到 Java exception**：WIT 的 error 是值不是控制流，
且 `result` 会出现在 record field 与 list 元素里，异常无法表达。

### 5.5 Record、enum、variant、flags

| WIT | Java |
|---|---|
| `record` | `record`（Java 17 record） |
| `enum` | `enum`，常量名 UPPER_SNAKE |
| `variant` | `sealed interface` + 每个 case 一个嵌套 `record` |
| `flags` | 生成 `record X(long bits)` + 静态常量 + `or`/`and`/`contains` |

**variant 的 payload-less case** → 空 record（`record Foo() implements V {}`）。
不用 singleton + `INSTANCE`：record 的值语义已经保证 `equals`/`hashCode` 正确。
case record 嵌套在 sealed interface 内部，避免污染 package 命名空间。

**`flags` 不用 `EnumSet`**：`EnumSet` 依赖 ordinal，序数不稳定，且丢掉 WIT 的位域语义
（下游 lowering 需要直接拿到 bits）。成员数 > 64 → v1 error。

### 5.6 Resource

```text
WIT resource  →  Java interface，extends AutoCloseable
```

- **`extends AutoCloseable`**：WIT resource 有隐式 drop，这是几乎免费的高价值映射。
  生成的 `close()` 必须 override 收窄为不 `throws Exception`。
- **instance method** → interface 的抽象实例方法
- **constructor** → 嵌套 `interface Factory { X create(...); }`
- **static method** → 嵌套 `interface Statics { ... }`

Factory / Statics 分离的额外好处：WIT 允许 resource 的 static method 与 instance
method 同名（不同命名空间），分离后天然不冲突。

**`own<T>` vs `borrow<T>`**：Java 没有 lifetime，两者必然映射到同一个 Java 类型。
这不是缺陷，但**必须在 spec 中显式写明「ownership 契约不在 Java 类型系统中表达，
仅通过 Javadoc 标注」**，否则下游 adapter 会做出错误假设。参数位置的 `borrow<T>`
必须 emit 一行 Javadoc 标注。

**开放问题（M2 gate 要回答）**：resource 映射成 `interface` 时，host adapter
没有地方挂 native handle。可能需要改成 `abstract class`，或在 interface 上加一个
`long handle()` accessor。**这一点在 M2 spike 之前不冻结。**
**（2026-09-06 已回答：M2 spike 通过 —— handle table 完全落在 adapter 侧，
纯 interface 方案未被证伪，见 `wit-java-mapping/spike/` 的验证记录。）**

### 5.7 Async 类型：v1 明确不支持

`future<T>`、`stream<T>`、`error-context` 在 mapping v1 中 → **error，
提示 "unsupported in mapping v1"**。

v1 文档曾提出 `future<T> → CompletionStage<T>`、`stream<T> → Flow.Publisher<T>`，
这里撤回。理由：

1. `Flow.Publisher` 不是类型声明，是 **runtime contract** —— 它带着 backpressure、
   `request(n)`、cancellation 的整套 Reactive Streams 规范义务，远超「声明」的范围，
   与 §3.3 直接冲突。
2. `CompletionStage` 没有 drop 语义，而 WIT 的 future/stream 是带 ownership 的 handle。
3. 见 §2.3：在没有用户逼迫的情况下主动接下 async，就是把 treadmill 请进门。

v2 mapping 再处理，届时倾向映射到 support package 自有的 `WitFuture<T>` /
`WitStream<T>`（形状可控），而不是标准库的 reactive 类型。

### 5.8 Support package

**这是 v1 文档最大的架构遗漏。** `Result`、`Unit`、`Tuple2..8`、`@Nullable`、
`@WitGenerated` 必须在**一个固定的 package 里只定义一次**。否则
`wasi.http.Result` 与 `wasi.io.Result` 是两个不兼容的类型，跨 package 的函数签名立刻炸。

规则：

- 默认 package：见 §11.3（与命名空间归属绑定，不是随意取的字符串）
- `--support-package <fqn>` 可覆盖
- 默认随输出一起生成；`--no-support` 假设已在 classpath（供 build plugin 复用
  已发布的 `wit-java-support` artifact）
- **support 类的内容由 mapping version 决定，与 tool version 解耦**
- **support package 的 FQN 变更是 mapping breaking change**，必须在 M4 冻结前定死

验收标准：

1. 对同一组 WIT 分两次 generate 到不同 output dir，两边 support package 的
   `.java` 字节级一致
2. 一个 world 里跨两个 WIT package 传递 `result<T, E>`，生成结果 `javac` 通过

### 5.9 Identifier mangling

WIT identifier 语法是 `word ::= [a-z][0-9a-z]*`、`label ::= word ('-' word)*`。
**kebab → camel 在 word 边界上是双射**，所以 v1 文档里 `foo-bar` 与 `fooBar`
同时存在的冲突例子不成立 —— `fooBar` 根本不是合法 WIT identifier。

转换规则：

| 位置 | 形式 |
|---|---|
| type、interface、resource、variant case | UpperCamel |
| method、record component、function 参数 | lowerCamel |
| enum 常量、flags 常量 | UPPER_SNAKE |

Mangling 规则（一律追加后缀 `_`，确定性）：

1. **Java 关键字与保留字**。WIT 中 `%class` `%package` `%import` `%new` `%static`
   `%int` 等都是合法 identifier。→ `class_`、`package_` …
2. **`java.lang.Object` 的 final 方法**：`wait`、`notify`、`notifyAll`、`getClass`。
   在 interface 中声明 `void wait()` 会**直接编译失败**（`Object.wait()` 是 final）。
   → `wait_`、`notify_` …
3. **record accessor 撞 Object 方法**：component 名为 `to-string` / `hash-code` /
   `equals` 时，生成的 accessor 若返回类型不兼容会编译失败（`int toString()`）。
   → 一律 mangle 这组名字，不做签名兼容性判断（规则简单可预测优先）。
4. **`_` 结尾仍冲突** → 继续追加，`class__`；若与已有映射结果冲突 → error（§5.10）。

保留名清单固化在 mapping spec 中，加入清单是 breaking change。

### 5.10 Collision：必须 error 的情形

以下全部是可写测试的硬错误，退出码非 0，错误信息必须指向 WIT 源位置：

1. 两个 WIT item 映射到同一个 Java FQN（最常见来源：跨 interface 的同名 type
   flatten 到同一 Java package，见 §6.2）
2. mangling 产生的名字与另一个已映射名字冲突
3. tuple 元数 > 8；flags 成员数 > 64
4. v1 不支持的类型构造：`future`、`stream`、`error-context`、fixed-size `list<T, N>`
5. `--option-style=nullable` 下出现嵌套 `option`
6. package 映射后两个 WIT package 落到同一 Java package 且含同名 item

### 5.11 Doc comments → Javadoc

**v1 文档完全没提，但这是 declaration-only 工具最大的用户价值** ——
在 IDE 里能看到 WIT 的文档。

规则：

- WIT doc comment → Javadoc，保留原文换行
- HTML escape `<` `>` `&`；`@` 在行首时转义为 `&#64;`，否则会被当成 Javadoc tag
- 自定义标注写在正文中而非 `@` tag（避免 doclint 报错）：`borrow<T>` 参数、
  `u64` 无符号语义、`char` 取值域
- 每个生成文件头部：`// @generated by wit-java <tool-ver>, mapping v1 — DO NOT EDIT`

验收标准：`javadoc -Xdoclint:all` 跑 wasi-http 全量生成结果，零 error。

---

## 6. WIT package → Java package

### 6.1 版本段

`ns:pkg@x.y.z` → `<root>.<ns>.<pkg>.<version-segment>`

version segment 按 semver 兼容区间取，而不是简单截取：

- `major > 0` → `v{major}`（`1.2.3` → `v1`）
- `major == 0` → `v0_{minor}`（`0.2.3` → `v0_2`，因为 0.x 的 minor 是 breaking）
- pre-release → 追加 sanitize 后的段（`0.3.0-draft` → `v0_3_draft`）。
  WASI 大量使用 pre-release，不能忽略

示例：`wasi:http@0.3.0-draft` → `wasi.http.v0_3_draft`

### 6.2 Interface 粒度（v1 文档未回答）

`--interface-package-style`：

- `nested`（默认）：每个 WIT interface 生成自己的子 package。
  `wasi:http@0.2/types` → `wasi.http.v0_2.types`。
  **天然消除跨 interface 的同名 type 冲突**
- `flat`：所有 interface 的 type 落到 WIT package 对应的 Java package。
  更紧凑、手写 host 代码时可读性更好，但同名即 error（§5.10.1）

### 6.3 显式覆盖

`--package-map <file>`，TOML：

```toml
"wasi:http" = "org.example.wasi.http"
"wasi:io@0.2" = "org.example.wasi.io.v2"
```

匹配优先级：带版本的精确匹配 > 不带版本 > 默认规则。

---

## 7. World 与 role（v1 文档的语义缺口）

`import foo` 对 host 意味着「我要**提供** foo」，对 guest 意味着「我要**调用** foo」。
v1 文档的 `World { Imports, Exports }` 没有回答方向，下游拿到会懵。

`--role host|guest|both`（默认 `both`）：

- **interface 本身的形状与 role 无关**（interface 是无方向的），role 只影响
  world 聚合类的形状与 Javadoc 措辞
- `host`：`Imports` 是「你要实现的」，`Exports` 是「你可以调用的」
- `guest`：反之
- `both`：两套聚合类都生成，放在不同子 package

---

## 8. Java Declaration IR

不做 `wit-parser AST → StringBuilder → .java`。中间放一个小的 Java declaration model：

```text
JavaProject
 └─ JavaFile
     ├─ package
     ├─ imports          (由 import resolve pass 填充)
     └─ declarations
         ├─ Interface
         ├─ Record
         ├─ Enum
         └─ SealedInterface
```

**关键：IR 与 renderer 之间必须有一个 import resolve pass。** 当两个不同 package
的类 simple name 相同时，必须自动降级为 FQN。这是整条链路里最容易出 bug 的地方，
v1 文档没提。

好处：mapping 与 rendering 彻底分离，未来的 Java 8 renderer / Kotlin backend /
不同 formatting 都只是换 renderer（命名结构如何支持这一点见 §11.2）。

---

## 9. Java 版本基线

**Java 17**。理由：`record` 与 `sealed interface` 是 `record`/`variant` 映射的基础。
不为 Java 8 兼容性增加 generator 复杂度；确有需求时再加 `--java-version 8` renderer。

不需要 Java 21：declaration-only 不生成 `switch`，用不到 pattern matching。
`Flow` 是 Java 9，但 §5.7 已把它移出 v1。

---

## 10. CLI

```bash
wit-java generate <wit-path> --out <dir> [options]
wit-java check    <wit-path>            [options]     # 不写文件
wit-java mapping-info --mapping-version v1            # 打印映射表
```

Options：

```text
--out <dir>
--package-root <fqn>                       默认无前缀
--package-map <file>
--interface-package-style nested|flat      默认 nested
--mapping-version v1                       默认最新 stable
--support-package <fqn>                    默认见 §11.3
--no-support
--option-style optional|nullable           默认 optional
--u64 long|BigInteger                      默认 long
--role host|guest|both                     默认 both
--features <name>...                       feature gate，可重复
--all-features
```

### 10.1 Feature gates（v1 文档遗漏）

wasi-http / wasi-cli 大量使用 `@since(version = ...)` 与 `@unstable(feature = ...)`。
`Resolve` 默认会 skip unstable items，第一次跑真实 WASI 就会发现生成结果莫名其妙缺东西。
`--features` / `--all-features` 是必需项，不是 nice-to-have。

---

## 11. 命名与命名空间

### 11.1 为什么不叫 `wit2java`

三个理由，前两个是定位问题，第三个是陷阱：

1. **`X2Y` 命名的是工具，不是产品。** §1 说产品是 mapping spec，实现是附属品。
   `dos2unix` / `pdf2text` 这一类构词传达的是「一次性转换脚本」，与「规范」是反义的。
   BA 生态里没有一个项目这么命名：`wit-bindgen`、`wit-parser`、`wasm-tools`、
   `jco`、`componentize-js`、`wac`、`wizer`。
2. **它把 Java 焊死在仓库名里。** §8 已预留 Kotlin renderer 的可能，而 Kotlin
   共享同一份 WIT 语义模型与大部分 mapping 决策（`Result`、`variant`、resource）。
   用 `wit2java` 命名，Kotlin backend 只能另开仓库重实现一遍。
3. **不要用那个「显而易见」的替代名 `wit-bindgen-java`。** 它占用 BA 的命名空间
   （若 BA 后来要做官方 Java backend，你在 squatting），且 `bindgen` 承诺的是
   完整绑定（含 lifting/lowering），而本项目明确不做那一半 —— 名字过度承诺，
   用户拿到手发现只有声明，第一印象就是负的。

同理排除：`jwit` / `witj`（不可读不可搜）、`javawit`（读起来是反方向）、
植物类代号（Chicory → Endive 是 Dylibso/BA 那条线的传承，跟名会被误认为 fork，
且完全不可搜索 —— `jco` 能用短名是因为背后有 BA 在推分发，本项目没有那个渠道，
可搜索性是唯一获客途径）。

### 11.2 命名结构：spec 与实现分仓

**规范与实现分成两个仓库。** 这不是洁癖：§1 的成功判据是「有第二个实现采纳
mapping v1」，那么规范必须能被独立引用。一份躺在实现仓库 `docs/` 子目录里的文档，
Endive 要采纳就得说「我们遵循某个第三方工具仓库里的一个子目录」，组织上很别扭；
而独立仓库可以被直接 cite，将来也可以整体移交 BA 而不牵扯本项目的 Rust 代码
（这同时是 §2.3 死因 (b) 的缓解手段之一）。

| 角色 | 名称 | 内容 |
|---|---|---|
| 规范 | **`wit-java-mapping`** | 纯文档仓库：`v1.md`、`v2.md`、conformance test 数据 |
| 实现 | **`wit-java`** | Rust workspace，参考实现 |
| 支持库 | **`wit-java-support`** | Maven artifact，配合 `--no-support` |

`wit-java` 作为实现名：符合 BA 的连字符惯例；可搜索（google "wit java" 能撞上）；
不含 `bindgen` 因而不过度承诺；也不像 `wit2java` 那样自我矮化。

Rust workspace 内部：

```text
wit-java-core     generator 全部逻辑（library）
wit-java-cli      薄壳，产出 binary `wit-java`
```

将来若做 Kotlin：`wit-kotlin` + 从 `wit-java-mapping` 抽出共享部分为
`wit-jvm-mapping`。命名结构能长大。

**crates.io 可用性已核实（2026-09）**：`wit-java`、`wit-java-core`、
`wit-java-cli`、`wit-java-mapping` 均未被占用。

### 11.3 Java package 的归属问题

`dev.wit-java.support` 这类写法要求**真的拥有对应域名**，否则不符合反向域名惯例，
Maven Central 也不会签发该 groupId。

**在 M0 gate 通过之前不买域名。** 用 GitHub 反向域名，Sonatype 正式接受：

```text
groupId     io.github.<gh-user>
artifactId  wit-java-support
support pkg io.github.<gh-user>.wit.java.support
```

`--support-package` 的默认值即上述 support pkg。

迁移路径：项目确实活下来、或进入 BA 之后，再迁到真域名或 `org.bytecodealliance`。
**但 support package 的 FQN 变更是 mapping breaking change**（§5.8），
因此这个决定最迟必须在 M4 冻结 mapping v1 之前定死，不能拖到 1.0 之后。

### 11.4 命名空间占位清单（M0 的一部分）

M0 尚未跑完，但命名空间是廉价的，先占住：

- [ ] crates.io：`wit-java`、`wit-java-core`、`wit-java-cli`
      各推一个 `0.0.0` 占位版本
- [ ] crates.io：`wit-java-mapping`（即使规范仓库不产出 crate，防止被占）
- [ ] GitHub：`wit-java`、`wit-java-mapping` 两个空仓库（README 写清定位）
- [ ] Maven Central：完成 `io.github.<gh-user>` groupId 的 Sonatype 验证
      （走 GitHub 验证，几分钟）
- [ ] **不占域名。** 等 M0 有结果

---

## 12. Library API 与分发

```text
wit-java-core     Rust library，generator 全部逻辑
wit-java-cli      薄壳，binary 名 `wit-java`
wit-java-support  已发布的 Java tiny artifact（配合 --no-support）
```

CLI、Maven plugin、Gradle plugin、IDE 集成全部复用同一个 `wit-java-core`。

### 12.1 跨语言边界（v1 文档跳过了）

Maven / Gradle plugin 是 JVM 的，无法 link Rust。第一版的答案：
**plugin 只 wrap 预编译 CLI binary**，通过 classifier jar 分发，参考 protoc-jar 模式。

Binary 矩阵：`linux-x86_64` / `linux-aarch64` / `macos-x86_64` / `macos-aarch64` /
`windows-x86_64`。

Panama FFM 直接加载 cdylib 是更优雅的方案，但需要 Java 22+，留给后续版本。

---

## 13. Maven / Gradle plugin

第一版不实现。之后：

```text
build system → wit-java CLI binary → generated-sources
```

**不重新实现 mapping**，也不重新实现 IR 或 renderer。

---

## 14. 测试策略

核心 invariant：

```text
WIT → wit-java → Java → javac → SUCCESS（零 warning）
```

### 14.1 测试分类

**Mapping tests** —— 单个 WIT 类型构造 → 预期 Java 片段。

**Golden tests** —— 完整 `.java` 文件对比，带 `--bless` 更新机制。
没有 `--bless` 就没人愿意在改 mapping 时维护 golden。

**Compile tests**：

```bash
javac --release 17 -Xlint:all -Werror $(find out -name '*.java')
```

**Javadoc tests**：

```bash
javadoc -Xdoclint:all <生成的所有包>    # 零 error
```

**Determinism tests** —— 同一输入在 Linux / macOS / Windows 各跑两次，
六份输出 `sha256` 全等。要求 renderer 显式排序（文件顺序、import 顺序、
declaration 顺序），换行统一 `\n`，不 emit 时间戳。

**Negative tests** —— §5.10 的六类 collision，每类至少一个 case，
要求非 0 退出码 + 指向 WIT 源位置的错误信息，**而不是**生成一份编译不过的 Java。

### 14.2 真实 corpus

| package | 覆盖点 |
|---|---|
| `wasi:cli@0.2.x` | 基础、world/role |
| `wasi:http@0.2.x` | resource 密集、feature gates |
| `wasi:filesystem@0.2.x` | **flags 密集** |
| `wasi:sockets@0.2.x` | **tuple + resource 密集** |
| 一个 0.3 async world | 验证 §5.7 的 error 路径正确触发 |

---

## 15. Mapping versioning

需要稳定的不是 Rust 实现，是 **WIT → Java API shape**。

落地机制（v1 文档只有口号，这里具体化）：

- 文件头：`// @generated by wit-java <tool-ver>, mapping v1 — DO NOT EDIT`
- support package 提供自有的 `@WitGenerated(mapping = "v1")`
  （不用 `javax.annotation.processing.Generated`，避免引入依赖）
- `--mapping-version v1`，**默认锁定最新 stable 而非最新**
- `wit-java check --mapping-version v1` 供 CI
- **演进规则：mapping vN 只能追加，不能修改。** 新的 WIT 类型构造在 vN 中报
  `unsupported in mapping vN`，而不是偷偷给一个映射
- **support package FQN 属于 mapping 的一部分**，其变更是 breaking change（§11.3）

工具版本与 mapping 版本正交：`wit-java 1.4` 可以同时支持 `mapping v1` 与 `v2`。

---

## 16. 错误处理原则

```text
无法表达   → error
存在歧义   → error
命名冲突   → error
```

所有 error 必须携带 WIT 源位置（`Resolve` 提供）。
基础设施 code generator 宁可拒绝生成，也不生成一份「能编译但语义错」的产物。

---

## 17. 项目边界与生态位置

```text
                        wit-java-mapping
                          (规范，可独立引用)
                                │
                                ▼
                            wit-java
                    (参考实现：WIT → Java declarations)
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
   Endive adapter        Chicory adapter        wasmtime-java adapter
  (lifting/lowering)    (lifting/lowering)      (lifting/lowering)
        │                       │                       │
        └───────────────────────┴───────────────────────┘
                     共享同一套 Java API shape
```

core 不依赖任何 adapter。但 **mapping v1 冻结前必须至少验证一个 adapter 能用**（§19 M2）。

---

## 18. 与 java2wit 的关系

反方向做成独立项目 `java-wit`（同样避免 `X2Y`）：

```text
Java source → JavaParser/JDT → Java declaration model → WIT
```

不把 wit-java 膨胀成双向 compiler。两边共享 `wit-java-mapping`。

---

## 19. Roadmap 与 go/no-go gate

### M0 — 生态确认 + 命名空间占位（写代码之前，约一周）

死因 (b) 的缓解手段是**在开工前确认没有人从另一头把同一件事做完**。

1. **BA Zulip 询问 Endive 的 component model 计划**：时间表、打算如何生成 Java 类型、
   是否愿意把类型映射抽成独立 spec。
   - **判定线**：若「已有人在做完整 bindgen」→ 改为向其提 PR 或放弃，不平行造轮子。
     若「还没人碰」或「欢迎」→ 绿灯。
2. **联系 Arcjet 博客作者**：如果有工具生成 WIT 的 Java 类型声明，但 lifting/lowering
   仍需自写，是否有价值。
   - **判定线**：若答案是「要全套，半套没用」→ 说明边界划错了，
     Chicory/Endive adapter 必须进 v1 范围，而不是像 §17 那样推给未来。
3. **完成 §11.4 的命名空间占位清单。** 这一项不设判定线，无论 1/2 结果如何都做完
   —— 成本几乎为零，而名字被抢的返工成本很高。

### M1 — mapping spec v0 draft（先文档，后代码）

在 `wit-java-mapping` 仓库把 §5 展开成完整的 `v1.md`，然后**手工**过一遍
wasi-filesystem 与 wasi-sockets 的 WIT，检查这张表能否覆盖它们用到的每一个类型构造。

- **验收**：不存在「表里没有」的类型构造；每个 known limitation 都有明确记录

### M2 — Runtime 可用性 spike（mapping 冻结的前置条件）

挑一个含 resource 的小 world（如 `wasi:io/streams` 子集），手写理想中的 Java
declaration，再手写 Chicory 或 Endive 的 lowering 去实现它。

- **验收**：lowering 层**不修改任何一行 declaration**即可工作
- 做不到 → 说明 §5.6 的开放问题（interface vs abstract class vs handle accessor）
  尚未定型，**mapping v1 不得冻结**

### M3 — 参考实现

adapter → mapper → IR → renderer，配合 §14 的全套测试。

### M4 — mapping v1 冻结 + 1.0

冻结条件：

- M2 通过
- §14.2 全部 corpus 通过 compile + javadoc + determinism
- **support package FQN 已最终确定**（§11.3）

### M5 — Maven / Gradle plugin，binary 分发

---

## 20. 总结

v1 文档的架构分层是对的，边界收得也对，问题在于**篇幅分配与项目实际难度不匹配**：
架构图占了三成，mapping 不到一成，而项目的全部难度在 mapping。

v2 的四个关键改动：

1. **定位**：从「独立工具」改为「共享 mapping spec + 参考实现」。
   成功判据是有第二个实现采纳，不是 star 数。
2. **mapping 展开**：修正 `char` 的 correctness bug，补全整数宽度、flags、tuple、
   borrow、support package、identifier mangling、collision 规则、Javadoc、
   feature gates、determinism；撤回 async 类型的过度承诺。
3. **命名**：`wit2java` → `wit-java`，规范独立成 `wit-java-mapping` 仓库。
   命名结构服务于「可被第二个实现采纳、可移交」这个目标，而不只是好看。
4. **前置 gate**：M0 生态确认与 M2 runtime spike 都在写代码或冻结 mapping 之前。
   teavm-java 死于无人认领；本项目最可能的死法是有人从另一头把同一件事做完了。
