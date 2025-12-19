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

### 解析债券曲线的数据结构.

```rust
#[cfg(test)]
mod tests {
    use crate::Pump;

    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test() {
        let solana = Solana::new(solana_network_sdk::types::Mode::MAIN).unwrap();
        let pump = Pump::new(Arc::new(solana));
        let bond_curve = pump.create_bond_curve();
        let pool = bond_curve
            .get_bond_curve_pool_info("9RxTSGsTu3VdEGxRy6h3Jmk3hgP4Cfssw8SiPP4PRuKG")
            .await
            .unwrap();
        println!("Bond Curve Pool: {:?}", pool);
    }
}
```