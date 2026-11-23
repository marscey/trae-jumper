//! 豆包网页端 API 客户端（纯 Cookie 会话鉴权）。
//!
//! 端点与字段口径见 `docs/doubao-api/API-INVENTORY.md`。
//! 实测结论：
//! - 无需 Authorization Bearer/JWT，仅凭 Cookie 会话即可调通 `/alice/commerce/*`；
//! - 公共查询参数缺失会导致概览块缺失 / `710010202 invalid param`；
//! - `a_bogus` / `msToken` 风控签名在浏览器回放实测可省略；后端裸请求若被网关
//!   拦截，兜底方案为内嵌 WebView 代发 fetch（见方案文档风险表）。

use anyhow::{anyhow, Result};
use reqwest::{header, Client};
use serde_json::json;
use std::time::Duration;

use super::doubao_types::*;

const HOST_WWW: &str = "https://www.doubao.com";
const HOST_ACCOUNTS: &str = "https://accounts.doubao.com";

const DESKTOP_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

fn build_http_client() -> Result<Client> {
    Ok(Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()?)
}

/// 豆包 API 客户端。
pub struct DoubaoApiClient {
    client: Client,
    cookies: String,
}

impl DoubaoApiClient {
    pub fn new(cookies: &str) -> Result<Self> {
        // Cookie header value 只允许可见 ASCII（\x20-\x7e）和 tab。
        // 过滤所有控制字符（Chrome v10 解密残留 / \x00 / \r 等），
        // 否则 reqwest HeaderValue::from_bytes 会报 "failed to parse header value"。
        let cleaned: String = cookies
            .chars()
            .filter(|c| {
                let c = *c as u32;
                (0x20..=0x7e).contains(&c) || c == b'\t' as u32
            })
            .collect();
        if cleaned.is_empty() {
            return Err(anyhow!(
                "清洗后 Cookie 为空：原始输入 {} 字节，过滤后无有效可见 ASCII 字符。\n\
                 可能原因：Cookie 解密失败、客户端未登录、或输入本身不是有效的 Cookie 串。",
                cookies.len()
            ));
        }
        Ok(Self {
            client: build_http_client()?,
            cookies: cleaned,
        })
    }

    /// 当前 cookies 字符串（预留：后续如需合并 Set-Cookie 在此扩展）。
    pub fn cookies(&self) -> &str {
        &self.cookies
    }

    /// `/alice/commerce/*` 网页端公共查询参数（aid=497858，Cookie 会话为网页端登录态）。
    fn common_query() -> Vec<(String, String)> {
        vec![
            ("version_code".to_string(), "20800".to_string()),
            ("language".to_string(), "zh".to_string()),
            ("device_platform".to_string(), "web".to_string()),
            ("doubao_device_platform".to_string(), "web".to_string()),
            ("aid".to_string(), "497858".to_string()),
            ("real_aid".to_string(), "497858".to_string()),
            ("pkg_type".to_string(), "release_version".to_string()),
            ("region".to_string(), "CN".to_string()),
            ("sys_region".to_string(), "CN".to_string()),
            ("samantha_web".to_string(), "1".to_string()),
            ("web_platform".to_string(), "desktop".to_string()),
        ]
    }

    /// 豆包客户端（桌面 App）公共查询参数（aid=582478）。
    ///
    /// 实测用于 `/alice/ug/campaign/landing/checkin/` 等客户端专属路径。
    /// 与 `common_query` 的关键差异：aid/real_aid=582478、client_platform=pc_client、
    /// doubao_device_platform=desktop、channel=mac_official、pc_version/runtime。
    fn client_query() -> Vec<(String, String)> {
        vec![
            ("version_code".to_string(), "20800".to_string()),
            ("language".to_string(), "zh".to_string()),
            ("device_platform".to_string(), "web".to_string()),
            ("doubao_device_platform".to_string(), "desktop".to_string()),
            ("aid".to_string(), "582478".to_string()),
            ("real_aid".to_string(), "582478".to_string()),
            ("pkg_type".to_string(), "release_version".to_string()),
            ("region".to_string(), "CN".to_string()),
            ("sys_region".to_string(), "CN".to_string()),
            ("client_platform".to_string(), "pc_client".to_string()),
            ("channel".to_string(), "mac_official".to_string()),
            ("pc_version".to_string(), "2.37.12".to_string()),
            ("runtime".to_string(), "web".to_string()),
        ]
    }

    fn build_headers(&self, host: &str) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse()?);
        headers.insert(header::ACCEPT, "application/json, text/plain, */*".parse()?);
        // accounts.doubao.com（passport 端点）用自身 Origin；www.doubao.com 用 HOST_WWW
        let origin = if host.contains("accounts.") {
            format!("https://{}", host)
        } else {
            HOST_WWW.to_string()
        };
        let referer = if host.contains("accounts.") {
            format!("https://{}/", host)
        } else {
            format!("{}/", HOST_WWW)
        };
        headers.insert(header::ORIGIN, origin.parse()?);
        headers.insert(header::REFERER, referer.parse()?);
        headers.insert(header::USER_AGENT, DESKTOP_UA.parse()?);
        let cookie_value = header::HeaderValue::from_bytes(self.cookies.as_bytes())
            .map_err(|e| anyhow!("Cookie 格式错误: {}", e))?;
        headers.insert(header::COOKIE, cookie_value);
        Ok(headers)
    }

    /// 豆包客户端风格 headers（不带 Origin/Referer，UA 模拟桌面 App）。
    ///
    /// 用于 `/alice/ug/campaign/landing/*` 等客户端专属路径。
    fn build_client_headers(&self) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse()?);
        headers.insert(header::ACCEPT, "application/json, text/plain, */*".parse()?);
        // 客户端 Chromium 的 Origin 不是 www.doubao.com，而是 chrome-extension:// 或不带
        // 实测客户端签到请求的 Origin 为 null / 不存在
        headers.insert(header::USER_AGENT, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 Doubao/2.37.12".parse()?);
        headers.insert(header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9".parse()?);
        let cookie_value = header::HeaderValue::from_bytes(self.cookies.as_bytes())
            .map_err(|e| anyhow!("Cookie 格式错误: {}", e))?;
        headers.insert(header::COOKIE, cookie_value);
        Ok(headers)
    }

    /// 解析信封：`code == 0` 取 `data`，否则抛出带 code/msg 的错误。
    fn unwrap_envelope<T>(env: DoubaoEnvelope<T>, endpoint: &str) -> Result<T> {
        if env.code != 0 {
            let msg = env
                .msg
                .or(env.message)
                .unwrap_or_else(|| "未知错误".to_string());
            return Err(anyhow!("豆包接口 [{}] 返回错误 code={} msg={}", endpoint, env.code, msg));
        }
        env.data.ok_or_else(|| anyhow!("豆包接口 [{}] 响应 data 为空", endpoint))
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str, query: &[(String, String)], endpoint: &str) -> Result<T> {
        let resp = self
            .client
            .get(url)
            .headers(self.build_headers(HOST_ACCOUNTS)?)
            .query(query)
            .send()
            .await
            .map_err(|e| anyhow!("请求 [{}] 失败: {}", endpoint, e))?;
        let env: DoubaoEnvelope<T> = resp
            .json()
            .await
            .map_err(|e| anyhow!("解析 [{}] 响应失败: {}", endpoint, e))?;
        Self::unwrap_envelope(env, endpoint)
    }

    async fn post_json<T: serde::de::DeserializeOwned>(&self, path: &str, body: &serde_json::Value, endpoint: &str) -> Result<T> {
        self.post_json_with_query(path, body, endpoint, &Self::common_query()).await
    }

    async fn post_json_with_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        endpoint: &str,
        query: &[(String, String)],
    ) -> Result<T> {
        let url = format!("{}{}", HOST_WWW, path);
        #[cfg(debug_assertions)]
        {
            let query_str: String = query.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            let cookie_preview: String = self.cookies.chars().take(80).collect();
            println!(
                "[DEBUG][doubao][{}] POST {url}?{query_str}\n  body={}\n  cookies={}...(len={})",
                endpoint,
                body,
                cookie_preview,
                self.cookies.len(),
            );
        }
        let resp = self
            .client
            .post(&url)
            .headers(self.build_headers(HOST_WWW)?)
            .query(query)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow!("请求 [{}] 失败: {}", endpoint, e))?;

        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        #[cfg(debug_assertions)]
        println!("[DEBUG][doubao][{}] HTTP {} → {}", endpoint, status, &raw[..raw.len().min(500)]);

        let env: DoubaoEnvelope<T> = serde_json::from_str(&raw)
            .map_err(|e| anyhow!("解析 [{}] 响应失败: {} (raw={})", endpoint, e, &raw[..raw.len().min(200)]))?;
        Self::unwrap_envelope(env, endpoint)
    }

    /// 客户端风格 POST（用 build_client_headers 不带 Origin/Referer，UA 带 Doubao 标识）。
    /// 用于 `/alice/ug/campaign/landing/*` 等客户端专属路径。
    async fn post_json_client<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        endpoint: &str,
        query: &[(String, String)],
    ) -> Result<T> {
        let url = format!("{}{}", HOST_WWW, path);
        #[cfg(debug_assertions)]
        {
            let query_str: String = query.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            println!(
                "[DEBUG][doubao-client][{}] POST {url}?{query_str}\n  body={}",
                endpoint, body,
            );
        }
        let resp = self
            .client
            .post(&url)
            .headers(self.build_client_headers()?)
            .query(query)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow!("请求 [{}] 失败: {}", endpoint, e))?;

        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        #[cfg(debug_assertions)]
        println!("[DEBUG][doubao-client][{}] HTTP {} → {}", endpoint, status, &raw[..raw.len().min(500)]);

        let env: DoubaoEnvelope<T> = serde_json::from_str(&raw)
            .map_err(|e| anyhow!("解析 [{}] 响应失败: {} (raw={})", endpoint, e, &raw[..raw.len().min(200)]))?;
        Self::unwrap_envelope(env, endpoint)
    }

    // ============ D1 账户资料 ============

    /// 拉取账户资料（昵称/头像/脱敏手机/UID 等），用于新增账号校验与资料回填。
    pub async fn get_account_info(&self) -> Result<DoubaoAccountInfo> {
        let url = format!("{}/passport/account/info/v2/", HOST_ACCOUNTS);
        self.get_json::<DoubaoAccountInfo>(&url, &[], "D1 account/info").await
    }

    // ============ D2 额度概览 ============

    /// 额度概览：套餐、剩余总额、分组汇总、概览额度包、活动奖励额度。
    pub async fn subscription_overview(&self) -> Result<OverviewData> {
        self.post_json("/alice/commerce/sale/subscription/overview/", &json!({}), "D2 overview")
            .await
    }

    // ============ D3 分组明细 ============

    /// 额度分组明细：按类别聚合的额度包（已用/总量/百分比/到期）。
    pub async fn quota_balance_detail(&self) -> Result<DetailData> {
        self.post_json("/alice/commerce/sale/subscription/quota_balance/detail/", &json!({}), "D3 quota detail")
            .await
    }

    // ============ D4 消耗流水 ============

    /// 消耗流水（近 30 天窗口）。`category` 3=赠送 / 7=付费；`cursor` 翻页游标（字段名待翻页样本补全）。
    pub async fn usage_timeline(
        &self,
        category: Option<i32>,
        cursor: Option<String>,
        page_size: Option<i32>,
    ) -> Result<TimelineData> {
        let mut body = json!({});
        if let Some(c) = category {
            body["category"] = json!(c);
        }
        if let Some(c) = cursor {
            body["cursor"] = json!(c);
        }
        if let Some(p) = page_size {
            body["page_size"] = json!(p);
        }
        self.post_json("/alice/commerce/usage/timeline/", &body, "D4 usage timeline")
            .await
    }

    // ============ D5 订阅状态 ============

    pub async fn subscription_status(&self) -> Result<SubscriptionStatusData> {
        self.post_json("/alice/commerce/sale/subscription/status/", &json!({}), "D5 subscription status")
            .await
    }

    // ============ D8 重置卡列表 ============

    pub async fn card_list(&self) -> Result<CardListData> {
        self.post_json("/alice/commerce/marketing/card/list/", &json!({}), "D8 card list")
            .await
    }

    // ============ D9 卡余额 ============

    pub async fn card_balance(&self) -> Result<CardBalanceData> {
        self.post_json("/alice/commerce/marketing/card/balance/", &json!({}), "D9 card balance")
            .await
    }

    // ============ D10 签到状态 + Claim ============

    /// 签到状态查询 + 自动 claim（一体化接口，调用即签到发放额度）。
    ///
    /// 实测路径：`POST /alice/ug/campaign/landing/checkin/`，请求体仅 {"scene":"pc_incentive_task_v3"}。
    /// 注意：此接口**既是查询也是 claim**，幂等（已签到则返回状态，不重复发奖）。
    ///
    /// 使用客户端风格 headers（不带 Origin/Referer，UA 带 Doubao 标识），并带
    /// client_query()（aid=582478）。如果 cookies 属于网页端 aid=497858 会话，
    /// 会回退尝试 common_query()。
    pub async fn checkin(&self) -> Result<DoubaoCheckinData> {
        let body = json!({"scene": "pc_incentive_task_v3"});
        // 先尝试客户端参数（aid=582478，与客户端 cookies 匹配）
        let client_result = self
            .post_json_client(
                "/alice/ug/campaign/landing/checkin/",
                &body,
                "D10 checkin(client_query)",
                &Self::client_query(),
            )
            .await;

        // 如果客户端参数返回系统错误（710010702），回退用网页端参数（aid=497858）
        match client_result {
            Ok(data) => Ok(data),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("710010702") || err_str.contains("系统错误") {
                    println!("[WARN][doubao-checkin] client_query 失败: {err_str}，回退 common_query");
                    self.post_json_client(
                        "/alice/ug/campaign/landing/checkin/",
                        &body,
                        "D10 checkin(common_query-fallback)",
                        &Self::common_query(),
                    )
                    .await
                } else {
                    Err(e)
                }
            }
        }
    }
}
