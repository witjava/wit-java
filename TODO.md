# wit-java 实施计划

> **进度**（2026-09-06，B→E 全程完成，暂停于发布动作前）：
> - ✅ A 骨架 / B（spec 全文 + 人工验证 + conformance + Tier1 checker）
>   / C（M2 spike **通过**，spec §5.11 解除冻结阻塞）
> - ✅ D 全部（D1–D10）：实现 + conformance 正例/负例全绿 + golden(BLESS=1)
>   + determinism + WASI corpus（合并过 `javac 17 -Werror` 与 doclint 零警告）
>   + naming 单测 + CI(workflows 就绪)，clippy 零警告，fmt 干净
> - ✅ E-ready：冻结的机器可验条件全部满足。
> - ✅ 已发布（2026-09-06，private）：`witjava/wit-java`（b5a7e3b）与
>   `witjava/wit-java-mapping`（fb8cb4f）已推送，main 分支，CI 已自动触发。
> - ✅ 复审加固轮（2026-09-06，详见两仓 CHANGELOG [Unreleased]）：
>   修复 inline world interface 整体被丢弃、interface 声明与同名成员类型
>   静默覆盖（真实 WASI 的 error/network/terminal-* 触发，spec §6 补规则）、
>   包/世界段保留字、`--package-map@x.y`、`--u64=BigInteger` 的 long 注、
>   `close` 撞名、`*/` 注入；生成 Javadoc 不再造填充句；用法错误不再冒用
>   WJ 码；conformance 21 case（14 正 + 7 负）；`wasi-cli-0.3-rc` 实为
>   upstream v0.2.7 快照，已改名 `wasi-cli-0.2.7` 并修正 VERSIONS.md；
>   javadoc 验收加 `-Werror`；CI latest-JDK 步骤不再吞错误。
> - ✅ 二轮加固（2026-09-06）：修复 `--option-style=nullable` 生成的
>   `@Nullable` 缺 import（javac 直接 cannot find symbol；新增 nullable +
>   BigInteger 风格的 corpus 编译回归测试）；修复空 interface（无类型无函数，
>   或全部被 feature gate 过滤）被静默丢弃而 world accessor 仍引用它
>   （spec §2/§6 本就要求恒发；新增 `empty-interface` conformance case，
>   现 22 case：15 正 + 7 负）；interface FQN 内部不变量失败改为报诊断
>   而非静默跳过 world 聚合；源码注释中误标为 "spec §" 的 DESIGN 节号
>   已更正；DESIGN §11.3 补 Q1 决议注（support FQN 定为
>   `io.github.witjava.support`）。
> - 剩余发布动作：crates.io `0.0.0` 占位 → Sonatype `io.github.witjava` 验证
>   （发 1.0 前 repo 需转 public）→ mapping 仓打 `mapping/v1` tag → wit-java 发 1.0
> - 实现期写回 spec 的规则：泛型装箱（§5.1.1）、interface 声明恒发（§6）、
>   interface 声明 mangle（§6）、package-info 注解 FQN 形式（§9）、
>   map rationale 修正（§5.12）、`*/` 转义与空行丢弃（§9）

本文件是 `DESIGN.md` 的执行拆解。章节引用（§N）一律指 `DESIGN.md`。
第 1 章「决策记录」是对 DESIGN.md 的**修订与补全**，冲突时以本章为准。

两个本地目录已就位，先在本地开发，稍后 push 成两个 GitHub 仓库：

```text
~/code/wit-java            实现（Rust workspace）+ 本文件
~/code/wit-java-mapping    规范 + conformance 数据 + checker
```

---

## 0. 排期判断（与 §19 的差异）

§19 把 M0（生态确认）放在「写代码之前」。保留，但拆细，否则会变成
「等两封邮件回复，一个月什么都没干」：

- **M0-1 / M0-2 是外部异步项，gate 的是「投入 M3 实现」，不是 gate「动手」。**
  尽早发出，M3 开始前收到答复即可。
- **M1（mapping spec draft）不依赖 M0 答复。** 即使 M0 判红灯（已有人在做完整
  bindgen），M1 的产物仍然有用 —— 它正是向对方提 PR 或提案时要拿出的东西。
- **M0-3（命名空间占位）无判定线，立刻做完。**

本地开发顺序：**A（骨架）→ B（M1 规范）→ C（M2 spike）→ D（M3 实现）**，
M0 的外部询问在 A 阶段同时发出。

---

## 1. 决策记录

### 1.1 已决（Q 系列，2026-09-06 讨论确认）

| # | 决定 | 依据 |
|---|---|---|
| **Q1** | **support package = `io.github.witjava.support`**；建 GitHub org **`witjava`**（已核实可用），两仓库置于其下，Maven groupId `io.github.witjava` | FQN 出现在每个生成文件的 import 行里，带个人 handle 是采纳障碍（与 §11.2 拆仓的动机同源）；org 可整体转让 → **将来移交 BA 不再是 mapping breaking change**，消掉 §11.3 承认的那次 breaking |
| **Q2** | **resource → 纯 `interface extends AutoCloseable`**。M2 由「三选一」降级为「找反例」 | §5.6 的「没地方挂 handle」前提不成立：组件导出方向 adapter 用私有 impl 类持有 handle，host 实现方向用 `IdentityHashMap` 做 instance↔handle 映射。`abstract class` 的硬伤是单继承 —— **一个 Java 类无法同时实现两个 WIT resource**，且 `long handle` 是把 runtime 表示写进规范（违 §3.3）；`interface + long handle()` 会让 host 侧实现者被迫实现一个对他无意义的方法 |
| **Q3** | conformance **两层**：Tier 1（规范性）API shape 等价；Tier 2（非规范）字节级 golden 仅约束本实现。规范仓库**破例**放一个 JDK-only checker | 要求第二个实现字节级一致 = 要求它重实现我的 formatter，与 §1 成功判据直接冲突。checker 不含 mapping 逻辑（不知道 WIT 是什么，只读 Java），不违 §11.2 的分仓精神 |
| **Q4** | 不引入 `insta`，单一 `BLESS=1` 机制 | 两套 bless 流程增加摩擦；§14 判断「没 bless 没人维护 golden」的敌人是摩擦。且 insta 不擅长比对**文件集合**，而 mapping 改动最常见的后果正是文件增删 |
| **Q5** | `<java-pkg>.<world>.<role>.{Imports,Exports}`；**role 段恒存在**（单 role 也生成）；**world 恒独占一段**（`nested`/`flat` 都是）；**interface 与 role 无关，只生成一份** | 单 role 省略 role 段会让 `host`→`both` 切换时全部 FQN 位移，违 §5.9「规则简单可预测优先」。一个 WIT package 可含多个 world，flat 下不给 world 独立段会直接撞 `Imports`。interface 随 role 复制会产生两个不兼容的同名类型（§5.8 的坑） |
| **Q6** | 两仓均用 **`Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`**；生成文件**不 emit 许可证头**，只有 §15 的 `// @generated` 行 | 已核实 `wit-bindgen` 用的正是这个三段式（`wasmtime` 是 `Apache-2.0 WITH LLVM-exception`）。support 源码会被生成进用户代码库，LLVM exception 的存在意义就是免除派生产物的 attribution 义务，对 codegen 对症 |

### 1.2 已决（R 系列，规范补全 —— DESIGN.md §5 的缺口）

| # | 缺口 | 决定 |
|---|---|---|
| **R1** | **`map<K,V>`**：`wit-parser` 0.258 有 `TypeDefKind::Map(Type, Type)`，§5 完全没提 | **v1 → error `WJ0001`**。不映射到 `java.util.Map`：`list<u8>` 已按 §5.2 映射为 `byte[]`，而 `byte[]` 的 `equals` 是**引用相等**，`map<list<u8>, T>` 会静默行为错误 —— 与 §5.1 拒绝 `char→char` 同类的 correctness bug。§15 规定 vN 只能追加，先 error 后补安全，先映射后改是 breaking |
| **R2** | **world 聚合类的形状**：§7 只回答方向，没回答形状 | `Imports`/`Exports` 均为 interface；每个 imported/exported **interface** 出一个 accessor（`WallClock wallClock();`）；world 内联的 `import foo: func(...)` 直接作为抽象方法挂在 `Imports` 上；world 内联 type 定义落在 world 自己的 package 段 |
| **R3** | **type alias**：`type x = y`、`use a.{b as c}`（`TypeDefKind::Type`），§5 未提 | **不生成新 Java 类型**，解析到底层类型的 FQN。Java 无 type alias；生成 subclass 会破坏值语义与互换性。别名信息记入 Javadoc。resource 的 alias 同理 |
| **R4** | **flags 细节**：§5.5 只给了大方向 | `record X(long bits)`；成员常量为 **`X` 实例**：`public static final X FOO = new X(1L << n)`；方法集**精简为** `X or(X)` / `X and(X)` / `boolean contains(X)` / `static X empty()`。**不给 `not()`/`xor()`**（`not` 需要成员数 mask，语义易错，且下游可直接操作 `bits`）。成员数 0 合法（只有 `empty()`）；> 64 → `WJ0003` |
| **R5** | **javadoc 验收标准是空的**：实测 `-Xdoclint:all` 对缺 `@param`/`@return` 只报 warning、**exit 0**，§14 的「零 error」恒真 | 验收改为 `javadoc -Xdoclint:all,-missing` 且**零 warning**。WIT doc comment 无 per-param 文档，`missing` 组必须关掉，否则永远有噪声；关掉之后才能把标准提到零 warning |
| **R6** | **support 类型的 API 面** | 最小面：`sealed interface Result<T,E>` + `Ok`/`Err` record + 静态工厂 `ok()`/`err()`。**不给 `map`/`flatMap`/`orElse`**。§15「vN 只能追加」→ 方法可以后加，先加就冻死 |
| **R7** | **package-info / 注解** | 每个生成 package 出 `package-info.java`，承载 WIT package/interface 级 doc；`@WitGenerated(mapping="v1")` 用 **RUNTIME** retention（下游工具可反射检测，成本为零） |
| **R8** | **换行与 determinism** | 两仓 `.gitattributes` 强制 LF（golden 目录 `-text` 或 `text eol=lf`）。否则 Windows CI 上 determinism 测试必然假失败，且会被误判为 renderer bug |

### 1.3 上游已替我们解决的

- `Function.result` 在 wit-parser 0.258 是 `Option<Type>` —— **multi-return / named
  results 已被上游移除**，无需在 mapping 里处理。`None` → Java `void`
- `Resolve.features: IndexSet<String>` + `all_features: bool` —— §10.1 的
  `--features` / `--all-features` 直接对接，无需自建 gate 逻辑

### 1.4 error code 注册表（新增，进 `v1.md`）

跨实现比对错误信息子串太脆（措辞、语言都会变），负例断言改为「非 0 退出码 + 指定 code」：

```text
WJ0001  unsupported-type-construct    future / stream / error-context /
                                      fixed-size list<T,N> / map<K,V>
                                      （消息中须含具体构造名）
WJ0002  tuple-arity-exceeded          元数 > 8
WJ0003  flags-arity-exceeded          成员 > 64
WJ0004  fqn-collision                 两个 WIT item 映射到同一 Java FQN
WJ0005  mangling-collision            mangling 结果与已映射名冲突
WJ0006  nested-option-under-nullable  --option-style=nullable 下嵌套 option
WJ0007  package-collision             两个 WIT package / world / interface
                                      落到同一 Java package 且含同名 item
```

所有 error 仍须携带 WIT 源位置（§16）。code 一旦发布即冻结，新增只能追加。

---

## A. 本地仓库骨架（无 generator 代码）

### A1. `wit-java-mapping`

- [ ] `git init`；`main` 分支
- [ ] `README.md`：定位为「WIT → Java mapping specification」，明确
      *this repo is a specification, not an implementation*，链接参考实现
- [ ] `LICENSE`：`Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`（Q6）
- [ ] `.gitattributes`：强制 LF（R8）
- [ ] 目录：`spec/v1.md`、`spec/known-limitations.md`、`spec/error-codes.md`、
      `conformance/`、`tools/checker/`、`CHANGELOG.md`
- [ ] `spec/v1.md` 骨架：按 §5 小节号建空标题，**补上 R1–R4 对应的新小节**

### A2. `wit-java`

- [ ] `git init`；`.gitignore`（`target/`、`.localonly/`、`*.class`、`out/`）
- [ ] `LICENSE` 同 A1；`.gitattributes` 强制 LF（R8）
- [ ] `README.md`：强调 declaration-only，不含 lifting/lowering（避免 §11.1(3)
      的过度承诺）
- [ ] Cargo workspace 骨架（**只建 crate，不写逻辑**）：

```text
Cargo.toml                 [workspace]，resolver = "2"
crates/wit-java-core/      library：全部 generator 逻辑
crates/wit-java-cli/       薄壳：clap 解析 → core，binary 名 wit-java
xtask/                     javac / javadoc / determinism 测试驱动
tests/corpus/              vendored WASI WIT（见 D9）
```

- [ ] 依赖基线钉死：`wit-parser = "0.258"`、`thiserror`、`clap` v4、`toml`、`camino`。
      **不引入 async runtime，不引入模板引擎**（renderer 手写，见 D6）
- [ ] workspace `license = "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT"`
- [ ] `rust-toolchain.toml` 钉版本；`rustfmt.toml`；clippy 在 CI `-D warnings`
- [ ] 本地装 **JDK 17**（`brew install --cask temurin@17`）。当前是 JDK 26，
      `javac --release 17` 可用，但 doclint 行为随版本变，**§14 验收以 17 为基线**，
      CI 另跑一个 latest JDK 作前瞻（不作 gate）

### A3. 两仓关系

- [ ] **不用 submodule**。conformance 数据以「vendored 快照 + 版本号」引入
      `wit-java/tests/conformance/<mapping-ver>/`，避免开发期强耦合
- [ ] 约定：**spec 先改，实现后跟**。实现不得先于 spec 引入新的映射行为

---

## B. M1 — mapping spec v1 draft（先文档，后代码）

### B1. `spec/v1.md` 正文

- [ ] Primitive（§5.1）：13 个标量全表 + `char→int`、`u64→long` 的 rationale 与
      Javadoc **原文措辞**（措辞属于规范，不属于实现细节）
- [ ] List/tuple（§5.2）：`list<u8>|list<s8> → byte[]` 特例边界逐例写清
      （`option<list<u8>>`、`list<list<u8>>`、record component 各一例）
- [ ] `option`（§5.3）：两种 style 的完整规则；nullable 下嵌套 option → `WJ0006`
- [ ] `result`（§5.4）：四形态表 + support 类型**完整源码**入 spec（R6 的最小面）
- [ ] record / enum / variant（§5.5）
- [ ] **flags（R4）**：常量为 `X` 实例、方法集 `or`/`and`/`contains`/`empty`、
      0 成员合法、> 64 → `WJ0003`
- [ ] resource（§5.6 + Q2）：纯 interface；`Factory`/`Statics` 嵌套接口；
      `close()` **幂等、对应 drop、不 throws**；**不规定 `equals`/`hashCode`**
      （那是 runtime 语义，adapter 自决）；`own`/`borrow` 的 Javadoc 契约
- [ ] **type alias（R3）**：不生成新类型，解析到底层 FQN
- [ ] **`map<K,V>`（R1）** 与 async（§5.7）：均 → `WJ0001`，附 rationale
- [ ] support package（§5.8）：全部类型源码 + FQN 规则，默认
      `io.github.witjava.support`（Q1）
- [ ] mangling（§5.9）：保留字清单**逐字固化** —— Java 关键字全表、`Object` final
      方法（`wait`/`notify`/`notifyAll`/`getClass`）、record accessor 冲突组
      （`toString`/`hashCode`/`equals`/`clone`/`finalize`）
- [ ] Javadoc（§5.11）：转义规则、文件头**逐字**格式；**`package-info.java`（R7）**
- [ ] package 映射（§6）：version segment 算法（含 pre-release sanitize 字符集）、
      `nested`/`flat`、`--package-map` 三级优先级
- [ ] **world / role（§7 + Q5 + R2）**：`<pkg>.<world>.<role>.{Imports,Exports}`、
      accessor 形状、内联 func 的处理、interface 不随 role 复制
- [ ] `spec/error-codes.md`：1.4 的注册表

### B2. 人工验证（§19 M1 验收）

- [ ] 手工过 `wasi:filesystem@0.2.x`：逐个类型构造对照，找「表里没有」的
- [ ] 手工过 `wasi:sockets@0.2.x`：重点 tuple + resource 密集处
- [ ] `spec/known-limitations.md`：`u64` 无符号、`own`/`borrow` 不可区分、NaN 语义、
      非返回位置的 `Optional`、alias 语义丢失（R3）
- [ ] **验收**：不存在未覆盖的类型构造；每条 limitation 有明确记录

### B3. conformance（Q3 的落地）

- [ ] 数据格式：每 case = `case.wit` + `case.toml`(CLI 选项) + `expected/**.java`；
      负例 = `case.wit` + 期望 **error code**（1.4）
- [ ] **Tier 1 checker**（`tools/checker/`，JDK-only，约 300 行）：
      用 `com.sun.source` Compiler Tree API **解析**（不编译）生成目录，
      输出 canonical API digest —— package / 类型名 / kind / supertype /
      成员签名 / 修饰符，规范化后排序。任何语言写的实现都能自测
- [ ] 明确「conformance 通过」的定义：正例 digest 等价，负例非 0 退出码 + 命中 code
- [ ] 起手 20~30 个 case 覆盖 §5 每小节 + R1–R4，M3 期间随实现增长

---

## C. M2 — runtime 可用性 spike（mapping 冻结的前置条件）

Q2 已定纯 interface，本阶段的任务从「选型」变成**证伪**。

- [ ] 选定 world：`wasi:io/streams` 子集（`input-stream`/`output-stream` +
      `stream-error` variant），含 resource、variant、`result`、`list<u8>`
- [ ] 按 B1 规范**手写**理想 Java declaration（同时是 D 阶段第一个 golden 基准）
- [ ] runtime 选 **Chicory**（有已发布 artifact，本地可跑）。Endive 若届时仍无
      component model 支持则不作 spike 目标，只在 M0 询问中跟进
- [ ] 手写一层 lifting/lowering 实现这些 declaration
- [ ] **证伪判据**：若 lowering 需要修改任意一行 declaration，或需要 adapter
      之外的信息 → 纯 interface 方案被证伪，**重开选型，mapping v1 不得冻结**
- [ ] 记录 lowering 侧需要而 declaration 未给的东西（flags bits 可见性、variant
      discriminant 顺序稳定性等），补进 spec

---

## D. M3 — 参考实现

**策略：先打通 walking skeleton（`record` + primitive → `.java` → `javac` 通过），
再逐个类型构造加宽。** 不攒到最后一次性联调。

### D1. error / diagnostics（最先做）

- [ ] `Diagnostic`：**error code（1.4）** + message + WIT 源位置 + 可选 note
- [ ] 收集多条后统一报出，不首错即停（对 collision 类体验差别很大）
- [ ] 退出码：0 成功 / 1 用户输入错误（含全部 `WJ*`）/ 2 内部错误

### D2. `wit/` — WIT adapter（§3.1）

- [ ] `Resolve` 加载：目录 / 单文件 / deps
- [ ] **feature gates（§10.1）**：接 `Resolve.features` / `all_features`（1.3 已确认
      API）。**必须在跑真实 WASI 之前完成**，否则会把 unstable 被过滤当成自己的 bug
- [ ] 投影内部 model：`Package`/`Interface`/`World`/`TypeDef`/`Function`/`Resource`
- [ ] doc comment 原样带出（供 §5.11）
- [ ] 上游耦合面写进 `src/wit/README.md`：升级 wit-parser 只需查这一个模块

### D3. `config/` — 选项

- [ ] `GenerateOptions`：§10 全部选项
- [ ] `--package-map` TOML + §6.3 三级匹配优先级
- [ ] 选项校验（互斥、非法 FQN 早报错）
- [ ] `--mapping-version`：默认锁最新 **stable**（§15），v1 之外拒绝

### D4. `naming/`（§5.9 / §6.1）

- [ ] kebab → UpperCamel / lowerCamel / UPPER_SNAKE
- [ ] 保留字清单（从 spec 抄，**注释注明 spec 出处**）
- [ ] mangling：追加 `_`，冲突继续追加，与已映射名冲突 → `WJ0005`
- [ ] version segment：`v{major}` / `v0_{minor}` / pre-release sanitize
- [ ] `Fqn` 类型；全局唯一性由 D5 注册表保证
- [ ] **本模块单测密度最高**（纯函数），先写表驱动测试

### D5. `map/` — mapping 实现（核心）

按序实现，每步保持端到端可编译：

- [ ] primitive → record + Javadoc（**walking skeleton 里程碑**）
- [ ] enum、flags（R4）
- [ ] list / tuple / `Unit`（含 `byte[]` 特例）
- [ ] `option`（两种 style）
- [ ] `result`（四形态）
- [ ] variant（sealed interface + 嵌套 record）
- [ ] **type alias（R3）**：解析到底层 FQN，不生成新类型
- [ ] resource（纯 interface，Q2）
- [ ] world / role 聚合类（Q5 + R2）
- [ ] 不支持构造 → `WJ0001`：future / stream / error-context /
      fixed-size list / **map（R1）**
- [ ] **全局 FQN 注册表**：注册即查冲突 → `WJ0004`/`WJ0007`。
      这是 §5.10 中 1/2/6 三类的**统一实现点**，不要散落各处

### D6. `ir/` + `passes/` + `render/`

- [ ] IR：`JavaProject → JavaFile → Decl{Interface,Record,Enum,SealedInterface}`，
      类型引用一律 `Fqn`，**renderer 之前不出现字符串拼接**
- [ ] **import resolve pass**（§8，最易出 bug 处）：
      - 收集本文件全部 `Fqn` 引用
      - 同 simple name 冲突 → 保留一个 import，其余降级 FQN
        （降级规则确定：按 FQN 字典序，第一个胜出）
      - `java.lang.*` 不 import；同 package 不 import
      - 专项测试：跨 package 同名、嵌套类型、泛型参数内冲突
- [ ] renderer：手写，无模板引擎。**确定性硬要求**（§14）：
      文件/import/declaration/成员顺序全部显式排序；换行统一 `\n`；
      文件尾统一换行；不 emit 时间戳
- [ ] 文件头 `// @generated by wit-java <tool-ver>, mapping v1 — DO NOT EDIT`，
      **不带许可证头（Q6）**
- [ ] Javadoc 渲染：转义 `<` `>` `&`、行首 `@` → `&#64;`
- [ ] `package-info.java` 生成（R7）

### D7. `support/`（§5.8）

- [ ] 内容随 **mapping version** 走，不随 tool version 走：`support/v1/` 静态源码，
      `include_str!` 进二进制
- [ ] 默认 FQN `io.github.witjava.support`（Q1）；`--support-package` 可覆盖，
      package 声明是**唯一允许的字符串替换点**
- [ ] `--no-support` 跳过生成
- [ ] API 面按 R6 保持最小
- [ ] **验收**：两次 generate 到不同目录，support `.java` 字节级一致；
      跨两个 WIT package 传 `result<T,E>` 能 `javac` 通过

### D8. CLI

- [ ] `generate` / `check` / `mapping-info`（§10）
- [ ] `check` 不写文件，走完整链路只报 diagnostic
- [ ] `mapping-info --mapping-version v1` 从**同一份数据**生成映射表，
      不手写第二份

### D9. 测试（§14，与 D1–D8 同步长）

- [ ] **mapping tests**：单构造 → 期望 Java 片段（表驱动，内联断言，无 insta）
- [ ] **golden tests**：整目录对比，`BLESS=1 cargo test` 更新（Q4）
- [ ] **compile test**：`javac --release 17 -Xlint:all -Werror`，JDK 17 基线
- [ ] **javadoc test（R5 修正）**：`javadoc -Xdoclint:all,-missing`，**零 warning**
- [ ] **determinism test**：同输入两次 `sha256` 全等；跨平台由 CI 补（R8 的
      `.gitattributes` 是前提）
- [ ] **negative tests**：1.4 每个 code 至少一例，断言退出码 + code + WIT 源位置
- [ ] **conformance runner**：消费 B3 数据集，跑 Tier 1 checker
- [ ] corpus vendoring：`wasi:cli` / `http` / `filesystem` / `sockets` @0.2.x +
      一个 0.3 async world，**按 upstream commit 钉死并记录来源**

### D10. CI

- [ ] `ci.yml`：fmt / clippy `-D warnings` / test
- [ ] matrix：ubuntu / macos / windows × JDK 17；另加 latest JDK 前瞻（不 gate）
- [ ] mapping 仓库：conformance 数据 schema 校验 + checker 自测

---

## E. M4 — mapping v1 冻结 + 1.0

- [ ] C 阶段证伪判据未触发（Q2 成立）
- [ ] §14.2 全部 corpus 通过 compile + javadoc + determinism + conformance
- [ ] support FQN 已最终确定（Q1 已定，org 建成即锁死）
- [ ] `v1.md` 打 tag `mapping/v1`；写入「vN 只能追加，不能修改」（§15）；
      error code 注册表同步冻结
- [ ] `wit-java` 发 1.0，README 标注 implements mapping v1

---

## F. M5 — 分发与 plugin（v1 之后）

- [ ] binary 矩阵：linux-x86_64 / linux-aarch64 / macos-x86_64 / macos-aarch64 /
      windows-x86_64
- [ ] `wit-java-support` 发 Maven Central（groupId `io.github.witjava`）
- [ ] Maven / Gradle plugin：**只 wrap 预编译 CLI**，不重实现 mapping（§13）
- [ ] Panama FFM 直接加载 cdylib：Java 22+，更后续

---

## G. M0 — 外部项（现在就发，D 开始前收敛）

只能由你本人做：

- [ ] **BA Zulip 询问 Endive 的 component model 计划**：时间表、打算如何生成 Java
      类型、是否愿意把类型映射抽成独立 spec
      - 红灯：已有人在做完整 bindgen → 转为提 PR 或放弃，不平行造轮子
- [ ] **联系 Arcjet 博客作者**（<https://blog.arcjet.com/running-wasm-on-the-jvm/>）：
      「只有类型声明、lifting/lowering 仍需自写」是否有价值
      - 红灯：「要全套，半套没用」→ 边界划错，adapter 必须进 v1 范围
- [ ] 命名空间占位（§11.4，无判定线，立刻做完）：
      - [ ] **建 GitHub org `witjava`**（已核实可用），两个空仓库
            `witjava/wit-java`、`witjava/wit-java-mapping`
      - [ ] crates.io：`wit-java` / `wit-java-core` / `wit-java-cli` /
            `wit-java-mapping` 各推 `0.0.0` 占位（2026-09 核实全部未占用）
      - [ ] Maven Central：`io.github.witjava` 的 Sonatype 验证（走 org 验证）
      - [ ] **不买域名**（§11.3）

---

## H. 风险登记

| 风险 | 影响 | 应对 |
|---|---|---|
| 有人从另一头做完同一件事（§2.2b 的现代版） | 项目归零 | G 的两项询问，**D 阶段前收敛** |
| M2 证伪纯 interface（Q2） | B 阶段返工 | C 排在 D 之前；spec 的 resource 一节标注「M2 未通过前不冻结」 |
| wit-parser 上游 API 变动 | 编译中断 | 耦合面收敛在 `wit/` 单模块 + 该模块 README |
| 单人项目失去维护（§2.3b） | 死因复现 | 规范独立成仓 + org 可转让（Q1）+ 可独立运行的 conformance checker（Q3） |
| async / `map` 压力提前到来 | 范围失控 | §5.7 与 R1 已定：报 `WJ0001`，**忍住** |
| JDK 版本间 doclint 行为漂移 | 验收标准漂移 | 17 为基线，最新 JDK 仅前瞻（R5 已实测 26 上的 `missing` 行为） |

---

## I. 下一步

1. A1 + A2（骨架，无逻辑代码）—— 半天
2. G 三项同时发出（异步，越早越好）
3. B1 + B2（`v1.md` 正文与人工验证）—— 接下来的主要工作量
4. B3（conformance 格式 + checker）
5. C（M2 spike）→ D（实现）
