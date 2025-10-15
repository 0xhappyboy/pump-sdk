<h1 align="center">
    Pump.fun SDK
</h1>
<h4 align="center">
实现了与 Pump.fun 交互的相关功能.
</h4>
<p align="center">
  <a href="https://github.com/0xhappyboy/pump-sdk/LICENSE"><img src="https://img.shields.io/badge/License-GPL3.0-d1d1f6.svg?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=googledocs&label=license&logoColor=BEC5C9" alt="License"></a>
</p>
<p align="center">
<a href="./README_zh-CN.md">简体中文</a> | <a href="./README.md">English</a>
</p>

## 例子

### 监听 Pump.fun 相关事件.

```rust
use pump_sdk::listener::Listener;
#[tokio::main]
pub async fn main() {
    let listener = Listener::new(500, 500);
    let _ = listener
        .start(
            |_event| {
                // handle trade event
            },
            |_event| {
                // handle liquidity event
            },
        )
        .await;
    let _ = tokio::signal::ctrl_c().await;
}
```