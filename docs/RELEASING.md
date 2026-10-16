# 发布 Windows 版本

正式构建统一使用 Cargo 的默认目录：

```powershell
cargo build --release --locked
./scripts/package-release.ps1
```

输出位于 `target/release/`：

- `mori.exe`：Windows x86_64 可执行文件，图标与界面资源已内嵌。
- `mori-windows-x86_64.zip`：程序、README、发布指南、项目许可证、第三方声明和对应许可证文件。
- `SHA256SUMS.txt`：可执行文件和 ZIP 的 SHA-256 校验值。

打包脚本不会创建其他发布目录，临时打包文件也位于 `target/release` 并在结束时清理。用户配置仍保存在系统应用数据目录，和程序输出位置无关。

## 用 tag 注明更新内容

可以。Git 的“附注 tag”自带一段说明，推送 tag 时会一起上传。先确保代码与工作流已提交并推送，然后执行：

```powershell
$notes = @'
## 更新内容
- 支持自定义模型连接
- 支持删除连接并保留聊天记录
- 修复图片重复粘贴
'@

git tag -a v1.0.1 -m $notes
git push origin v1.0.1
```

此命令只是使用示例，版本号与更新内容由发布者填写。长篇说明也可以保存在 UTF-8 Markdown 文件中，再执行 `git tag -a v1.0.1 -F 路径/说明.md`。正式发布前，同步 `Cargo.toml`、`Cargo.lock` 与 `assets/mori.rc` 中的应用版本；tag 名决定 Release 的名称，不会自动修改应用内版本。

`.github/workflows/release.yml` 在推送 `v*` tag 时自动执行：

1. 在 Windows 2022 x86_64 runner 安装 Rust stable，检查格式、运行测试。
2. 使用 `cargo build --locked --release` 构建，并生成 ZIP 与校验文件。
3. 在独立发布任务中校验产物，创建 GitHub Release 并上传三个文件。

只有发布任务拥有仓库 `contents: write` 权限，使用 GitHub 自动提供的 `GITHUB_TOKEN`，无需额外配置个人令牌。仓库或组织的 Actions 策略仍需允许运行工作流和写入 Release。

发布说明规则：

- **附注 tag**（上面的 `git tag -a`，或签名 tag）：使用 tag 注释原文作为 Release 描述。
- **轻量 tag**（`git tag v1.0.1`）：调用 GitHub 自动生成发布说明，内容以 GitHub 的 PR / 版本比较规则为准。
- **预发布**：tag 名包含 `-` 时，例如 `v1.1.0-rc.1`，标记为 prerelease，不设为 latest。
- **重新运行任务**：已有 Release 时只重新上传附件，保留手工编辑过的描述。开启 immutable releases 的仓库会禁止改写已发布附件，此时应发布新版本。

工作流通过 GitHub API 读取远端附注 tag，避免 checkout 将本地 tag 引用转换为 commit 后丢失注释。已发布版本需要改写说明时，编辑 `docs/releases/v版本号.md` 并推送 main；`Sync release notes` 会仅同步本次改动文件对应的已存在 Release，不创建版本、不上传附件、不移动 tag。未改变说明文件的普通推送不会改写 Release 描述。

推送普通分支不会触发发布。工作流只发布已经存在的 tag，不会创建或移动 tag。若构建失败，可从 Actions 查看日志并重新运行；发布任务只会在构建成功后运行。

依据：[GitHub CLI release create](https://cli.github.com/manual/gh_release_create)、[GitHub Actions push 事件](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#push)。

## Codex 发布技能

项目内技能为 [.agents/skills/mori-release/SKILL.md](../.agents/skills/mori-release/SKILL.md)，可用 `$mori-release` 执行版本准备与发布。技能区分准备和实际发布授权，并要求跟进 Actions 到 Release 产物确认。
