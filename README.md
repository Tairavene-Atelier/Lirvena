# Lirvena

「寄声脉」是 Lirvena 的官方中文译名。Lirvena 是面向多账号场景的现代 QQ
协议客户端，采用 `AGPL-3.0-only` 开源。

当前仓库仍处于早期工程阶段。Linux 客户端已能与 Ceylith 建立加密会话、协商受签名
Profile，并通过真实 QQ 网络获取和轮询登录二维码；二维码可输出到终端和 PNG。扫码后的
凭据交换、严格响应解码、初始在线同步、有界 Ceylith 动作执行器、状态确认及由签名 Profile
调度的 Heartbeat/延迟同步已经实现，但尚待真实扫码验收。登录后的 QQ 连接使用单读者会话，
能够把异步 Push 与请求响应分流，并按签名 Profile 选择已编译的回显、配置回包、字段回包、
有界旧视频通知回包、InfoSync 状态投影、观察和保护性下线原语。Profile 绑定的私有运行材料由
Ceylith 持有；Lirvena 不接受用户提供的 Profile 或动作材料文件。消息 Push 已接入共享账号事件流，
能够有界解码消息外层、路由元数据、去重身份及富文本中的文字、@、表情、JSON/XML、戳一戳和现代图片、视频、语音
元数据，并将兼容图片、视频字段投影到同一媒体模型。其他 element 会保留原始编码并显式标记为
未支持。OneBot 已能发送文本、@、经典表情、回复、JSON/XML 富内容、戳一戳、图片、QQ 兼容语音和
MP4 视频；媒体统一支持状态目录内的
本地文件、公开 HTTP(S)、`base64://` 与 `cache://` 引用，经过有界解析、QQ 元数据协商和顺序
上传后才进入消息。语音当前原生接收 MP3、AMR、Tencent SILK v3，并会把标准 SILK v3
规范化为 Tencent framing；其他音频格式在可靠的有界转码链接入前会明确失败，不会伪装格式上传。
群请求和好友请求可通过 Lagrange 兼容的查询 action 获取，并保留与审批 action 相同的版本化 flag。
视频使用同一有界引用解析器，经双文件元数据协商分别上传视频与缩略图；未提供缩略图时使用内建
最小有效图片，不从任意路径或命令生成隐式输入。
每账号独立、私有、最多 4096 条的 SQLite
消息状态支持 `get_msg`；收到的 QQ 消息会持久保留有界原始元素及来源关联，使同会话 OneBot
`reply` 可在重启后真实引用，无法唯一反查的入站回复仍显式标记为未支持。仍在保留范围内的群聊与私聊 `delete_msg` 会跨进程重启复用原始 QQ
关联字段真实撤回，`mark_msg_as_read` 会复用同一持久关联提交 QQ 已读报告。缺少关联证据、
已淘汰的 ID 或未确认成功的 QQ 回包都会明确失败。
具有完整群消息关联的新记录还支持标准 `set_essence_msg` 与 `delete_essence_msg`；从旧存储
代际迁移且缺少 random 的记录不会伪造精华操作。
目录查询已覆盖好友、群、群成员和标准 `get_stranger_info`；公开资料响应会校验请求 QQ 号，
且只投影有界、有效的公开字段。
完整 OneBot 和生产部署尚未完成，因此当前版本仍不能声称已经登录或稳定在线。

账号授权模式固定为 `public`、`require_grant` 和 `allow_public_fallback`。缺少授权时，
`require_grant` 拒绝启动，`allow_public_fallback` 仅能在启动阶段发出强警告后进入 Public；
在线 Full 授权失效必须先保护性下线，不能在同一登录代际热切换。多个账号共享一个
Installation 级 Ceylith 操作连接所有者和有界请求队列；Full 模式另用一条专用 Watch
连接接收续签、额度、策略与撤权事件，避免长轮询阻塞签名请求。Watch 断链或撤权会先关闭
当前 QQ 流，再把账号状态持久化为保护性下线。额度超限不会替用户挑选降级账号。

推荐把安装和账号配置写入 `lirvena.json`，通过 `LIRVENA_CONFIG_PATH` 指向它；相对路径以该配置
文件所在目录为基准。配置按 `ceylith`、`installation`、`profile` 和 `accounts` 分区，每个账号分别
指定本地 slot 标识文件、授权模式、设备画像与二维码输出路径，示例见
[`lirvena.example.json`](lirvena.example.json)。旧的单账号环境变量配置暂时兼容。

账号状态默认存入当前目录的 `.lirvena-state`，文件配置可通过
`installation.state_directory` 指定，旧配置也可使用 `LIRVENA_STATE_DIRECTORY`。每个账号使用
独立 SQLite WAL 和独立 QQ transport；所有账号共享 Installation 级 Ceylith 操作连接与 Full
Watch。Linux 上已有状态目录必须仅对当前用户可访问。

用户可编辑的合成设备画像默认保存在当前目录的 `device.json`，也可通过
`LIRVENA_DEVICE_CONFIG_PATH` 指定。缺少文件时 Lirvena 会原子生成一次并稳定复用；用户可修改
GUID、MAC、设备名/型号、系统内核、内核版本和结构化电源画像。未知字段、全零标识、组播
MAC、不可能的电量、控制字符、超长文本和未知 schema 代际会被拒绝。可参考
[`device.example.json`](device.example.json)，实际 `device.json` 不进入 Git。

IP 栈和网络类型由 Lirvena 根据实际 transport 派生，不作为稳定设备画像让用户填写原始
QQ 数字值。生产应用版本、Profile 固定开关和安全链材料只在 Ceylith 的私有 Profile 注册表
定义，经加密会话下发签名 Profile；Lirvena 公开源码和测试不硬编码这些生产值。没有冻结证据
时，外部画像字段不会被擅自拼入 QQ wire。

## 通知

Lirvena 原生支持 Bark Server V2、固定 JSON Webhook 和 SMTP。通知使用独立 SQLite
outbox；默认冷却 15 分钟，失败后按 1 分钟、5 分钟、30 分钟、2 小时重试，严重事件最长
保留 24 小时。Ceylith Watch、授权变化和保护性下线会进入同一事件模型，通知网络失败不会
改变 QQ 安全状态机。

设置对应前缀即可启用适配器：

- Bark：`LIRVENA_NOTIFY_BARK_SERVER`、`LIRVENA_NOTIFY_BARK_KEY_PATH`；
- Webhook：`LIRVENA_NOTIFY_WEBHOOK_URL`，可选
  `LIRVENA_NOTIFY_WEBHOOK_HEADERS_PATH` 与 `LIRVENA_NOTIFY_WEBHOOK_HMAC_PATH`；
- SMTP：`LIRVENA_NOTIFY_SMTP_HOST`、`LIRVENA_NOTIFY_SMTP_PORT`、
  `LIRVENA_NOTIFY_SMTP_SECURITY`、`LIRVENA_NOTIFY_SMTP_FROM`、
  `LIRVENA_NOTIFY_SMTP_TO`，认证时同时设置 username/password 文件路径。

密钥、HMAC、SMTP 凭据和自定义 header 文件在 Linux 上必须是当前用户私有文件。SMTP
只接受 `starttls` 或 `implicit_tls`，不提供明文回退。可用下列命令真实验证所有已配置
目标，或把 `all` 换成 `bark`、`webhook`、`smtp`：

```text
lirvena notify test all
```

## 构建

需要 Rust 1.96。仓库将 Cargo 并行任务限制为 2，以降低本地和 CI 的内存压力。

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 名称与发行物

项目代码、包名、协议标识和配置键统一使用 `Lirvena`。中文品牌文案可使用
“Lirvena（寄声脉）”。官方发行物与第三方构建的标识规则见
[TRADEMARKS.md](TRADEMARKS.md)。

## 安全

不要在公开 issue、提交、测试或日志中附带 Token、抓包、账号数据、私有服务端材料或
逆向证据。漏洞请通过 GitHub 的私密安全报告渠道提交，详见
[SECURITY.md](SECURITY.md)。
