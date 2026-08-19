use anyhow::{anyhow, Result};
use reqwest::{header, Client};
use serde_json::json;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use super::types::*;

const API_BASE_US: &str = "https://api-us-east.trae.ai";
const API_BASE_SG: &str = "https://api-sg-central.trae.ai";
const API_BASE_UG: &str = "https://ug-normal.trae.ai";
const API_BASE_CN: &str = "https://api.trae.cn";

/// Trae API 客户端
pub struct TraeApiClient {
    client: Client,
    cookies: String,
    jwt_token: Option<String>,
    api_base: String,  // 动态 API 端点
}

impl TraeApiClient {
    /// 创建新的 API 客户端（使用 Cookies）
    pub fn new(cookies: &str) -> Result<Self> {
        let client = Client::builder()
            .build()?;

        // 清理 Cookie 字符串：移除换行符、多余空格
        let cleaned_cookies = cookies
            .lines()
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join("")
            .replace("  ", " ");

        // 从 cookies 中检测区域
        let api_base = Self::detect_api_base_from_cookies(&cleaned_cookies);

        Ok(Self {
            client,
            cookies: cleaned_cookies,
            jwt_token: None,
            api_base,
        })
    }

    /// 创建新的 API 客户端（使用 Token）
    pub fn new_with_token(token: &str) -> Result<Self> {
        let client = Client::builder()
            .build()?;

        // 国内版优先使用 api.trae.cn，国际版默认新加坡（请求失败时自动回退其他端点）
        let api_base = if crate::trae_app::current().is_cn {
            API_BASE_CN.to_string()
        } else {
            API_BASE_SG.to_string()
        };

        Ok(Self {
            client,
            cookies: String::new(),
            jwt_token: Some(token.to_string()),
            api_base,
        })
    }

    /// 从 Cookies 中检测 API 端点
    fn detect_api_base_from_cookies(cookies: &str) -> String {
        // 检查 store-idc 或 trae-target-idc
        if cookies.contains("store-idc=useast") || cookies.contains("trae-target-idc=useast") {
            API_BASE_US.to_string()
        } else if cookies.contains("store-idc=alisg") || cookies.contains("trae-target-idc=alisg") {
            API_BASE_SG.to_string()
        } else {
            // 默认使用新加坡
            API_BASE_SG.to_string()
        }
    }

    /// 尝试多个 API 端点获取数据（当前端点失败后按 CN → SG → US 回退）
    async fn try_api_endpoints<T, F, Fut>(&self, path: &str, request_fn: F) -> Result<T>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // 先尝试当前设置的端点
        let url = format!("{}{}", self.api_base, path);
        if let Ok(result) = request_fn(url).await {
            return Ok(result);
        }

        // 依次回退其他端点（跳过与当前相同的）
        let fallbacks = if self.api_base == API_BASE_CN {
            [API_BASE_SG, API_BASE_US, API_BASE_UG]
        } else {
            [API_BASE_CN, API_BASE_SG, API_BASE_US]
        };

        let mut last_err = anyhow!("所有 API 端点均失败");
        for base in fallbacks {
            if *base == self.api_base {
                continue;
            }
            let url = format!("{}{}", base, path);
            match request_fn(url).await {
                Ok(result) => return Ok(result),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    /// 当前站点 Origin/Referer（国内版 www.trae.cn，国际版 www.trae.ai）
    fn web_origin(&self) -> &'static str {
        if self.api_base == API_BASE_CN || crate::trae_app::current().is_cn {
            "https://www.trae.cn"
        } else {
            "https://www.trae.ai"
        }
    }

    /// 构建请求头（仅使用 Token，不需要 Cookies）
    fn build_headers_token_only(&self) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse()?);
        headers.insert(header::ACCEPT, "application/json, text/plain, */*".parse()?);
        let origin = self.web_origin();
        headers.insert(header::ORIGIN, origin.parse()?);
        headers.insert(header::REFERER, format!("{}/", origin).parse()?);
        headers.insert(
            header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".parse()?,
        );

        if let Some(token) = &self.jwt_token {
            let auth_value = header::HeaderValue::from_bytes(
                format!("Cloud-IDE-JWT {}", token).as_bytes()
            ).map_err(|e| anyhow!("Token 格式错误: {}", e))?;
            headers.insert(header::AUTHORIZATION, auth_value);
        }

        Ok(headers)
    }

    /// 通过 Token 获取用户信息（从 entitlement 接口获取 user_id）
    pub async fn get_user_info_by_token(&self) -> Result<TokenUserInfo> {
        // 先解析 JWT Token 获取基本信息
        let token = self.jwt_token.as_ref().ok_or_else(|| anyhow!("Token 不存在"))?;
        let jwt_data = Self::parse_jwt_token(token)?;

        // 尝试多个 API 端点（含国内 api.trae.cn）
        let headers = self.build_headers_token_only()?;
        let endpoints = [&self.api_base, API_BASE_CN, API_BASE_SG, API_BASE_US];

        let mut last_error = anyhow!("所有 API 端点都失败");

        for base in endpoints.iter() {
            let url = format!("{}/trae/api/v2/pay/user_current_entitlement_list", base);

            let response = self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"require_usage": true}))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<EntitlementListResponse>().await {
                        Ok(data) => {
                            let user_id_from_api = data.user_entitlement_pack_list
                                .first()
                                .map(|p| p.entitlement_base_info.user_id.clone())
                                .unwrap_or_else(|| jwt_data.user_id.clone());

                            let user_detail = self.get_user_info_with_token().await.ok();

                            return Ok(TokenUserInfo {
                                user_id: user_id_from_api,
                                tenant_id: jwt_data.tenant_id,
                                screen_name: user_detail.as_ref().map(|u| u.screen_name.clone()),
                                avatar_url: user_detail.as_ref().and_then(|u| if u.avatar_url.is_empty() { None } else { Some(u.avatar_url.clone()) }),
                                email: user_detail.as_ref().and_then(|u| u.non_plain_text_email.clone()),
                            });
                        }
                        Err(e) => {
                            last_error = anyhow!("解析响应失败: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    last_error = anyhow!("API 返回错误: {}", resp.status());
                }
                Err(e) => {
                    last_error = anyhow!("请求失败: {}", e);
                }
            }
        }

        Err(last_error)
    }

    /// 尝试用 Token 调用 GetUserInfo 接口（依次尝试 UG / CN 端点）
    async fn get_user_info_with_token(&self) -> Result<UserInfoResult> {
        let headers = self.build_headers_token_only()?;
        let mut last_err = anyhow!("获取用户信息失败");

        for base in [API_BASE_UG, API_BASE_CN] {
            let url = format!("{}/cloudide/api/v3/trae/GetUserInfo", base);
            let response = self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"IfWebPage": true}))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let data: GetUserInfoResponse = resp.json().await
                        .map_err(|e| anyhow!("解析用户信息失败: {}", e))?;
                    return Ok(data.result);
                }
                Ok(resp) => {
                    last_err = anyhow!("获取用户信息失败: {}", resp.status());
                }
                Err(e) => {
                    last_err = anyhow!("请求失败: {}", e);
                }
            }
        }

        Err(last_err)
    }

    /// 解析 JWT Token 获取用户信息
    fn parse_jwt_token(token: &str) -> Result<JwtPayload> {
        // JWT 格式: header.payload.signature
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow!("无效的 JWT Token 格式"));
        }

        // 解码 payload 部分（第二部分）
        let payload_b64 = parts[1];
        // JWT 使用 base64url 编码，需要处理 padding
        let padding = (4 - payload_b64.len() % 4) % 4;
        let padded = format!("{}{}", payload_b64, "=".repeat(padding));
        // 替换 base64url 字符为标准 base64
        let standard_b64 = padded.replace('-', "+").replace('_', "/");

        let payload_bytes = BASE64.decode(&standard_b64)
            .map_err(|e| anyhow!("解码 JWT payload 失败: {}", e))?;

        let payload_str = String::from_utf8(payload_bytes)
            .map_err(|e| anyhow!("JWT payload 不是有效的 UTF-8: {}", e))?;

        let payload: JwtPayloadRaw = serde_json::from_str(&payload_str)
            .map_err(|e| anyhow!("解析 JWT payload 失败: {}", e))?;

        Ok(JwtPayload {
            user_id: payload.data.id,
            tenant_id: payload.data.tenant_id,
        })
    }

    /// 公共静态方法：从 JWT Token 中解析 user_id（用于导入降级场景）
    pub fn parse_jwt_user_id(token: &str) -> Result<String> {
        let payload = Self::parse_jwt_token(token)?;
        Ok(payload.user_id)
    }

    /// 构建请求头
    fn build_headers(&self, with_auth: bool) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse()?);
        headers.insert(header::ACCEPT, "application/json, text/plain, */*".parse()?);

        // 使用 from_bytes 来处理包含特殊字符的 Cookie
        let cookie_value = header::HeaderValue::from_bytes(self.cookies.as_bytes())
            .map_err(|e| anyhow!("Cookie 格式错误: {}", e))?;
        headers.insert(header::COOKIE, cookie_value);

        let origin = self.web_origin();
        headers.insert(header::ORIGIN, origin.parse()?);
        headers.insert(header::REFERER, format!("{}/", origin).parse()?);
        headers.insert(
            header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".parse()?,
        );

        if with_auth {
            if let Some(token) = &self.jwt_token {
                let auth_value = header::HeaderValue::from_bytes(
                    format!("Cloud-IDE-JWT {}", token).as_bytes()
                ).map_err(|e| anyhow!("Token 格式错误: {}", e))?;
                headers.insert(header::AUTHORIZATION, auth_value);
            }
        }

        Ok(headers)
    }

    /// 获取用户 Token
    pub async fn get_user_token(&mut self) -> Result<UserTokenResult> {
        let url = format!("{}/cloudide/api/v3/common/GetUserToken", self.api_base);
        let headers = self.build_headers(false)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("获取 Token 失败: {}", response.status()));
        }

        let data: GetUserTokenResponse = response.json().await?;
        self.jwt_token = Some(data.result.token.clone());
        Ok(data.result)
    }

    /// 获取用户信息
    pub async fn get_user_info(&self) -> Result<UserInfoResult> {
        let url = format!("{}/cloudide/api/v3/trae/GetUserInfo", API_BASE_UG);
        let headers = self.build_headers(false)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&json!({"IfWebPage": true}))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("获取用户信息失败: {}", response.status()));
        }

        let data: GetUserInfoResponse = response.json().await?;
        Ok(data.result)
    }

    /// 获取用户配额和使用量
    pub async fn get_entitlement_list(&self) -> Result<EntitlementListResponse> {
        let url = format!("{}/trae/api/v2/pay/user_current_entitlement_list", self.api_base);
        let headers = self.build_headers(true)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&json!({"require_usage": true}))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("获取配额信息失败: {}", response.status()));
        }

        let data: EntitlementListResponse = response.json().await?;
        Ok(data)
    }

    /// 查询使用记录
    pub async fn query_usage(
        &self,
        start_time: i64,
        end_time: i64,
        page_size: i32,
        page_num: i32,
    ) -> Result<UsageQueryResponse> {
        let url = format!(
            "{}/trae/api/v1/pay/query_user_usage_group_by_session",
            self.api_base
        );
        let headers = self.build_headers(true)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&json!({
                "start_time": start_time,
                "end_time": end_time,
                "page_size": page_size,
                "page_num": page_num
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("查询使用记录失败: {}", response.status()));
        }

        let data: UsageQueryResponse = response.json().await?;
        Ok(data)
    }

    /// 获取使用量汇总（简化版，用于前端展示）
    pub async fn get_usage_summary(&mut self) -> Result<UsageSummary> {
        // 确保有 token
        if self.jwt_token.is_none() {
            self.get_user_token().await?;
        }

        let entitlements = self.get_entitlement_list().await?;
        Self::parse_entitlements_to_summary(entitlements)
    }

    /// 通过 Token 获取使用量汇总
    pub async fn get_usage_summary_by_token(&self) -> Result<UsageSummary> {
        let headers = self.build_headers_token_only()?;
        let endpoints = [&self.api_base, API_BASE_SG, API_BASE_US];

        let mut last_error = anyhow!("所有 API 端点都失败");

        for base in endpoints.iter() {
            let url = format!("{}/trae/api/v2/pay/user_current_entitlement_list", base);
            println!("[DEBUG] Trying API endpoint: {}", url);

            let response = self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"require_usage": true}))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let response_text = resp.text().await?;
                    println!("[DEBUG] API Response from {}: {}", base, response_text);

                    match serde_json::from_str::<EntitlementListResponse>(&response_text) {
                        Ok(entitlements) => {
                            let summary = Self::parse_entitlements_to_summary(entitlements)?;
                            println!("[DEBUG] Parsed Summary: fast_request_limit={}, extra_fast_request_limit={}",
                                summary.fast_request_limit, summary.extra_fast_request_limit);
                            return Ok(summary);
                        }
                        Err(e) => {
                            last_error = anyhow!("解析响应失败: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    println!("[DEBUG] API {} returned error: {}", base, resp.status());
                    last_error = anyhow!("API 返回错误: {}", resp.status());
                }
                Err(e) => {
                    println!("[DEBUG] API {} request failed: {}", base, e);
                    last_error = anyhow!("请求失败: {}", e);
                }
            }
        }

        Err(last_error)
    }

    /// 解析配额信息为使用量汇总
    fn parse_entitlements_to_summary(entitlements: EntitlementListResponse) -> Result<UsageSummary> {
        let mut summary = UsageSummary::default();

        for pack in entitlements.user_entitlement_pack_list {
            let base = &pack.entitlement_base_info;
            let usage = &pack.usage;
            let quota = &base.quota;

            // 判断是否是额外礼包（product_type == 2）
            if base.product_type == 2 {
                // Extra Package
                summary.extra_fast_request_limit = quota.premium_model_fast_request_limit;
                // 使用 premium_model_fast_amount 作为实际使用量
                summary.extra_fast_request_used = usage.premium_model_fast_amount;
                summary.extra_fast_request_left =
                    summary.extra_fast_request_limit as f64 - summary.extra_fast_request_used;
                summary.extra_expire_time = base.end_time;

                // 尝试获取礼包名称
                if let Some(pkg_extra) = &base.product_extra.package_extra {
                    if pkg_extra.package_source_type == 6 {
                        summary.extra_package_name = "2026 Anniversary Treat".to_string();
                    }
                }
            } else {
                // Free/Pro Plan
                summary.plan_type = if base.product_id == 0 {
                    "Free".to_string()
                } else {
                    "Pro".to_string()
                };
                summary.reset_time = base.end_time;

                summary.fast_request_limit = quota.premium_model_fast_request_limit;
                // 使用 premium_model_fast_amount 作为实际使用量
                summary.fast_request_used = usage.premium_model_fast_amount;
                summary.fast_request_left =
                    summary.fast_request_limit as f64 - summary.fast_request_used;

                summary.slow_request_limit = quota.premium_model_slow_request_limit;
                // 使用 premium_model_slow_amount 作为实际使用量
                summary.slow_request_used = usage.premium_model_slow_amount;
                summary.slow_request_left =
                    summary.slow_request_limit as f64 - summary.slow_request_used;

                summary.advanced_model_limit = quota.advanced_model_request_limit;
                // 使用 advanced_model_amount 作为实际使用量
                summary.advanced_model_used = usage.advanced_model_amount;
                summary.advanced_model_left =
                    summary.advanced_model_limit as f64 - summary.advanced_model_used;

                summary.autocomplete_limit = quota.auto_completion_limit;
                // 使用 auto_completion_amount 作为实际使用量
                summary.autocomplete_used = usage.auto_completion_amount;
                summary.autocomplete_left =
                    summary.autocomplete_limit as f64 - summary.autocomplete_used;
            }
        }

        Ok(summary)
    }

    /// 查询礼包状态
    pub async fn query_birthday_bonus(&self) -> Result<bool> {
        let url = format!("{}/trae/api/v1/pay/query_birthday_bonus", self.api_base);
        let headers = self.build_headers_token_only()?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("查询礼包状态失败: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;

        // 返回是否已领取
        Ok(data["bonus_claimed"].as_bool().unwrap_or(false))
    }

    /// 领取礼包
    pub async fn claim_birthday_bonus(&self) -> Result<()> {
        let url = format!("{}/trae/api/v1/pay/claim_birthday_bonus", self.api_base);
        let headers = self.build_headers_token_only()?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("领取礼包失败: {}", response.status()));
        }

        Ok(())
    }

    // ============================================================
    // 国内版（CN / WORK）积分计费接口
    // ============================================================

    /// (辅助) 通过 Token 拉一次 v2 版 entitlement list，只在积分账号下才用到
    async fn get_entitlement_list_v2_by_token(&self) -> Result<EntitlementListResponse> {
        let headers = self.build_headers_token_only()?;
        let endpoints_order: Vec<&str> = if self.api_base == API_BASE_CN {
            vec![&self.api_base, API_BASE_SG, API_BASE_US]
        } else {
            vec![&self.api_base, API_BASE_CN, API_BASE_SG]
        };
        let mut last_err = anyhow!("所有 v2 entitlement 端点都失败");
        for base in endpoints_order {
            let url = format!("{}/trae/api/v2/pay/user_current_entitlement_list", base);
            match self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"require_usage": true}))
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => {
                    let txt = r.text().await.unwrap_or_default();
                    println!("[DEBUG] ===== RAW v2 entitlement JSON ({} chars) =====", txt.len());
                    println!("[DEBUG] FULL_RESPONSE: {}", txt);
                    match serde_json::from_str::<EntitlementListResponse>(&txt) {
                        Ok(d) => return Ok(d),
                        Err(e) => {
                            println!("[WARN] 解析 v2 entitlement 失败: {}", e);
                            last_err = anyhow!("解析 v2 entitlement 失败: {}", e);
                        }
                    }
                }
                Ok(r) => {
                    let c = r.status().as_u16();
                    let body = r.text().await.unwrap_or_default();
                    if c == 401 {
                        return Err(anyhow!("401 未授权（v2 entitlement 接口）"));
                    }
                    last_err = anyhow!("v2 entitlement HTTP {}: {}", c,
                                       body.chars().take(300).collect::<String>());
                }
                Err(e) => { last_err = anyhow!("请求失败: {}", e); }
            }
        }
        Err(last_err)
    }

    /// (辅助) 获取 web_user_pay_status — 用来拿 Free/Lite/Pro 计划名
    async fn get_web_user_pay_status_by_token(&self) -> Result<WebUserPayStatusResponse> {
        let headers = self.build_headers_token_only()?;
        let endpoints_order: Vec<&str> = if self.api_base == API_BASE_CN {
            vec![&self.api_base, API_BASE_SG, API_BASE_US]
        } else {
            vec![&self.api_base, API_BASE_CN, API_BASE_SG]
        };
        let mut last_err = anyhow!("所有 web_user_pay_status 端点都失败");
        for base in endpoints_order {
            let url = format!("{}/trae/api/v2/pay/web_user_pay_status", base);
            match self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({}))
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => {
                    let txt = r.text().await.unwrap_or_default();
                    match serde_json::from_str::<WebUserPayStatusResponse>(&txt) {
                        Ok(d) => return Ok(d),
                        Err(e) => { last_err = anyhow!("解析 pay_status 失败: {}", e); }
                    }
                }
                Ok(r) => {
                    let c = r.status().as_u16();
                    if c == 401 { return Err(anyhow!("401 未授权（pay_status 接口）")); }
                    last_err = anyhow!("pay_status HTTP {}", c);
                }
                Err(e) => { last_err = anyhow!("请求失败: {}", e); }
            }
        }
        Err(last_err)
    }

    /// 通过 Token 获取积分详情（对应官网 trae.cn/dashboard#usage 的总可用/通用/Work 专属/奖励）
    ///
    /// 调用链：
    ///   1. POST /trae/api/v2/pay/cn_credits_billing_status  —  判断是否积分计费
    ///   2. POST /trae/api/v2/pay/user_current_entitlement_list —  拿积分 pack 明细（total/used/end_time）
    ///   3. POST /trae/api/v2/pay/web_user_pay_status —  拿计划名（Free / Lite / Pro）
    ///
    /// 若接口 1 返回 `is_credits_billing == false`，直接返回空 CreditSummary，
    /// 调用方（account_manager）会回退显示旧 UsageSummary。
    pub async fn get_credits_billing_status_by_token(&self) -> Result<CreditSummary> {
        // ---------- Step 1: cn_credits_billing_status — 开关判断 ----------
        let headers = self.build_headers_token_only()?;
        let path = "/trae/api/v2/pay/cn_credits_billing_status";
        let endpoints_order: Vec<&str> = if self.api_base == API_BASE_CN {
            vec![&self.api_base, API_BASE_SG, API_BASE_US, API_BASE_UG]
        } else if self.api_base == API_BASE_SG {
            vec![&self.api_base, API_BASE_CN, API_BASE_US, API_BASE_UG]
        } else {
            vec![&self.api_base, API_BASE_CN, API_BASE_SG, API_BASE_US]
        };

        let mut billing_switch: Option<CreditsBillingStatusResponse> = None;
        let mut last_err: anyhow::Error = anyhow!("所有 API 端点都失败（积分开关）");
        for base in &endpoints_order {
            let url = format!("{}{}", base, path);
            println!("[DEBUG] Trying credits endpoint: {}", url);
            match self.client.post(&url).headers(headers.clone())
                .json(&json!({})).send().await
            {
                Ok(r) if r.status().is_success() => {
                    let txt = r.text().await.unwrap_or_default();
                    println!("[DEBUG] credits_billing_status raw response (first 3KB): {}",
                             txt.chars().take(3000).collect::<String>());
                    match serde_json::from_str::<CreditsBillingStatusResponse>(&txt) {
                        Ok(raw) => { billing_switch = Some(raw); break; }
                        Err(e) => {
                            println!("[WARN] 解析 billing_switch 响应失败: {}", e);
                            last_err = anyhow!("解析 billing_switch 失败: {}", e);
                        }
                    }
                }
                Ok(r) => {
                    let code = r.status();
                    let txt = r.text().await.unwrap_or_default();
                    if code.as_u16() == 401 {
                        return Err(anyhow!("401 未授权（积分接口）"));
                    }
                    last_err = anyhow!("积分接口 HTTP {}: {}", code,
                                       txt.chars().take(300).collect::<String>());
                }
                Err(e) => { last_err = anyhow!("请求失败: {}", e); }
            }
        }
        let switch = billing_switch.ok_or(last_err)?;
        if !switch.is_credits_billing {
            return Ok(CreditSummary {
                is_credits_billing: false,
                plan_name: switch.user_pay_identity_str.clone().unwrap_or_else(|| "Free".to_string()),
                plan_expire_time: switch.plan_expire_time,
                ..Default::default()
            });
        }

        // ---------- Step 2: v2 entitlement list — 真正的积分数值 ----------
        let entitlements = self.get_entitlement_list_v2_by_token().await?;
        println!("[DEBUG] credits step2 OK: entitlement_packs.len = {}",
                 entitlements.user_entitlement_pack_list.len());

        // ---------- Step 3: web_user_pay_status — 拿 plan 名（最佳努力） ----------
        let pay_status = match self.get_web_user_pay_status_by_token().await {
            Ok(p) => {
                println!("[DEBUG] credits step3 OK: user_pay_identity_str = {:?}",
                         p.user_pay_identity_str);
                Some(p)
            }
            Err(e) => {
                println!("[WARN] 获取 pay_status 失败，忽略: {}", e);
                None
            }
        };

        // ---------- 聚合 ----------
        let summary = Self::assemble_credit_summary_from_parts(
            switch, entitlements, pay_status,
        );
        println!(
            "[DEBUG] ===== CREDIT SUMMARY =====\n  is_credits_billing={}\n  plan_name={:?}\n  plan_expire={}\n  total_available={}\n  general: total={} used={} left={}\n  work:    total={} used={} left={}\n  reward_total_left={}\n  reward_entries={}",
            summary.is_credits_billing,
            summary.plan_name,
            summary.plan_expire_time,
            summary.total_available,
            summary.general.total_limit, summary.general.used, summary.general.left,
            summary.work_exclusive.total_limit, summary.work_exclusive.used, summary.work_exclusive.left,
            summary.reward_total_left,
            summary.reward_entries.len(),
        );
        for (i, r) in summary.reward_entries.iter().enumerate() {
            println!("  reward[{}]: {:?} | total={} used={} scope={} expire={}",
                     i, r.title, r.total, r.used, r.scope, r.expire_time);
        }
        Ok(summary)
    }

    /// 将 billing_switch + v2 entitlement_list + pay_status 拼成前端展示用的 CreditSummary
    fn assemble_credit_summary_from_parts(
        switch: CreditsBillingStatusResponse,
        entitlements: EntitlementListResponse,
        pay_status: Option<WebUserPayStatusResponse>,
    ) -> CreditSummary {
        // 打印 user_id 便于定位账号
        let user_id = entitlements.user_entitlement_pack_list.first()
            .map(|p| p.entitlement_base_info.user_id.clone())
            .unwrap_or_else(|| "unknown".to_string());
        println!("[DEBUG] ===== ASSEMBLE CREDIT SUMMARY for user_id={} ===== ({} packs)",
            user_id, entitlements.user_entitlement_pack_list.len());
        if !switch.is_credits_billing {
            return CreditSummary {
                is_credits_billing: false,
                plan_name: pay_status.as_ref().map(|p| p.user_pay_identity_str.clone())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| {
                        switch.user_pay_identity_str.clone().unwrap_or_else(|| "Free".to_string())
                    }),
                plan_expire_time: switch.plan_expire_time,
                ..Default::default()
            };
        }

        let mut general_total = 0.0f64;
        let mut general_used  = 0.0f64;
        let mut general_nearest_expire = 0i64;
        let mut work_total    = 0.0f64;
        let mut work_used     = 0.0f64;
        let mut work_nearest_expire = 0i64;
        let mut plan_end_time = 0i64;
        let mut reward_entries: Vec<RewardCreditsEntry> = Vec::new();

        for pack in &entitlements.user_entitlement_pack_list {
            let base = &pack.entitlement_base_info;
            let quota = &base.quota;
            let usage = &pack.usage;
            let limit = quota.credits_limit;
            let used  = usage.credits_amount;

            // status != 1 的 pack（比如未生效 / 已过期的免费兑换 pack 也显示，但不包含在主套餐里）
            // 只统计 credits_limit > 0 且 is_hide == false 的条目
            if limit <= 0.0 || pack.is_hide {
                println!("[DEBUG] SKIP pack: pid={} desc={:?} limit={} is_hide={} status={}",
                    base.product_id, pack.display_desc, limit, pack.is_hide, pack.status);
                continue;
            }

            // 主套餐订阅（product_type == 0）的 end_time 作为 plan_expire_time
            if base.product_type == 0 && pack.status == 1 {
                if base.end_time > plan_end_time { plan_end_time = base.end_time; }
            }

            let scope = match base.available_endpoint {
                0 => "general",          // 通用：TraeCode + TraeWork
                1 => "work_exclusive",   // Work 专属
                _ => "general",
            };

            println!(
                "[DEBUG] pack: pid={} ptype={} avail_ep={} -> scope={} | credits_limit={} used={} left={} | status={} group_type={} is_hide={} desc={:?} end={}",
                base.product_id, base.product_type, base.available_endpoint, scope,
                limit, used, limit - used, pack.status, pack.group_type, pack.is_hide, pack.display_desc, base.end_time,
            );

            match scope {
                "general" => {
                    general_total += limit;
                    general_used  += used;
                    println!("[DEBUG]   -> general running: total={} used={} left={}",
                        general_total, general_used, general_total - general_used);
                    if base.end_time > 0 && (general_nearest_expire == 0 || base.end_time < general_nearest_expire) {
                        general_nearest_expire = base.end_time;
                    }
                }
                _ => {
                    work_total += limit;
                    work_used  += used;
                    println!("[DEBUG]   -> work running: total={} used={} left={}",
                        work_total, work_used, work_total - work_used);
                    if base.end_time > 0 && (work_nearest_expire == 0 || base.end_time < work_nearest_expire) {
                        work_nearest_expire = base.end_time;
                    }
                }
            }

            // 奖励 / 兑换 / 礼包类的 pack 生成奖励条目
            let is_rewardish = base.product_type == 2      // 礼包/兑换
                || pack.group_type == 4                    // 分组：奖励
                || base.product_id >= 200;                 // 高 product_id 一般是活动积分
            if is_rewardish {
                reward_entries.push(RewardCreditsEntry {
                    title: if pack.display_desc.trim().is_empty() {
                        base.product_extra.package_extra.as_ref()
                            .map(|p| p.package_name.clone())
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| "积分礼包".to_string())
                    } else {
                        pack.display_desc.clone()
                    },
                    scope: scope.to_string(),
                    total: limit,
                    used,
                    expire_time: base.end_time,
                    sub_count: 1,
                });
            }
        }

        // 如果 v2 entitlements 本身也暴露了主 subscription 的 end_time 就用它，否则兜底
        if plan_end_time == 0 {
            // fallback: 取所有 pack 中的最大 end_time
            if let Some(m) = entitlements.user_entitlement_pack_list.iter()
                .map(|p| p.entitlement_base_info.end_time).max() { plan_end_time = m; }
        }
        if switch.plan_expire_time > 0 { plan_end_time = switch.plan_expire_time; }

        let general_left = (general_total - general_used).max(0.0);
        let work_left    = (work_total    - work_used).max(0.0);

        let reward_total_left = reward_entries.iter()
            .map(|e| (e.total - e.used).max(0.0)).sum::<f64>();

        let plan_name = pay_status.as_ref()
            .map(|p| p.user_pay_identity_str.clone())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| switch.user_pay_identity_str.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| if !switch.plan_name.trim().is_empty() {
                switch.plan_name.clone()
            } else { "Free".to_string() });

        CreditSummary {
            is_credits_billing: true,
            plan_name,
            plan_expire_time: plan_end_time,
            total_available: general_left + work_left,
            general: CreditsCategory {
                total_limit: general_total,
                used: general_used,
                left: general_left,
                nearest_expire_time: general_nearest_expire,
            },
            work_exclusive: CreditsCategory {
                total_limit: work_total,
                used: work_used,
                left: work_left,
                nearest_expire_time: work_nearest_expire,
            },
            reward_total_left,
            reward_entries,
        }
    }

    /// 旧解析函数保留兜底：当 billing_status 响应本身就带数值时使用（接口扩展的新字段未来生效时）
    #[allow(dead_code)]
    fn parse_credits_to_summary(raw: CreditsBillingStatusResponse) -> CreditSummary {
        if !raw.is_credits_billing {
            return CreditSummary {
                is_credits_billing: false,
                plan_name: raw.plan_name,
                plan_expire_time: raw.plan_expire_time,
                ..Default::default()
            };
        }
        let fixup = |mut c: CreditsCategory| -> CreditsCategory {
            if c.total_limit > 0.0 && c.left.abs() < f64::EPSILON {
                c.left = (c.total_limit - c.used).max(0.0);
            }
            c
        };
        let general = fixup(raw.general_credits);
        let work_exclusive = fixup(raw.work_exclusive_credits);
        let (reward_total_left, reward_entries) = match raw.reward_credits {
            Some(r) => (r.total_left, r.entries),
            None => (0.0, Vec::new()),
        };
        let plan_name = if raw.plan_name.trim().is_empty() {
            raw.user_pay_identity_str.clone().unwrap_or_else(|| "Free".to_string())
        } else { raw.plan_name.clone() };
        CreditSummary {
            is_credits_billing: true,
            plan_name,
            plan_expire_time: raw.plan_expire_time,
            total_available: general.left + work_exclusive.left,
            general,
            work_exclusive,
            reward_total_left,
            reward_entries,
        }
    }
}
