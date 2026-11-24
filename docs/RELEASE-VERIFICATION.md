# 发布验证 · Release verification

## 发布包复测签名

GitHub Actions 从同一发布记录固定五个包和各校验文件的 SHA-256。各平台下载安装前核对文件摘要。通过整套复测后，GitHub Actions 使用 Sigstore 签名测试声明，将签名与原发布包的摘要关联。原发布包和版本标签保留原样。

| 平台 | 复测范围 |
| --- | --- |
| Windows x64 | 静默安装并启动 NSIS 版，解压并启动便携版，检查两套程序的 WebView2 数据及 Authenticode 状态。 |
| macOS Apple Silicon 与 Intel | 挂载 DMG，复制应用包并启动，检查应用签名与装订的 Apple 公证票据。 |
| Ubuntu x64 | 安装发布的 DEB，在 Xvfb 会话启动。 |
| 所有平台 | Rust 测试使用真实系统密钥库，验证测试凭据的保存和清除、三个 CLI 夹具的加密备份、逐文件写入与原字节恢复。原生文件选择器仍需交互复测。 |

Windows 的系统签名需要受信任的 Authenticode 发布者身份。macOS 的系统签名和公证需要 Developer ID Application 证书与 Apple 公证凭据。发布页的 Sigstore 复测证明与系统签名状态可分别查看。

## Verify the detached statement

In a directory containing a package and the detached Sigstore bundle from the same release, run the following command with an authenticated GitHub CLI. This example verifies the v1.8.0 portable ZIP. Substitute the matching bundle and package filenames to verify another version.

```powershell
gh attestation verify ./WhaleSwitch-1.8.0-windows-x64-portable.zip --bundle ./WhaleSwitch-1.8.0-verification.sigstore.json --repo Pasukalu/WhaleSwitch --signer-workflow Pasukalu/WhaleSwitch/.github/workflows/verify-release.yml --predicate-type https://github.com/Pasukalu/WhaleSwitch/attestations/native-release-retest/v1 --deny-self-hosted-runners
```

The statement covers the published Windows NSIS installer and portable ZIP, both macOS DMGs, and the Ubuntu DEB. It records the original release commit and the separate verification run. Native keychain operations use isolated fixtures and disposable credentials.

For new releases, the main build workflow generates a SLSA provenance attestation from packages built in that run. Verify a package from a release that has build provenance with the following command.

```sh
gh attestation verify ./PACKAGE_FILENAME --repo Pasukalu/WhaleSwitch --signer-workflow Pasukalu/WhaleSwitch/.github/workflows/checks.yml --deny-self-hosted-runners
```

[GitHub Releases](https://github.com/Pasukalu/WhaleSwitch/releases) · [Retest workflow](../.github/workflows/verify-release.yml) · [GitHub CLI verification](https://cli.github.com/manual/gh_attestation_verify)
