//! 账号数据导入导出与清空。

use super::*;

impl AccountManager {
    /// 清空所有账号数据
    pub fn clear_all_accounts(&mut self) -> Result<usize> {
        let count = self.store.accounts.len();
        self.store.accounts.clear();
        self.store.active_account_id = None;
        self.store.current_account_id = None;
        self.save_store()?;
        println!("[INFO] 已清空所有账号数据，共删除 {} 个账号", count);
        Ok(count)
    }

    /// 导出账号数据
    /// login_source 采用 AccountBrief 运行时修正后的有效值，而非 Account 原始值：
    /// - oauth_enabled=true → 强制 WebviewOauth（无论 Account.login_source 是什么）
    /// - Unknown + 有 cookies → 推断 WebviewCookie
    pub fn export_accounts(&self) -> Result<String> {
        let export_data: Vec<serde_json::Value> = self.store.accounts.iter().map(|acc| {
            // —— 计算修正后的有效 login_source（与 get_accounts 中 AccountBrief 推导逻辑一致）——
            let effective_source = if acc.oauth_enabled {
                // OAuth 凭证已启用，无论 Account.login_source 原始值是什么，导出值强制修正为 WebviewOauth
                AccountLoginSource::WebviewOauth
            } else if acc.login_source == AccountLoginSource::Unknown {
                // Unknown 按凭证特征推断
                if !acc.cookies.is_empty() { AccountLoginSource::WebviewCookie }
                else { AccountLoginSource::Unknown }
            } else {
                acc.login_source
            };
            serde_json::json!({
                "name": acc.name,
                "email": acc.email,
                "cookies": acc.cookies,
                "user_id": acc.user_id,
                "tenant_id": acc.tenant_id,
                "region": acc.region,
                "plan_type": acc.plan_type,
                "avatar_url": acc.avatar_url,
                "jwt_token": acc.jwt_token,
                "machine_id": acc.machine_id,
                "login_source": effective_source,
                // —— OAuth Native 完整凭证链（v1.0.0 新增，防止导入后 OAuth 账号丢失自管能力）——
                "oauth_enabled": acc.oauth_enabled,
                "oauth_refresh_token": acc.oauth_refresh_token,
                "oauth_refresh_expired_at": acc.oauth_refresh_expired_at,
                "oauth_public_key_pem": acc.oauth_public_key_pem,
                "oauth_private_key_pem": acc.oauth_private_key_pem,
                "oauth_device_id": acc.oauth_device_id,
                "oauth_machine_id": acc.oauth_machine_id,
                "oauth_host": acc.oauth_host,
            })
        }).collect();

        serde_json::to_string_pretty(&export_data)
            .map_err(|e| anyhow!("导出失败: {}", e))
    }

    /// 导入账号数据
    pub async fn import_accounts(&mut self, data: &str) -> Result<usize> {
        let import_data: Vec<serde_json::Value> = serde_json::from_str(data)
            .map_err(|e| anyhow!("JSON 解析失败: {}", e))?;
        let mut imported_count = 0;

        for item in import_data.iter() {
            let token = item.get("jwt_token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let cookies = item.get("cookies")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_name = item.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_avatar = item.get("avatar_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_user_id = item.get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let exported_region = item.get("region")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // 读取导出的来源标记（老格式导出可能没有该字段，默认 Unknown）
            let exported_login_source: AccountLoginSource = item.get("login_source")
                .and_then(|v| serde_json::from_value::<AccountLoginSource>(v.clone()).ok())
                .unwrap_or_default();

            // 读取 OAuth Native 完整凭证链（老格式导出可能没有这些字段，默认零值）
            let oauth_enabled = item.get("oauth_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let oauth_refresh_token = item.get("oauth_refresh_token").and_then(|v| v.as_str().map(|s| s.to_string()));
            let oauth_refresh_expired_at = item.get("oauth_refresh_expired_at").and_then(|v| v.as_str().map(|s| s.to_string()));
            let oauth_public_key_pem = item.get("oauth_public_key_pem").and_then(|v| v.as_str().map(|s| s.to_string()));
            let oauth_private_key_pem = item.get("oauth_private_key_pem").and_then(|v| v.as_str().map(|s| s.to_string()));
            let oauth_device_id = item.get("oauth_device_id").and_then(|v| v.as_str().map(|s| s.to_string()));
            let oauth_machine_id = item.get("oauth_machine_id").and_then(|v| v.as_str().map(|s| s.to_string()));
            let oauth_host = item.get("oauth_host").and_then(|v| v.as_str().map(|s| s.to_string()));

            // 优先使用 Token 添加（Token 方式更稳定，且不依赖 cookies）
            let add_success = if !token.is_empty() {
                let cookies_opt = if cookies.is_empty() { None } else { Some(cookies.clone()) };
                let preferred_name = if exported_name.is_empty() { None } else { Some(exported_name.clone()) };
                // 使用导出的来源标记，而不是硬编码 ManualToken
                match self.add_account_by_token(token.clone(), cookies_opt, preferred_name, exported_login_source).await {
                    Ok(_) => true,
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("已存在") {
                            // 账号已存在，也算导入成功（后续会更新名称 + 来源 + OAuth 字段）
                            true
                        } else {
                            // API 调用失败（如网络不可达、Token 过期等），尝试直接从导出数据创建账号
                            let fallback_user_id = if !exported_user_id.is_empty() {
                                exported_user_id.clone()
                            } else {
                                crate::api::TraeApiClient::parse_jwt_user_id(&token).unwrap_or_default()
                            };
                            if !fallback_user_id.is_empty() {
                                let fallback_tenant_id = item.get("tenant_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let mut fallback_account = Account::new(
                                    exported_name.clone(),
                                    item.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    cookies.clone(),
                                    fallback_user_id,
                                    fallback_tenant_id,
                                );
                                fallback_account.avatar_url = exported_avatar.clone();
                                fallback_account.region = exported_region.clone();
                                fallback_account.jwt_token = Some(token.clone());
                                // 从 JWT 解析 exp 设置 token_expired_at，避免后台 refresh 误判
                                fallback_account.token_expired_at = crate::api::TraeApiClient::parse_jwt_exp(&token)
                                    .ok()
                                    .and_then(|exp| {
                                        chrono::DateTime::from_timestamp(exp, 0)
                                            .map(|dt| dt.to_rfc3339())
                                    });
                                // 恢复导出的来源标记 + OAuth 凭证链（clone 保留原值供补丁阶段使用）
                                fallback_account.login_source = exported_login_source;
                                fallback_account.oauth_enabled = oauth_enabled;
                                fallback_account.oauth_refresh_token = oauth_refresh_token.clone();
                                fallback_account.oauth_refresh_expired_at = oauth_refresh_expired_at.clone();
                                fallback_account.oauth_public_key_pem = oauth_public_key_pem.clone();
                                fallback_account.oauth_private_key_pem = oauth_private_key_pem.clone();
                                fallback_account.oauth_device_id = oauth_device_id.clone();
                                fallback_account.oauth_machine_id = oauth_machine_id.clone();
                                fallback_account.oauth_host = oauth_host.clone();

                                let already_exists = self.store.accounts.iter().any(|a| a.user_id == fallback_account.user_id);
                                if already_exists {
                                    true
                                } else {
                                    self.store.accounts.push(fallback_account);
                                    if self.store.active_account_id.is_none() {
                                        self.store.active_account_id = Some(self.store.accounts.last().unwrap().id.clone());
                                    }
                                    true
                                }
                            } else {
                                false
                            }
                        }
                    }
                }
            } else if !cookies.is_empty() {
                match self.add_account(cookies).await {
                    Ok(_) => true,
                    Err(e) => {
                        !e.to_string().contains("已存在")
                    }
                }
            } else {
                false
            };

            // 补丁阶段：无论是新建、已存在还是 fallback，统一补全导出时的来源 + OAuth 字段
            // 顺序：先恢复 OAuth 凭证（决定账号类型），再按凭证修正 login_source
            // （因为 add_account_by_token / add_account 只负责打基础凭证，来源和 OAuth 链需要额外恢复）
            if !exported_user_id.is_empty() {
                if let Some(acc) = self.store.accounts.iter_mut().find(|a| a.user_id == exported_user_id) {
                    if !exported_name.is_empty() {
                        acc.name = exported_name.clone();
                    }
                    if !exported_avatar.is_empty() {
                        acc.avatar_url = exported_avatar.clone();
                    }
                    if !exported_region.is_empty() {
                        acc.region = exported_region.clone();
                    }
                    // 1. 先恢复 OAuth Native 凭证链（只要导出值有任何一个 OAuth 字段，就整体覆盖）
                    let has_oauth_in_export = oauth_enabled || oauth_refresh_token.is_some();
                    if has_oauth_in_export {
                        acc.oauth_enabled = oauth_enabled;
                        acc.oauth_refresh_token = oauth_refresh_token;
                        acc.oauth_refresh_expired_at = oauth_refresh_expired_at;
                        acc.oauth_public_key_pem = oauth_public_key_pem;
                        acc.oauth_private_key_pem = oauth_private_key_pem;
                        acc.oauth_device_id = oauth_device_id;
                        acc.oauth_machine_id = oauth_machine_id;
                        acc.oauth_host = oauth_host;
                    }
                    // 2. 确保 token_expired_at 被正确解析（所有路径兜底：add_account_by_token / fallback / 已存在账号）
                    if acc.token_expired_at.is_none() {
                        if let Some(jwt) = acc.jwt_token.as_deref() {
                            acc.token_expired_at = crate::api::TraeApiClient::parse_jwt_exp(jwt)
                                .ok()
                                .and_then(|exp| {
                                    chrono::DateTime::from_timestamp(exp, 0)
                                        .map(|dt| dt.to_rfc3339())
                                });
                        }
                    }
                    // 3. 再设置 login_source：OAuth 凭证优先修正（对齐 AccountBrief 运行时逻辑）
                    if acc.oauth_enabled {
                        acc.login_source = AccountLoginSource::WebviewOauth;
                    } else if exported_login_source != AccountLoginSource::Unknown {
                        acc.login_source = exported_login_source;
                    }
                }
            }

            if add_success {
                imported_count += 1;
            }
        }

        self.save_store()?;
        Ok(imported_count)
    }

}
